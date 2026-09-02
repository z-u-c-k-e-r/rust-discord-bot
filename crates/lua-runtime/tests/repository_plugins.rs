use std::path::Path;

use zuckerbot_lua::{LuaLimits, LuaRuntime};

#[test]
fn loads_repository_plugins_and_their_command_contracts() {
    let plugins = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins");
    let runtime = LuaRuntime::from_directory(plugins, LuaLimits::default()).unwrap();
    let commands = runtime
        .command_specs()
        .iter()
        .map(|command| command.name.as_str())
        .collect::<Vec<_>>();

    assert_eq!(commands, ["about", "meme", "ping"]);
}
