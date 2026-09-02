use anyhow::{Context as _, Result, anyhow};
use serenity::{
    all::{
        Command, CommandOptionType, GuildId, InstallationContext, InteractionContext, Permissions,
    },
    builder::{CreateCommand, CreateCommandOption},
    client::Context,
};

use crate::{
    AppState,
    lua::{
        LuaCommandDefinition, LuaCommandOption, LuaInstallationContext, LuaInteractionContext,
        LuaOptionKind,
    },
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RegistrationScope {
    Global,
    Guild,
}

pub async fn register(ctx: &Context, state: &AppState) -> Result<usize> {
    let definitions = state.scripts.command_definitions().await;
    let scope = if state.config.discord_dev_guild_id.is_some() {
        RegistrationScope::Guild
    } else {
        RegistrationScope::Global
    };
    let commands = definitions
        .iter()
        .map(|definition| build_command(definition, scope))
        .collect::<Result<Vec<_>>>()?;
    let count = commands.len();

    if let Some(guild_id) = state.config.discord_dev_guild_id {
        GuildId::new(guild_id)
            .set_commands(&ctx.http, commands)
            .await
            .context("failed to register development-guild commands")?;
        tracing::info!(guild_id, count, "registered development guild commands");
    } else {
        Command::set_global_commands(&ctx.http, commands)
            .await
            .context("failed to register global commands")?;
        tracing::info!(count, "registered global commands");
    }

    Ok(count)
}

fn build_command(
    definition: &LuaCommandDefinition,
    scope: RegistrationScope,
) -> Result<CreateCommand> {
    definition
        .validate_contexts()
        .map_err(|message| anyhow!("command /{}: {message}", definition.name))?;

    let mut command = CreateCommand::new(&definition.name)
        .description(&definition.description)
        .nsfw(definition.nsfw);

    if scope == RegistrationScope::Global {
        command = command
            .integration_types(
                definition
                    .resolved_integration_types()
                    .into_iter()
                    .map(installation_context)
                    .collect(),
            )
            .contexts(
                definition
                    .resolved_contexts()
                    .into_iter()
                    .map(interaction_context)
                    .collect(),
            );
    }

    if let Some(bits) = &definition.default_member_permissions {
        let bits = bits.parse::<u64>().with_context(|| {
            format!(
                "command /{} has invalid default_member_permissions",
                definition.name
            )
        })?;
        command = command.default_member_permissions(Permissions::from_bits_truncate(bits));
    }

    for option in &definition.options {
        command = command.add_option(build_option(option)?);
    }

    Ok(command)
}

const fn installation_context(context: LuaInstallationContext) -> InstallationContext {
    match context {
        LuaInstallationContext::Guild => InstallationContext::Guild,
        LuaInstallationContext::User => InstallationContext::User,
    }
}

const fn interaction_context(context: LuaInteractionContext) -> InteractionContext {
    match context {
        LuaInteractionContext::Guild => InteractionContext::Guild,
        LuaInteractionContext::BotDm => InteractionContext::BotDm,
        LuaInteractionContext::PrivateChannel => InteractionContext::PrivateChannel,
    }
}

fn build_option(definition: &LuaCommandOption) -> Result<CreateCommandOption> {
    let option_type = option_type(definition.kind);
    let mut option =
        CreateCommandOption::new(option_type, &definition.name, &definition.description)
            .required(definition.required)
            .set_autocomplete(definition.autocomplete);

    if let Some(min_length) = definition.min_length {
        option = option.min_length(min_length);
    }
    if let Some(max_length) = definition.max_length {
        option = option.max_length(max_length);
    }

    match definition.kind {
        LuaOptionKind::Integer => {
            if let Some(value) = definition.min_value {
                option = option.min_int_value(non_negative_integer(value, "min_value")?);
            }
            if let Some(value) = definition.max_value {
                option = option.max_int_value(non_negative_integer(value, "max_value")?);
            }
        }
        LuaOptionKind::Number => {
            if let Some(value) = definition.min_value {
                option = option.min_number_value(value);
            }
            if let Some(value) = definition.max_value {
                option = option.max_number_value(value);
            }
        }
        _ => {}
    }

    for choice in &definition.choices {
        option = match definition.kind {
            LuaOptionKind::String => option.add_string_choice(
                &choice.name,
                choice
                    .value
                    .as_str()
                    .ok_or_else(|| anyhow!("string choice {} must be a string", choice.name))?,
            ),
            LuaOptionKind::Integer => {
                let value = choice
                    .value
                    .as_i64()
                    .ok_or_else(|| anyhow!("integer choice {} must be an integer", choice.name))?;
                let value = i32::try_from(value).with_context(|| {
                    format!("integer choice {} is outside Discord's range", choice.name)
                })?;
                option.add_int_choice(&choice.name, value)
            }
            LuaOptionKind::Number => option.add_number_choice(
                &choice.name,
                choice
                    .value
                    .as_f64()
                    .ok_or_else(|| anyhow!("number choice {} must be numeric", choice.name))?,
            ),
            _ => {
                return Err(anyhow!(
                    "choices are only valid for string, integer and number options"
                ));
            }
        };
    }

    for child in &definition.options {
        option = option.add_sub_option(build_option(child)?);
    }

    Ok(option)
}

const fn option_type(kind: LuaOptionKind) -> CommandOptionType {
    match kind {
        LuaOptionKind::String => CommandOptionType::String,
        LuaOptionKind::Integer => CommandOptionType::Integer,
        LuaOptionKind::Number => CommandOptionType::Number,
        LuaOptionKind::Boolean => CommandOptionType::Boolean,
        LuaOptionKind::User => CommandOptionType::User,
        LuaOptionKind::Channel => CommandOptionType::Channel,
        LuaOptionKind::Role => CommandOptionType::Role,
        LuaOptionKind::Mentionable => CommandOptionType::Mentionable,
        LuaOptionKind::Attachment => CommandOptionType::Attachment,
        LuaOptionKind::Subcommand => CommandOptionType::SubCommand,
        LuaOptionKind::SubcommandGroup => CommandOptionType::SubCommandGroup,
    }
}

fn non_negative_integer(value: f64, name: &str) -> Result<u64> {
    if value < 0.0 || value.fract() != 0.0 {
        return Err(anyhow!("{name} must be a non-negative integer"));
    }
    if value > u64::MAX as f64 {
        return Err(anyhow!("{name} is too large"));
    }
    Ok(value as u64)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{RegistrationScope, build_command};
    use crate::lua::LuaCommandDefinition;

    fn definition(value: serde_json::Value) -> LuaCommandDefinition {
        serde_json::from_value(value).expect("command definition should deserialize")
    }

    #[test]
    fn global_command_uses_current_installation_and_interaction_contexts() {
        let definition = definition(json!({
            "name": "hello",
            "description": "Says hello.",
            "integration_types": ["guild", "user"],
            "contexts": ["guild", "bot_dm", "private_channel"],
            "nsfw": true
        }));

        let payload = serde_json::to_value(
            build_command(&definition, RegistrationScope::Global)
                .expect("global command should build"),
        )
        .expect("command should serialize");

        assert_eq!(
            payload["integration_types"]
                .as_array()
                .expect("integration_types array")
                .len(),
            2
        );
        assert_eq!(
            payload["contexts"]
                .as_array()
                .expect("contexts array")
                .len(),
            3
        );
        assert_eq!(payload["nsfw"], json!(true));
        assert!(payload.get("dm_permission").is_none());
    }

    #[test]
    fn guild_command_omits_global_only_context_fields() {
        let definition = definition(json!({
            "name": "hello",
            "description": "Says hello.",
            "integration_types": ["guild"],
            "contexts": ["guild"]
        }));

        let payload = serde_json::to_value(
            build_command(&definition, RegistrationScope::Guild)
                .expect("guild command should build"),
        )
        .expect("command should serialize");

        assert!(payload.get("integration_types").is_none());
        assert!(payload.get("contexts").is_none());
        assert!(payload.get("dm_permission").is_none());
    }

    #[test]
    fn legacy_dm_permission_is_migrated_without_using_the_deprecated_payload_field() {
        let definition = definition(json!({
            "name": "hello",
            "description": "Says hello.",
            "dm_permission": true
        }));

        let payload = serde_json::to_value(
            build_command(&definition, RegistrationScope::Global)
                .expect("legacy command should build"),
        )
        .expect("command should serialize");

        assert_eq!(
            payload["contexts"]
                .as_array()
                .expect("contexts array")
                .len(),
            2
        );
        assert!(payload.get("dm_permission").is_none());
    }
}
