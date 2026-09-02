use std::{
    collections::{BTreeMap, HashMap},
    fs,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicI64, Ordering},
    },
};

use mlua::{Error as MluaError, Function, HookTriggers, Lua, LuaSerdeExt, Table, Value, VmState};
use thiserror::Error;
use tokio::sync::RwLock;

use super::model::{
    LuaAction, LuaCommandOption, LuaEventContext, LuaExecutionContext, LuaModuleManifest,
    LuaOptionKind,
};

const MAX_ACTIONS_PER_EXECUTION: usize = 25;
const MAX_COMMANDS: usize = 100;
const MAX_OPTIONS_PER_LEVEL: usize = 25;

#[derive(Clone, Copy, Debug)]
pub struct LuaLimits {
    pub memory_bytes: usize,
    pub instruction_limit: i64,
    pub hook_granularity: u32,
}

#[derive(Clone)]
pub struct LuaEngine {
    scripts_dir: Arc<PathBuf>,
    limits: LuaLimits,
    registry: Arc<RwLock<Registry>>,
}

#[derive(Default)]
struct Registry {
    modules: BTreeMap<String, Arc<LoadedModule>>,
    command_to_module: HashMap<String, String>,
    event_to_modules: HashMap<String, Vec<String>>,
}

#[derive(Clone)]
struct LoadedModule {
    manifest: LuaModuleManifest,
    source: Arc<str>,
    display_name: Arc<str>,
}

#[derive(Debug, Error)]
pub enum LuaEngineError {
    #[error("cannot read Lua scripts from {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("Lua runtime error in {script}: {source}")]
    Runtime {
        script: String,
        #[source]
        source: MluaError,
    },
    #[error("invalid Lua module {script}: {message}")]
    InvalidModule { script: String, message: String },
    #[error("duplicate Lua module id {0}")]
    DuplicateModule(String),
    #[error("duplicate slash command /{0}")]
    DuplicateCommand(String),
    #[error("unknown Lua module {0}")]
    UnknownModule(String),
    #[error("module {module_id} does not implement command /{command}")]
    UnknownCommand { module_id: String, command: String },
    #[error("module {module_id} does not subscribe to event {event}")]
    UnknownEvent { module_id: String, event: String },
    #[error("Lua execution task failed: {0}")]
    TaskJoin(#[from] tokio::task::JoinError),
}

impl LuaEngine {
    pub fn load(scripts_dir: impl AsRef<Path>, limits: LuaLimits) -> Result<Self, LuaEngineError> {
        let scripts_dir = scripts_dir.as_ref().to_path_buf();
        let registry = build_registry(&scripts_dir, limits)?;

        Ok(Self {
            scripts_dir: Arc::new(scripts_dir),
            limits,
            registry: Arc::new(RwLock::new(registry)),
        })
    }

    pub async fn reload(&self) -> Result<usize, LuaEngineError> {
        let path = Arc::clone(&self.scripts_dir);
        let limits = self.limits;
        let next = tokio::task::spawn_blocking(move || build_registry(&path, limits)).await??;
        let count = next.modules.len();
        *self.registry.write().await = next;
        Ok(count)
    }

    pub async fn manifests(&self) -> Vec<LuaModuleManifest> {
        self.registry
            .read()
            .await
            .modules
            .values()
            .map(|module| module.manifest.clone())
            .collect()
    }

    pub async fn command_definitions(&self) -> Vec<super::model::LuaCommandDefinition> {
        self.registry
            .read()
            .await
            .modules
            .values()
            .flat_map(|module| module.manifest.commands.clone())
            .collect()
    }

    pub async fn module_for_command(&self, command: &str) -> Option<String> {
        self.registry
            .read()
            .await
            .command_to_module
            .get(command)
            .cloned()
    }

    pub async fn modules_for_event(&self, event: &str) -> Vec<String> {
        self.registry
            .read()
            .await
            .event_to_modules
            .get(event)
            .cloned()
            .unwrap_or_default()
    }

