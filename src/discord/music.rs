use anyhow::{Context as _, Result, anyhow};
use serenity::{
    all::{GuildId, UserId},
    client::Context,
};
use songbird::input::YoutubeDl;
use url::Url;

use crate::{AppState, lua::MusicOperation};

pub const MAX_QUEUE_SIZE: usize = 100;

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

    let existing_handler = manager.get(guild_id);

    if existing_handler.is_none() {
        match operation {
            MusicOperation::Leave => {
                return Ok("Bot nie jest połączony z kanałem głosowym.".to_owned());
            }
            MusicOperation::Queue => {
                return Ok(
                    "Kolejka jest pusta. Bot nie jest połączony z kanałem głosowym.".to_owned(),
                );
            }
            MusicOperation::Play => {
                // Play will connect below
            }
            _ => {
                return Err(anyhow!("Bot nie jest połączony z kanałem głosowym."));
            }
        }
    }

    let handler_lock = match existing_handler {
        Some(handler) => {
            // If bot is connected, control actions require user to be in the same voice channel
            if !matches!(operation, MusicOperation::Queue) {
                let user_channel = user_voice_channel(ctx, guild_id, user_id)?;
                let bot_channel = {
                    let h = handler.lock().await;
                    h.current_channel()
                        .map(|ch| serenity::all::ChannelId::new(ch.0.get()))
                        .or_else(|| {
                            let bot_id = ctx.cache.current_user().id;
                            ctx.cache.guild(guild_id).and_then(|g| {
                                g.voice_states.get(&bot_id).and_then(|vs| vs.channel_id)
                            })
                        })
                };
                if let Some(bot_channel) = bot_channel
                    && user_channel != bot_channel
                {
                    return Err(anyhow!(
                        "musisz znajdować się na tym samym kanale głosowym co bot"
                    ));
                }
            }

            if matches!(operation, MusicOperation::Leave) {
                manager.remove(guild_id).await?;
                return Ok("Rozłączono bota z kanałem głosowym.".to_owned());
            }

            handler
        }
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

            if handler.queue().len() >= MAX_QUEUE_SIZE {
                return Err(anyhow!(
                    "kolejka odtwarzania osiągnęła maksymalny limit {MAX_QUEUE_SIZE} utworów"
                ));
            }

            let source = if query.starts_with("https://") {
                YoutubeDl::new(state.media_http_client.clone(), query.to_owned())
            } else {
                YoutubeDl::new_search(state.media_http_client.clone(), query.to_owned())
            };
            handler.enqueue_input(source.into()).await;
            Ok(format!(
                "Dodano do kolejki. Liczba utworów: {}.",
                handler.queue().len()
            ))
        }
        MusicOperation::Volume => {
            let level = query
                .and_then(|q| q.trim().parse::<f32>().ok())
                .filter(|&v| (0.0..=150.0).contains(&v))
                .ok_or_else(|| {
                    anyhow!("poziom głośności musi być liczbą w zakresie od 0 do 150")
                })?;
            let factor = level / 100.0;
            if let Some(track) = handler.queue().current() {
                let _ = track.set_volume(factor);
            }
            Ok(format!("Ustawiono głośność odtwarzacza na {level:.0}%."))
        }
        MusicOperation::Loop => {
            if let Some(track) = handler.queue().current() {
                let info = track.get_info().await?;
                let enabled = match info.loops {
                    songbird::tracks::LoopState::Infinite => {
                        track.disable_loop()?;
                        false
                    }
                    _ => {
                        track.enable_loop()?;
                        true
                    }
                };
                Ok(if enabled {
                    "Włączono zapętlanie bieżącego utworu.".to_owned()
                } else {
                    "Wyłączono zapętlanie bieżącego utworu.".to_owned()
                })
            } else {
                Err(anyhow!("brak aktywnego utworu do zapętlenia"))
            }
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

pub fn is_blocked_ip(ip: std::net::IpAddr) -> bool {
    match ip {
        std::net::IpAddr::V4(ipv4) => {
            let octets = ipv4.octets();
            // 0.0.0.0/8
            if octets[0] == 0 {
                return true;
            }
            // 127.0.0.0/8 (loopback)
            if ipv4.is_loopback() {
                return true;
            }
            // 10.0.0.0/8 (private)
            if octets[0] == 10 {
                return true;
            }
            // 172.16.0.0/12 (private: 172.16.0.0 - 172.31.255.255)
            if octets[0] == 172 && (16..=31).contains(&octets[1]) {
                return true;
            }
            // 192.168.0.0/16 (private)
            if octets[0] == 192 && octets[1] == 168 {
                return true;
            }
            // 169.254.0.0/16 (link-local and cloud metadata like 169.254.169.254)
            if ipv4.is_link_local() || (octets[0] == 169 && octets[1] == 254) {
                return true;
            }
            // 100.64.0.0/10 (carrier-grade NAT)
            if octets[0] == 100 && (64..=127).contains(&octets[1]) {
                return true;
            }
            // 192.0.0.0/24 (IETF protocol assignments)
            if octets[0] == 192 && octets[1] == 0 && octets[2] == 0 {
                return true;
            }
            // Multicast 224.0.0.0/4 and Reserved 240.0.0.0/4
            if ipv4.is_multicast() || octets[0] >= 240 {
                return true;
            }
            // Broadcast 255.255.255.255
            if ipv4.is_broadcast() {
                return true;
            }
            false
        }
        std::net::IpAddr::V6(ipv6) => {
            if ipv6.is_loopback() || ipv6.is_unspecified() || ipv6.is_multicast() {
                return true;
            }
            let segments = ipv6.segments();
            // 2001:db8::/32 (Documentation)
            if segments[0] == 0x2001 && segments[1] == 0x0db8 {
                return true;
            }
            // fc00::/7 (Unique Local Address)
            if (segments[0] & 0xfe00) == 0xfc00 {
                return true;
            }
            // fe80::/10 (Link-Local Unicast)
            if (segments[0] & 0xffc0) == 0xfe80 {
                return true;
            }
            // IPv4-mapped IPv6 address (::ffff:a.b.c.d)
            if let Some(ipv4) = ipv6.to_ipv4_mapped() {
                return is_blocked_ip(std::net::IpAddr::V4(ipv4));
            }
            false
        }
    }
}

pub fn validate_source(state: &AppState, query: &str) -> Result<()> {
    if query.chars().any(char::is_control) {
        return Err(anyhow!("zapytanie zawiera niedozwolone znaki kontrolne"));
    }
    if query.trim_start().starts_with('-') {
        return Err(anyhow!("zapytanie nie może zaczynać się od myślnika"));
    }

    if !query.contains("://") {
        if query.contains(':')
            && !query.starts_with("https://")
            && let Ok(parsed) = Url::parse(query)
            && parsed.scheme() != "https"
        {
            return Err(anyhow!("adres źródła muzyki musi używać HTTPS"));
        }
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

    // Check if the host is an IP literal targeting blocked ranges
    if let Ok(ip) = host.parse::<std::net::IpAddr>()
        && is_blocked_ip(ip)
    {
        return Err(anyhow!(
            "adres źródła muzyki wskazuje na zablokowany prywatny adres IP: {host}"
        ));
    }

    let allowed = state
        .config
        .music_allowed_hosts
        .iter()
        .any(|candidate| host == *candidate || host.ends_with(&format!(".{candidate}")));
    if !allowed {
        return Err(anyhow!(
            "host {host} nie znajduje się na liście MUSIC_ALLOWED_HOSTS"
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

    #[test]
    fn test_is_blocked_ip_detects_private_loopback_and_metadata() {
        // Loopback
        assert!(is_blocked_ip(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))));
        assert!(is_blocked_ip(IpAddr::V4(Ipv4Addr::new(127, 255, 255, 255))));
        assert!(is_blocked_ip(IpAddr::V6(Ipv6Addr::LOCALHOST)));

        // RFC 1918 Private ranges
        assert!(is_blocked_ip(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1))));
        assert!(is_blocked_ip(IpAddr::V4(Ipv4Addr::new(172, 16, 0, 1))));
        assert!(is_blocked_ip(IpAddr::V4(Ipv4Addr::new(172, 31, 255, 255))));
        assert!(is_blocked_ip(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1))));

        // Link-local & cloud metadata
        assert!(is_blocked_ip(IpAddr::V4(Ipv4Addr::new(169, 254, 169, 254))));
        assert!(is_blocked_ip(IpAddr::V4(Ipv4Addr::new(169, 254, 1, 1))));

        // IPv6 Unique Local, Link-Local, Unspecified and Documentation
        assert!(is_blocked_ip(IpAddr::V6(Ipv6Addr::UNSPECIFIED)));
        assert!(is_blocked_ip(IpAddr::V6("fc00::1".parse().unwrap())));
        assert!(is_blocked_ip(IpAddr::V6("fe80::1".parse().unwrap())));
        assert!(is_blocked_ip(IpAddr::V6("2001:db8::1".parse().unwrap())));

        // Public internet IPs (must NOT be blocked)
        assert!(!is_blocked_ip(IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8))));
        assert!(!is_blocked_ip(IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1))));
        assert!(!is_blocked_ip(IpAddr::V6(
            "2001:4860:4860::8888".parse().unwrap()
        )));
    }
}
