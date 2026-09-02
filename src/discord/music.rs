use anyhow::{Context as _, Result, anyhow};
use serenity::{
    all::{GuildId, UserId},
    client::Context,
};
use songbird::input::YoutubeDl;
use url::Url;

use crate::{AppState, lua::MusicOperation};

pub async fn execute(
    ctx: &Context,
    state: &AppState,
    guild_id: GuildId,
    user_id: UserId,
    operation: MusicOperation,
    query: Option<&str>,
) -> Result<String> {
    let manager = songbird::get(ctx)
        .await
        .ok_or_else(|| anyhow!("Songbird voice manager is unavailable"))?
        .clone();

    if matches!(operation, MusicOperation::Leave) {
        if manager.get(guild_id).is_some() {
            manager.remove(guild_id).await?;
            return Ok("Rozłączono bota z kanałem głosowym.".to_owned());
        }
        return Ok("Bot nie jest połączony z kanałem głosowym.".to_owned());
    }

    let handler_lock = match manager.get(guild_id) {
        Some(handler) => handler,
        None => {
            let channel_id = user_voice_channel(ctx, guild_id, user_id)?;
            manager
                .join(guild_id, channel_id)
                .await
                .context("nie udało się dołączyć do kanału głosowego")?
        }
    };
    let mut handler = handler_lock.lock().await;

    match operation {
        MusicOperation::Play => {
            let query = query
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(|| anyhow!("brakuje tytułu albo adresu utworu"))?
                .trim();
            validate_source(state, query)?;

            let source = if query.starts_with("https://") {
                YoutubeDl::new(state.http_client.clone(), query.to_owned())
            } else {
                YoutubeDl::new_search(state.http_client.clone(), query.to_owned())
            };
            handler.enqueue_input(source.into()).await;
            Ok(format!(
                "Dodano do kolejki. Liczba utworów: {}.",
                handler.queue().len()
            ))
        }
        MusicOperation::Pause => {
            handler.queue().pause()?;
            Ok("Wstrzymano odtwarzanie.".to_owned())
        }
        MusicOperation::Resume => {
            handler.queue().resume()?;
            Ok("Wznowiono odtwarzanie.".to_owned())
        }
        MusicOperation::Skip => {
            handler.queue().skip()?;
            Ok("Pominięto bieżący utwór.".to_owned())
        }
        MusicOperation::Stop => {
            handler.queue().stop();
            Ok("Zatrzymano odtwarzanie i wyczyszczono kolejkę.".to_owned())
        }
        MusicOperation::Queue => Ok(format!(
            "W kolejce znajduje się {} utworów.",
            handler.queue().len()
        )),
        MusicOperation::Leave => unreachable!("leave is handled before taking the call lock"),
    }
}

fn user_voice_channel(
    ctx: &Context,
    guild_id: GuildId,
    user_id: UserId,
) -> Result<serenity::all::ChannelId> {
    let guild = ctx
        .cache
        .guild(guild_id)
        .ok_or_else(|| anyhow!("serwer nie znajduje się jeszcze w pamięci podręcznej"))?;

    guild
        .voice_states
        .get(&user_id)
        .and_then(|voice_state| voice_state.channel_id)
        .ok_or_else(|| anyhow!("najpierw dołącz do kanału głosowego"))
}

fn validate_source(state: &AppState, query: &str) -> Result<()> {
    if !query.contains("://") {
        return Ok(());
    }

    let url = Url::parse(query).context("nieprawidłowy adres źródła muzyki")?;
    if url.scheme() != "https" {
        return Err(anyhow!("adres źródła muzyki musi używać HTTPS"));
    }

    let host = url
        .host_str()
        .ok_or_else(|| anyhow!("adres źródła muzyki nie zawiera hosta"))?
        .to_ascii_lowercase();
    let allowed = state.config.music_allowed_hosts.iter().any(|candidate| {
        host == *candidate || host.ends_with(&format!(".{candidate}"))
    });
    if !allowed {
        return Err(anyhow!(
            "host {host} nie znajduje się na liście MUSIC_ALLOWED_HOSTS"
        ));
    }

    Ok(())
}
