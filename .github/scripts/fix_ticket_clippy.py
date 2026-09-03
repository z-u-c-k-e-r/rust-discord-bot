from pathlib import Path


def replace_once(path: str, old: str, new: str) -> None:
    file = Path(path)
    content = file.read_text(encoding="utf-8")
    if old not in content:
        raise SystemExit(f"expected source fragment is missing in {path}: {old[:160]!r}")
    file.write_text(content.replace(old, new, 1), encoding="utf-8")


replace_once(
    "src/discord/tickets.rs",
    """        if let Some(log_channel) = log_channel
            && log_channel != channel_id
        {
            if let Err(error) = send_transcript(ctx, log_channel, &ticket, &transcript).await {
                tracing::warn!(ticket_id = %ticket.id, ?error, \"cannot send transcript to log channel\");
            }
        }
""",
    """        if let Some(log_channel) = log_channel
            && log_channel != channel_id
            && let Err(error) = send_transcript(ctx, log_channel, &ticket, &transcript).await
        {
            tracing::warn!(ticket_id = %ticket.id, ?error, \"cannot send transcript to log channel\");
        }
""",
)

replace_once(
    "src/tickets/memory.rs",
    "        events.sort_by(|left, right| right.id.cmp(&left.id));\n",
    "        events.sort_by_key(|event| std::cmp::Reverse(event.id));\n",
)
