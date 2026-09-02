use std::{
    collections::{BTreeMap, HashSet},
    fs,
    path::{Path, PathBuf},
    sync::{
        Arc, RwLock,
        atomic::{AtomicU64, Ordering},
    },
    time::Instant,
};

use anyhow::{Context, Result, bail};
use mlua::{
    Function, HookTriggers, Lua, LuaOptions, LuaSerdeExt, StdLib, Table, Value, VmState,
};

use crate::{
    config::LuaLimits,
    model::{CommandContext, CommandManifest, LuaAction, ModuleManifest, ModuleSummary},
};

#[derive(Debug, Clone)]
pub struct LuaEngine {
    scripts_dir: PathBuf,
    limits: LuaLimits,
    modules: Arc<RwLock<BTreeMap<String, LoadedModule>>>,
}

#[derive(Debug, Clone)]
struct LoadedModule {
    manifest: ModuleManifest,
    source: Arc<str>,
    file_name: String,
}

#[derive(Debug, Clone)]
pub struct ResolvedLuaCommand {
    pub module_name: String,
    pub command: CommandManifest,
}

impl LuaEngine {
    pub fn load(scripts_dir: impl AsRef<Path>, limits: LuaLimits) -> Result<Self> {
        let engine = Self {
            scripts_dir: scripts_dir.as_ref().to_path_buf(),
            limits,
            modules: Arc::new(RwLock::new(BTreeMap::new())),
        };
        engine.reload()?;
        Ok(engine)
    }

    pub fn reload(&self) -> Result<usize> {
        let mut files = fs::read_dir(&self.scripts_dir)
            .with_context(|| format!("failed to read {}", self.scripts_dir.display()))?
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|path| path.extension().and_then(|extension| extension.to_str()) == Some("lua"))
            .collect::<Vec<_>>();
        files.sort();

        let mut next = BTreeMap::new();
        let mut commands = HashSet::new();

        for path in files {
            let source = fs::read_to_string(&path)
                .with_context(|| format!("failed to read {}", path.display()))?;
            let file_name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("module.lua")
                .to_owned();
            let manifest = parse_manifest(&source, &file_name, self.limits)?;
            validate_manifest(&manifest)?;

            if next.contains_key(&manifest.name) {
                bail!("duplicate Lua module name: {}", manifest.name);
            }
            for command in &manifest.commands {
                if !commands.insert(command.name.clone()) {
                    bail!("duplicate slash command name: {}", command.name);
                }
            }

            next.insert(
                manifest.name.clone(),
                LoadedModule {
                    manifest,
                    source: Arc::from(source),
                    file_name,
                },
            );
        }

        if next.is_empty() {
            bail!("no Lua modules were found in {}", self.scripts_dir.display());
        }

        let count = next.len();
        *self.modules.write().expect("Lua module lock poisoned") = next;
        tracing::info!(count, "Lua modules loaded atomically");
        Ok(count)
    }

    pub fn module_summaries(&self) -> Vec<ModuleSummary> {
        self.modules
            .read()
            .expect("Lua module lock poisoned")
            .values()
            .map(|module| ModuleSummary::from(&module.manifest))
            .collect()
    }

    pub fn manifests(&self) -> Vec<ModuleManifest> {
        self.modules
            .read()
            .expect("Lua module lock poisoned")
            .values()
            .map(|module| module.manifest.clone())
            .collect()
    }

    pub fn find_command(&self, command_name: &str) -> Option<ResolvedLuaCommand> {
        self.modules
            .read()
            .expect("Lua module lock poisoned")
            .values()
            .find_map(|module| {
                module
                    .manifest
                    .commands
                    .iter()
                    .find(|command| command.name == command_name)
                    .cloned()
                    .map(|command| ResolvedLuaCommand {
                        module_name: module.manifest.name.clone(),
                        command,
                    })
            })
    }

    pub fn execute(&self, module_name: &str, context: CommandContext) -> Result<Vec<LuaAction>> {
        let module = self
            .modules
            .read()
            .expect("Lua module lock poisoned")
            .get(module_name)
            .cloned()
            .with_context(|| format!("Lua module {module_name} is not loaded"))?;

        let lua = sandbox(self.limits)?;
        let module_table: Table = lua
            .load(module.source.as_ref())
            .set_name(&module.file_name)
            .eval()
            .with_context(|| format!("failed to evaluate {}", module.file_name))?;
        let handler: Function = module_table
            .get("handle")
            .with_context(|| format!("{} must export handle(ctx)", module.file_name))?;
        let input = lua.to_value(&context)?;
        let output: Value = handler
            .call(input)
            .with_context(|| format!("Lua command {} failed", context.command))?;

        if output.is_nil() {
            return Ok(Vec::new());
        }

        lua.from_value(output)
            .context("Lua handler must return an array of actions")
    }
}

