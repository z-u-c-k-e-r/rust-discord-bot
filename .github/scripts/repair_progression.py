from pathlib import Path


def replace_once(path: str, old: str, new: str, marker: str) -> None:
    file = Path(path)
    content = file.read_text(encoding="utf-8")
    if old in content:
        file.write_text(content.replace(old, new, 1), encoding="utf-8")
        return
    if marker not in content:
        raise SystemExit(f"expected source fragment is missing in {path}")


replace_once(
    "src/discord/progression.rs",
    '''pub async fn execute(
    ctx: &Context,
    state: &AppState,
    module_id: &str,
    guild_id: Option<GuildId>,
    channel_id: Option<ChannelId>,
    actor_id: Option<UserId>,
    actor_permissions: Permissions,
    command_context: bool,
    operation: &ProgressionOperation,
) -> Result<Option<String>> {
''',
    '''#[derive(Clone, Copy)]
pub(super) struct ProgressionExecutionContext {
    guild_id: Option<GuildId>,
    channel_id: Option<ChannelId>,
    actor_id: Option<UserId>,
    actor_permissions: Permissions,
    command_context: bool,
}

impl ProgressionExecutionContext {
    pub(super) const fn new(
        guild_id: Option<GuildId>,
        channel_id: Option<ChannelId>,
        actor_id: Option<UserId>,
        actor_permissions: Permissions,
        command_context: bool,
    ) -> Self {
        Self {
            guild_id,
            channel_id,
            actor_id,
            actor_permissions,
            command_context,
        }
    }
}

pub async fn execute(
    ctx: &Context,
    state: &AppState,
    module_id: &str,
    execution: ProgressionExecutionContext,
    operation: &ProgressionOperation,
) -> Result<Option<String>> {
    let ProgressionExecutionContext {
        guild_id,
        channel_id,
        actor_id,
        actor_permissions,
        command_context,
    } = execution;
''',
    "pub(super) struct ProgressionExecutionContext",
)

replace_once(
    "src/discord/executor.rs",
    '''            progression::execute(
                ctx,
                state,
                module_id,
                origin.guild_id,
                origin.channel_id,
                origin.actor_id,
                origin.actor_permissions,
                origin.enforce_actor_permissions,
                operation,
            )
''',
    '''            progression::execute(
                ctx,
                state,
                module_id,
                progression::ProgressionExecutionContext::new(
                    origin.guild_id,
                    origin.channel_id,
                    origin.actor_id,
                    origin.actor_permissions,
                    origin.enforce_actor_permissions,
                ),
                operation,
            )
''',
    "progression::ProgressionExecutionContext::new",
)

replace_once(
    "src/storage/progression.rs",
    "    Completed(CoinTransfer),\n",
    "    Completed(Box<CoinTransfer>),\n",
    "Completed(Box<CoinTransfer>)",
)

replace_once(
    "src/storage/memory.rs",
    '''        CoinTransferOutcome::Completed(CoinTransfer {
            sender,
            recipient,
            amount,
        })
''',
    '''        CoinTransferOutcome::Completed(Box::new(CoinTransfer {
            sender,
            recipient,
            amount,
        }))
''',
    "CoinTransferOutcome::Completed(Box::new(CoinTransfer",
)

replace_once(
    "src/storage/postgres.rs",
    '''        Ok(CoinTransferOutcome::Completed(CoinTransfer {
            sender,
            recipient,
            amount,
        }))
''',
    '''        Ok(CoinTransferOutcome::Completed(Box::new(CoinTransfer {
            sender,
            recipient,
            amount,
        })))
''',
    "CoinTransferOutcome::Completed(Box::new(CoinTransfer",
)
