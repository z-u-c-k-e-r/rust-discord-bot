mod commands;
#[expect(
    clippy::collapsible_if,
    reason = "the nested actor/owner hierarchy guard is intentionally explicit until the permission engine is extracted"
)]
mod executor;
#[expect(
    clippy::result_large_err,
    reason = "the private Discord response helper currently mirrors Serenity's public error type"
)]
mod handler;
mod music;