fn parse_manifest(source: &str, file_name: &str, limits: LuaLimits) -> Result<ModuleManifest> {
    let lua = sandbox(limits)?;
    let module: Table = lua
        .load(source)
        .set_name(file_name)
        .eval()
        .with_context(|| format!("failed to evaluate {file_name}"))?;
    let manifest: Table = module
        .get("manifest")
        .with_context(|| format!("{file_name} must export a manifest table"))?;
    lua.from_value(Value::Table(manifest))
        .with_context(|| format!("invalid manifest in {file_name}"))
}

fn sandbox(limits: LuaLimits) -> Result<Lua> {
    let lua = Lua::new_with(StdLib::ALL_SAFE, LuaOptions::default())?;
    lua.set_memory_limit(limits.memory_bytes)?;

    let started = Instant::now();
    let remaining = Arc::new(AtomicU64::new(limits.instruction_limit));
    let budget = Arc::clone(&remaining);
    lua.set_hook(
        HookTriggers::new().every_nth_instruction(1_000),
        move |_lua, _debug| {
            if started.elapsed() > limits.timeout {
                return Err(mlua::Error::RuntimeError(
                    "Lua execution exceeded the wall-clock limit".to_owned(),
                ));
            }

            let previous = budget.fetch_sub(1_000, Ordering::Relaxed);
            if previous <= 1_000 {
                return Err(mlua::Error::RuntimeError(
                    "Lua execution exceeded the instruction limit".to_owned(),
                ));
            }

            Ok(VmState::Continue)
        },
    )?;

    let print = lua.create_function(|_lua, message: String| {
        tracing::info!(target: "lua", %message);
        Ok(())
    })?;
    lua.globals().set("print", print)?;

    Ok(lua)
}

fn validate_manifest(manifest: &ModuleManifest) -> Result<()> {
    validate_identifier("module", &manifest.name, 32)?;
    if manifest.description.len() > 200 {
        bail!("module {} description is too long", manifest.name);
    }
    if manifest.commands.is_empty() {
        bail!("module {} does not define any commands", manifest.name);
    }

    for command in &manifest.commands {
        validate_identifier("command", &command.name, 32)?;
        if command.description.is_empty() || command.description.len() > 100 {
            bail!(
                "command {} description must contain 1 to 100 characters",
                command.name
            );
        }

        for option in &command.options {
            validate_identifier("option", &option.name, 32)?;
            if option.description.is_empty() || option.description.len() > 100 {
                bail!(
                    "option {} description must contain 1 to 100 characters",
                    option.name
                );
            }
        }
    }

    Ok(())
}

fn validate_identifier(kind: &str, value: &str, maximum: usize) -> Result<()> {
    let valid = !value.is_empty()
        && value.len() <= maximum
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_' || byte == b'-');

    if !valid {
        bail!(
            "{kind} identifier {value:?} must be lowercase ASCII and may contain digits, '_' or '-'"
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use tempfile::tempdir;

    use super::*;

    #[test]
    fn loads_and_executes_a_module() {
        let directory = tempdir().unwrap();
        fs::write(
            directory.path().join("test.lua"),
            r#"
return {
  manifest = {
    name = "test",
    version = "1.0.0",
    description = "test module",
    commands = {{ name = "hello", description = "Say hello" }}
  },
  handle = function(ctx)
    return {{ type = "reply", content = "hello " .. ctx.username, ephemeral = true }}
  end
}
"#,
        )
        .unwrap();

        let engine = LuaEngine::load(
            directory.path(),
            LuaLimits {
                memory_bytes: 2 * 1024 * 1024,
                instruction_limit: 100_000,
                timeout: Duration::from_millis(100),
            },
        )
        .unwrap();

        let actions = engine
            .execute(
                "test",
                CommandContext {
                    command: "hello".to_owned(),
                    guild_id: Some("1".to_owned()),
                    channel_id: "2".to_owned(),
                    user_id: "3".to_owned(),
                    username: "Adrian".to_owned(),
                    options: Default::default(),
                    module_config: Value::Null,
                },
            )
            .unwrap();

        assert!(matches!(
            actions.as_slice(),
            [LuaAction::Reply { content, ephemeral: true }] if content == "hello Adrian"
        ));
    }
}
