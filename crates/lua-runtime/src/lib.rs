use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

use mlua::{Function, HookTriggers, Lua, LuaSerdeExt, RegistryKey, Table, Value, VmState};
use thiserror::Error;
use zuckerbot_core::{
    CommandContext, CommandResponse, CommandSpec, PluginMetadata, ValidationError,
};

const DEFAULT_HOOK_INTERVAL: u64 = 10_000;

#[derive(Clone, Copy, Debug)]
pub struct LuaLimits {
    pub memory_bytes: usize,
    pub instructions_per_call: u64,
}

impl Default for LuaLimits {
    fn default() -> Self {
        Self {
            memory_bytes: 16 * 1024 * 1024,
            instructions_per_call: 1_000_000,
        }
    }
}

#[derive(Debug, Error)]
pub enum LuaRuntimeError {
    #[error("failed to read Lua plugin: {0}")]
    Io(#[from] std::io::Error),
    #[error("Lua execution failed: {0}")]
    Lua(#[from] mlua::Error),
    #[error("invalid Lua runtime limits: {0}")]
    InvalidLimits(&'static str),
    #[error("plugin metadata is invalid: {0}")]
    InvalidPlugin(String),
    #[error("plugin `{0}` is loaded more than once")]
    DuplicatePlugin(String),
    #[error("command `{0}` is loaded more than once")]
    DuplicateCommand(String),
    #[error("command `{0}` does not exist")]
    UnknownCommand(String),
    #[error("command contract is invalid: {0}")]
    Validation(#[from] ValidationError),
}

pub struct LuaRuntime {
    lua: Lua,
    handlers: HashMap<String, RegistryKey>,
    commands: Vec<CommandSpec>,
    plugins: Vec<PluginMetadata>,
    instruction_counter: Arc<AtomicU64>,
}

impl LuaRuntime {
    pub fn new(limits: LuaLimits) -> Result<Self, LuaRuntimeError> {
        if limits.memory_bytes == 0 {
            return Err(LuaRuntimeError::InvalidLimits(
                "memory limit must be greater than zero",
            ));
        }
        if limits.instructions_per_call == 0 {
            return Err(LuaRuntimeError::InvalidLimits(
                "instruction limit must be greater than zero",
            ));
        }

        let lua = Lua::new();
        lua.set_memory_limit(limits.memory_bytes)?;
        remove_dangerous_globals(&lua)?;

        let instruction_counter = Arc::new(AtomicU64::new(0));
        let hook_counter = Arc::clone(&instruction_counter);
        let instruction_limit = limits.instructions_per_call;
        let hook_interval = instruction_limit.min(DEFAULT_HOOK_INTERVAL) as u32;

        lua.set_hook(
            HookTriggers::new().every_nth_instruction(hook_interval),
            move |_, _| {
                let executed = hook_counter.fetch_add(
                    u64::from(hook_interval),
                    Ordering::Relaxed,
                ) + u64::from(hook_interval);

                if executed > instruction_limit {
                    return Err(mlua::Error::RuntimeError(format!(
                        "Lua instruction budget exceeded ({instruction_limit})"
                    )));
                }

                Ok(VmState::Continue)
            },
        )?;

        Ok(Self {
            lua,
            handlers: HashMap::new(),
            commands: Vec::new(),
            plugins: Vec::new(),
            instruction_counter,
        })
    }

    pub fn from_directory(
        directory: impl AsRef<Path>,
        limits: LuaLimits,
    ) -> Result<Self, LuaRuntimeError> {
        let mut runtime = Self::new(limits)?;
        runtime.load_directory(directory)?;
        Ok(runtime)
    }

    pub fn load_directory(
        &mut self,
        directory: impl AsRef<Path>,
    ) -> Result<usize, LuaRuntimeError> {
        let plugin_files = discover_plugin_files(directory.as_ref())?;
        for plugin_file in &plugin_files {
            let source = fs::read_to_string(plugin_file)?;
            self.load_plugin_source(&plugin_file.display().to_string(), &source)?;
        }

        Ok(plugin_files.len())
    }

    pub fn load_plugin_source(
        &mut self,
        chunk_name: &str,
        source: &str,
    ) -> Result<(), LuaRuntimeError> {
        self.reset_instruction_counter();
        let plugin_table: Table = self.lua.load(source).set_name(chunk_name).eval()?;
        let metadata = parse_metadata(&plugin_table)?;
        validate_plugin_metadata(&metadata)?;

        if self.plugins.iter().any(|plugin| plugin.name == metadata.name) {
            return Err(LuaRuntimeError::DuplicatePlugin(metadata.name));
        }

        let command_tables: Table = plugin_table.get("commands")?;
        let mut pending_commands = Vec::new();
        let mut pending_handlers = Vec::new();
        let mut names_in_plugin = HashSet::new();

        for command_table in command_tables.sequence_values::<Table>() {
            let command_table = command_table?;
            let dm_permission: Option<bool> = command_table.get("dm_permission")?;
            let command = CommandSpec {
                name: command_table.get("name")?,
                description: command_table.get("description")?,
                dm_permission: dm_permission.unwrap_or(true),
            };
            command.validate()?;

            if self.handlers.contains_key(&command.name)
                || !names_in_plugin.insert(command.name.clone())
            {
                return Err(LuaRuntimeError::DuplicateCommand(command.name));
            }

            let handler: Function = command_table.get("handler")?;
            pending_commands.push(command);
            pending_handlers.push(handler);
        }

        for (command, handler) in pending_commands.into_iter().zip(pending_handlers) {
            let command_name = command.name.clone();
            let registry_key = self.lua.create_registry_value(handler)?;
            self.commands.push(command);
            self.handlers.insert(command_name, registry_key);
        }

        self.plugins.push(metadata);
        self.commands.sort_by(|left, right| left.name.cmp(&right.name));
        self.plugins.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(())
    }

    pub fn command_specs(&self) -> &[CommandSpec] {
        &self.commands
    }

    pub fn plugin_metadata(&self) -> &[PluginMetadata] {
        &self.plugins
    }

    pub fn execute(
        &self,
        command_name: &str,
        context: &CommandContext,
    ) -> Result<CommandResponse, LuaRuntimeError> {
        let registry_key = self
            .handlers
            .get(command_name)
            .ok_or_else(|| LuaRuntimeError::UnknownCommand(command_name.to_owned()))?;

        self.reset_instruction_counter();
        let handler: Function = self.lua.registry_value(registry_key)?;
        let lua_context = self.lua.to_value(context)?;
        let response_value: Value = handler.call(lua_context)?;
        let response: CommandResponse = self.lua.from_value(response_value)?;
        response.validate()?;
        Ok(response)
    }

    fn reset_instruction_counter(&self) {
        self.instruction_counter.store(0, Ordering::Relaxed);
    }
}

fn remove_dangerous_globals(lua: &Lua) -> Result<(), mlua::Error> {
    let globals = lua.globals();
    for name in [
        "collectgarbage",
        "debug",
        "dofile",
        "io",
        "load",
        "loadfile",
        "os",
        "package",
        "require",
    ] {
        globals.set(name, Value::Nil)?;
    }
    Ok(())
}

fn parse_metadata(plugin_table: &Table) -> Result<PluginMetadata, mlua::Error> {
    let metadata_table: Table = plugin_table.get("metadata")?;
    Ok(PluginMetadata {
        name: metadata_table.get("name")?,
        version: metadata_table.get("version")?,
        description: metadata_table.get("description")?,
    })
}

fn validate_plugin_metadata(metadata: &PluginMetadata) -> Result<(), LuaRuntimeError> {
    if metadata.name.is_empty() || metadata.name.chars().count() > 64 {
        return Err(LuaRuntimeError::InvalidPlugin(
            "name must contain between 1 and 64 characters".to_owned(),
        ));
    }
    if !metadata.name.chars().all(|character| {
        character.is_ascii_lowercase()
            || character.is_ascii_digit()
            || matches!(character, '-' | '_')
    }) {
        return Err(LuaRuntimeError::InvalidPlugin(
            "name may contain only lowercase letters, digits, '-' and '_'".to_owned(),
        ));
    }
    if metadata.version.is_empty() || metadata.version.chars().count() > 32 {
        return Err(LuaRuntimeError::InvalidPlugin(
            "version must contain between 1 and 32 characters".to_owned(),
        ));
    }
    if metadata.description.is_empty() || metadata.description.chars().count() > 200 {
        return Err(LuaRuntimeError::InvalidPlugin(
            "description must contain between 1 and 200 characters".to_owned(),
        ));
    }
    Ok(())
}

fn discover_plugin_files(directory: &Path) -> Result<Vec<PathBuf>, std::io::Error> {
    let mut plugin_files = Vec::new();

    for entry in fs::read_dir(directory)? {
        let path = entry?.path();
        if path.is_file() && path.extension().is_some_and(|extension| extension == "lua") {
            plugin_files.push(path);
            continue;
        }

        if path.is_dir() {
            let entrypoint = path.join("main.lua");
            if entrypoint.is_file() {
                plugin_files.push(entrypoint);
            }
        }
    }

    plugin_files.sort();
    Ok(plugin_files)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_PLUGIN: &str = r#"
return {
  metadata = {
    name = "test",
    version = "0.1.0",
    description = "Test plugin"
  },
  commands = {
    {
      name = "ping",
      description = "Checks the Lua runtime.",
      handler = function(ctx)
        return {
          content = "Pong for " .. ctx.user_name,
          ephemeral = true
        }
      end
    }
  }
}
"#;

    fn context() -> CommandContext {
        CommandContext {
            command_name: "ping".to_owned(),
            interaction_id: "1".to_owned(),
            user_id: "2".to_owned(),
            user_name: "Adrian".to_owned(),
            guild_id: Some("3".to_owned()),
            channel_id: "4".to_owned(),
            locale: "pl".to_owned(),
            options: serde_json::Value::Null,
        }
    }

    #[test]
    fn loads_and_executes_a_lua_command() {
        let mut runtime = LuaRuntime::new(LuaLimits::default()).unwrap();
        runtime.load_plugin_source("test.lua", TEST_PLUGIN).unwrap();

        let response = runtime.execute("ping", &context()).unwrap();
        assert_eq!(response.content, "Pong for Adrian");
        assert!(response.ephemeral);
    }

    #[test]
    fn blocks_duplicate_commands() {
        let mut runtime = LuaRuntime::new(LuaLimits::default()).unwrap();
        runtime.load_plugin_source("test.lua", TEST_PLUGIN).unwrap();

        let duplicate = TEST_PLUGIN.replace("name = \"test\"", "name = \"test-two\"");
        let error = runtime
            .load_plugin_source("duplicate.lua", &duplicate)
            .unwrap_err();

        assert!(matches!(error, LuaRuntimeError::DuplicateCommand(command) if command == "ping"));
    }

    #[test]
    fn removes_process_and_filesystem_libraries() {
        let source = r#"
return {
  metadata = { name = "sandbox", version = "0.1.0", description = "Sandbox test" },
  commands = {
    {
      name = "sandbox",
      description = "Checks unavailable libraries.",
      handler = function(_)
        local safe = os == nil and io == nil and package == nil and require == nil
        return { content = safe and "safe" or "unsafe" }
      end
    }
  }
}
"#;
        let mut runtime = LuaRuntime::new(LuaLimits::default()).unwrap();
        runtime.load_plugin_source("sandbox.lua", source).unwrap();

        assert_eq!(runtime.execute("sandbox", &context()).unwrap().content, "safe");
    }

    #[test]
    fn interrupts_runaway_scripts() {
        let source = r#"
return {
  metadata = { name = "runaway", version = "0.1.0", description = "Limit test" },
  commands = {
    {
      name = "runaway",
      description = "Runs forever without the instruction limit.",
      handler = function(_)
        while true do end
      end
    }
  }
}
"#;
        let mut runtime = LuaRuntime::new(LuaLimits {
            memory_bytes: 4 * 1024 * 1024,
            instructions_per_call: 5_000,
        })
        .unwrap();
        runtime.load_plugin_source("runaway.lua", source).unwrap();

        let error = runtime.execute("runaway", &context()).unwrap_err();
        assert!(error.to_string().contains("instruction budget exceeded"));
    }
}
