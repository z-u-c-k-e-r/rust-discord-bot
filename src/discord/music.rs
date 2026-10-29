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
            if ipv6.is_loopback() {
                return true;
            }
            let segments = ipv6.segments();
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

        // IPv6 Unique Local and Link-Local
        assert!(is_blocked_ip(IpAddr::V6("fc00::1".parse().unwrap())));
        assert!(is_blocked_ip(IpAddr::V6("fe80::1".parse().unwrap())));

        // Public internet IPs (must NOT be blocked)
        assert!(!is_blocked_ip(IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8))));
        assert!(!is_blocked_ip(IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1))));
        assert!(!is_blocked_ip(IpAddr::V6(
            "2001:4860:4860::8888".parse().unwrap()
        )));
    }
}