    pub async fn manifest(&self, module_id: &str) -> Option<LuaModuleManifest> {
        self.registry
            .read()
            .await
            .modules
            .get(module_id)
            .map(|module| module.manifest.clone())
    }

    pub async fn execute_command(
        &self,
        module_id: &str,
        command: &str,
        context: LuaExecutionContext,
    ) -> Result<Vec<LuaAction>, LuaEngineError> {
        let module = self
            .registry
            .read()
            .await
            .modules
            .get(module_id)
            .cloned()
            .ok_or_else(|| LuaEngineError::UnknownModule(module_id.to_owned()))?;

        if !module
            .manifest
            .commands
            .iter()
            .any(|definition| definition.name == command)
        {
            return Err(LuaEngineError::UnknownCommand {
                module_id: module_id.to_owned(),
                command: command.to_owned(),
            });
        }

        let limits = self.limits;
        let command = command.to_owned();
        let module_id = module_id.to_owned();
        tokio::task::spawn_blocking(move || {
            execute_handler(
                &module,
                limits,
                "on_command",
                (&command, &context),
                &module_id,
            )
        })
        .await?
    }

    pub async fn execute_event(
        &self,
        module_id: &str,
        event: &str,
        context: LuaEventContext,
    ) -> Result<Vec<LuaAction>, LuaEngineError> {
        let module = self
            .registry
            .read()
            .await
            .modules
            .get(module_id)
            .cloned()
            .ok_or_else(|| LuaEngineError::UnknownModule(module_id.to_owned()))?;

        if !module.manifest.events.iter().any(|name| name == event) {
            return Err(LuaEngineError::UnknownEvent {
                module_id: module_id.to_owned(),
                event: event.to_owned(),
            });
        }

        let limits = self.limits;
        let event = event.to_owned();
        let module_id = module_id.to_owned();
        tokio::task::spawn_blocking(move || {
            execute_handler(&module, limits, "on_event", (&event, &context), &module_id)
        })
        .await?
    }
}

fn build_registry(scripts_dir: &Path, limits: LuaLimits) -> Result<Registry, LuaEngineError> {
    let mut paths = Vec::new();
    collect_lua_files(scripts_dir, &mut paths)?;
    paths.sort();

    let mut registry = Registry::default();

    for path in paths {
        let source = fs::read_to_string(&path).map_err(|source| LuaEngineError::Io {
            path: path.clone(),
            source,
        })?;
        let display_name = path.display().to_string();
        let manifest = inspect_manifest(&source, &display_name, limits)?;
        validate_manifest(&manifest, &display_name)?;

        if registry.modules.contains_key(&manifest.id) {
            return Err(LuaEngineError::DuplicateModule(manifest.id));
        }

        for command in &manifest.commands {
            if registry
                .command_to_module
                .insert(command.name.clone(), manifest.id.clone())
                .is_some()
            {
                return Err(LuaEngineError::DuplicateCommand(command.name.clone()));
            }
        }

        for event in &manifest.events {
            registry
                .event_to_modules
                .entry(event.clone())
                .or_default()
                .push(manifest.id.clone());
        }

        registry.modules.insert(
            manifest.id.clone(),
            Arc::new(LoadedModule {
                manifest,
                source: Arc::from(source),
                display_name: Arc::from(display_name),
            }),
        );
    }

    if registry.modules.is_empty() {
        return Err(LuaEngineError::InvalidModule {
            script: scripts_dir.display().to_string(),
            message: "no .lua modules were found".to_owned(),
        });
    }
    if registry.command_to_module.len() > MAX_COMMANDS {
        return Err(LuaEngineError::InvalidModule {
            script: scripts_dir.display().to_string(),
            message: format!(
                "the project defines {} global commands; Discord allows at most {MAX_COMMANDS}",
                registry.command_to_module.len()
            ),
        });
    }

    Ok(registry)
}

fn collect_lua_files(path: &Path, output: &mut Vec<PathBuf>) -> Result<(), LuaEngineError> {
    let entries = fs::read_dir(path).map_err(|source| LuaEngineError::Io {
        path: path.to_path_buf(),
        source,
    })?;

    for entry in entries {
        let entry = entry.map_err(|source| LuaEngineError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        let entry_path = entry.path();
        if entry_path.is_dir() {
            collect_lua_files(&entry_path, output)?;
        } else if entry_path
            .extension()
            .is_some_and(|extension| extension == "lua")
        {
            output.push(entry_path);
        }
    }

    Ok(())
}

fn inspect_manifest(
    source: &str,
    display_name: &str,
    limits: LuaLimits,
) -> Result<LuaModuleManifest, LuaEngineError> {
    let lua = sandboxed_lua(limits).map_err(|source| LuaEngineError::Runtime {
        script: display_name.to_owned(),
        source,
    })?;
    let module: Table = lua
        .load(source)
        .set_name(display_name)
        .eval()
        .map_err(|source| LuaEngineError::Runtime {
            script: display_name.to_owned(),
            source,
        })?;
    let manifest_value: Value =
        module
            .get("manifest")
            .map_err(|source| LuaEngineError::Runtime {
                script: display_name.to_owned(),
                source,
            })?;

    lua.from_value(manifest_value)
        .map_err(|source| LuaEngineError::Runtime {
            script: display_name.to_owned(),
            source,
        })
}

fn execute_handler<T: serde::Serialize>(
    module: &LoadedModule,
    limits: LuaLimits,
    handler_name: &str,
    arguments: (&str, &T),
    module_id: &str,
) -> Result<Vec<LuaAction>, LuaEngineError> {
    let lua = sandboxed_lua(limits).map_err(|source| LuaEngineError::Runtime {
        script: module.display_name.to_string(),
        source,
    })?;
    let module_table: Table = lua
        .load(module.source.as_ref())
        .set_name(module.display_name.as_ref())
        .eval()
        .map_err(|source| LuaEngineError::Runtime {
            script: module.display_name.to_string(),
            source,
        })?;
    let handler: Function =
        module_table
            .get(handler_name)
            .map_err(|source| LuaEngineError::Runtime {
                script: module.display_name.to_string(),
                source,
            })?;
    let context = lua
        .to_value(arguments.1)
        .map_err(|source| LuaEngineError::Runtime {
            script: module.display_name.to_string(),
            source,
        })?;
    let output: Value =
        handler
            .call((arguments.0, context))
            .map_err(|source| LuaEngineError::Runtime {
                script: module.display_name.to_string(),
                source,
            })?;
    let actions: Vec<LuaAction> =
        lua.from_value(output)
            .map_err(|source| LuaEngineError::Runtime {
                script: module.display_name.to_string(),
                source,
            })?;

    if actions.len() > MAX_ACTIONS_PER_EXECUTION {
        return Err(LuaEngineError::InvalidModule {
            script: module_id.to_owned(),
            message: format!(
                "a script returned {} actions; the limit is {MAX_ACTIONS_PER_EXECUTION}",
                actions.len()
            ),
        });
    }
    for action in &actions {
        action
            .validate()
            .map_err(|message| LuaEngineError::InvalidModule {
                script: module_id.to_owned(),
                message,
            })?;
    }

    Ok(actions)
}

fn sandboxed_lua(limits: LuaLimits) -> mlua::Result<Lua> {
    let lua = Lua::new();
    lua.set_memory_limit(limits.memory_bytes)?;

    let globals = lua.globals();
    for denied in [
        "collectgarbage",
        "coroutine",
        "debug",
        "dofile",
        "io",
        "load",
        "loadfile",
        "os",
        "package",
        "pcall",
        "require",
        "xpcall",
    ] {
        globals.set(denied, Value::Nil)?;
    }

    let api = lua.create_table()?;
    api.set("api_version", 1)?;
    api.set(
        "escape_mentions",
        lua.create_function(|_, input: String| Ok(input.replace('@', "@\u{200B}")))?,
    )?;
    api.set(
        "truncate",
        lua.create_function(|_, (input, max_chars): (String, usize)| {
            Ok(input.chars().take(max_chars).collect::<String>())
        })?,
    )?;
    api.set(
        "unix_time",
        lua.create_function(|_, ()| {
            use std::time::{SystemTime, UNIX_EPOCH};
            let seconds = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(MluaError::external)?
                .as_secs();
            Ok(seconds)
        })?,
    )?;
    globals.set("zuckerbot", api)?;

    let remaining = AtomicI64::new(limits.instruction_limit);
    let decrement = i64::from(limits.hook_granularity);
    lua.set_global_hook(
        HookTriggers::new().every_nth_instruction(limits.hook_granularity),
        move |_, _| {
            if remaining.fetch_sub(decrement, Ordering::Relaxed) <= decrement {
                Err(MluaError::runtime("Lua instruction limit exceeded"))
            } else {
                Ok(VmState::Continue)
            }
        },
    )?;

    Ok(lua)
}

fn validate_manifest(
    manifest: &LuaModuleManifest,
    display_name: &str,
) -> Result<(), LuaEngineError> {
    validate_identifier(&manifest.id, 64, "module id", display_name)?;
    if manifest.name.trim().is_empty() || manifest.name.chars().count() > 64 {
        return invalid(display_name, "module name must contain 1 to 64 characters");
    }
    if manifest.description.trim().is_empty() || manifest.description.chars().count() > 200 {
        return invalid(
            display_name,
            "module description must contain 1 to 200 characters",
        );
    }
    if manifest.category.trim().is_empty() || manifest.category.chars().count() > 32 {
        return invalid(
            display_name,
            "module category must contain 1 to 32 characters",
        );
    }

    for command in &manifest.commands {
        validate_identifier(&command.name, 32, "command name", display_name)?;
        let description_length = command.description.chars().count();
        if !(1..=100).contains(&description_length) {
            return invalid(
                display_name,
                "command descriptions must contain 1 to 100 characters",
            );
        }
        validate_options(&command.options, display_name)?;
    }

    for event in &manifest.events {
        validate_identifier(event, 64, "event name", display_name)?;
    }

    Ok(())
}

fn validate_options(
    options: &[LuaCommandOption],
    display_name: &str,
) -> Result<(), LuaEngineError> {
    if options.len() > MAX_OPTIONS_PER_LEVEL {
        return invalid(
            display_name,
            "a command or subcommand cannot define more than 25 options",
        );
    }

    for option in options {
        validate_identifier(&option.name, 32, "option name", display_name)?;
        let description_length = option.description.chars().count();
        if !(1..=100).contains(&description_length) {
            return invalid(
                display_name,
                "option descriptions must contain 1 to 100 characters",
            );
        }
        if option.choices.len() > 25 {
            return invalid(display_name, "an option cannot define more than 25 choices");
        }
        if matches!(
            option.kind,
            LuaOptionKind::Subcommand | LuaOptionKind::SubcommandGroup
        ) {
            validate_options(&option.options, display_name)?;
        } else if !option.options.is_empty() {
            return invalid(
                display_name,
                "only subcommands and subcommand groups can contain nested options",
            );
        }
    }

    Ok(())
}

fn validate_identifier(
    value: &str,
    max_len: usize,
    label: &str,
    display_name: &str,
) -> Result<(), LuaEngineError> {
    let length = value.len();
    let valid = (1..=max_len).contains(&length)
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-')
        });

    if valid {
        Ok(())
    } else {
        invalid(
            display_name,
            &format!(
                "{label} {value:?} must use 1 to {max_len} lowercase ASCII letters, digits, '_' or '-'"
            ),
        )
    }
}

fn invalid<T>(script: &str, message: &str) -> Result<T, LuaEngineError> {
    Err(LuaEngineError::InvalidModule {
        script: script.to_owned(),
        message: message.to_owned(),
    })
}
