#![cfg_attr(doc, doc = include_str!("../README.md"))]

pub mod bang;
pub mod cli;
pub mod config;

use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::LazyLock;
use std::time::{Duration, Instant};

use memchr::memchr;
use parking_lot::RwLock;
use tokio::time::interval;
use tracing::{debug, error, info};

use crate::bang::Bang;
use crate::config::AppConfig;

pub static BANG_CACHE: LazyLock<RwLock<HashMap<String, String>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));
static LAST_UPDATE: LazyLock<RwLock<Instant>> = LazyLock::new(|| RwLock::new(Instant::now()));

/// Get the bang command from the query.
/// this is the first '!' that is not preceded by a non-space character and
/// followed by a space.
#[must_use]
pub fn get_bang(query: &str) -> Option<&str> {
    let bytes = query.as_bytes();
    let len = bytes.len();

    // Fast path for short queries
    if len < 2 {
        return None;
    }

    // Check for bang at start (common case)
    if bytes[0] == b'!' {
        let mut end = 1;
        while end < len && bytes[end] != b' ' {
            end += 1;
        }
        // Valid bang needs at least one character after '!'
        if end > 1 {
            return Some(&query[0..end]);
        }
    }

    let mut offset = 0;

    while let Some(pos) = memchr(b'!', &bytes[offset..]) {
        let i = offset + pos;
        // must be preceded by a space
        if i > 0 && bytes[i - 1] == b' ' {
            // skip if nothing or space right after '!'
            if i + 1 < len && bytes[i + 1] != b' ' {
                // find next space (or end of slice)
                let end = memchr(b' ', &bytes[i + 1..]).map_or(len, |e| i + 1 + e);
                return Some(&query[i..end]);
            }
        }
        offset = i + 1;
    }

    None
}

#[must_use]
pub fn resolve(app_config: &AppConfig, query: &str) -> String {
    if query.is_empty() {
        return app_config.default_search.replace("{}", "");
    }

    let bytes = query.as_bytes();

    // Fastest path for most common case - single-word plain queries
    if bytes[0] != b'!' {
        // Quick check for spaces
        let mut has_space = false;
        for &b in bytes {
            if b == b' ' {
                has_space = true;
                break;
            }
        }

        // No spaces found, so there can't be any bang after the first word
        if !has_space {
            return app_config
                .default_search
                .replace("{}", &urlencoding::encode(query));
        }
    }

    if let Some(bang) = get_bang(query) {
        let cache = BANG_CACHE.read();
        let key_lower = bang[1..].to_ascii_lowercase();

        if let Some(url_template) = cache.get(&key_lower) {
            let replaced = query.replacen(bang, "", 1);
            let search_term = replaced.trim();
            let mut encoded_term = urlencoding::encode(search_term);

            // Fix slashes once in the encoded term
            if encoded_term.contains("%2F") {
                encoded_term = Cow::from(encoded_term.replace("%2F", "/"));
            }

            // Template handling
            if url_template.contains("{{{s}}}") {
                let result = url_template.replace("{{{s}}}", &encoded_term);
                if encoded_term.contains("%2F") {
                    return result.replace("%2F", "/");
                }
                return result;
            }

            // Simple append case
            let mut result = String::with_capacity(url_template.len() + encoded_term.len());
            result.push_str(url_template);
            result.push_str(&encoded_term);
            return result;
        }
    }

    // Default fallback
    app_config
        .default_search
        .replace("{}", &urlencoding::encode(query))
}

pub async fn periodic_update(app_config: AppConfig) {
    const NORMAL_INTERVAL_SECS: u64 = 12 * 60 * 60;
    let mut update_interval = interval(Duration::from_secs(NORMAL_INTERVAL_SECS));
    let mut backoff_minutes: u8 = 0;

    #[expect(
        clippy::infinite_loop,
        reason = "This is intended to run indefinitely in the background"
    )]
    loop {
        update_interval.tick().await;

        match update_bangs(&app_config).await {
            Ok(()) => {
                info!("Updated bang commands successfully");
                if backoff_minutes != 0 {
                    backoff_minutes = 0;
                    update_interval = interval(Duration::from_secs(NORMAL_INTERVAL_SECS));
                }
            },
            Err(e) => {
                error!("Failed to update bang commands: {e}");
                backoff_minutes = (backoff_minutes + 1).min(30);
                update_interval = interval(Duration::from_mins(u64::from(backoff_minutes)));
                // first tick is always immediately
                update_interval.tick().await;
            },
        }
    }
}

/// Update the bang cache with the latest bang commands.
///
/// # Errors
/// If it fails to update the bang cache.
pub async fn update_bangs(app_config: &AppConfig) -> anyhow::Result<()> {
    let cache_path = std::env::temp_dir().join("bang-cache.json");
    let cache_age_limit = Duration::from_secs(24 * 60 * 60);

    if let Ok(metadata) = std::fs::metadata(&cache_path)
        && let Ok(modified) = metadata.modified()
        && modified.elapsed()? < cache_age_limit
        && let Ok(contents) = std::fs::read_to_string(&cache_path)
    {
        let bang_entries: Vec<Bang> = serde_json::from_str(&contents)?;
        debug!("Bang cache is up to date.");
        update_cache(bang_entries, app_config);
        return Ok(());
    }

    let response = reqwest::get(&app_config.bangs_url).await?.text().await?;
    let bang_entries: Vec<Bang> = serde_json::from_str(&response)?;

    std::fs::write(cache_path, &response)?;
    update_cache(bang_entries, app_config);
    Ok(())
}

/// Update the bang cache with the provided bang commands.
///
/// # Errors
/// If it fails to get the write lock on the bang cache or the last update time.
fn update_cache(bang_entries: Vec<Bang>, app_config: &AppConfig) {
    let mut cache = BANG_CACHE.write();
    cache.clear();
    for bang in bang_entries {
        cache.insert(bang.trigger.clone(), bang.url_template.clone());
    }
    if let Some(bangs) = &app_config.bangs {
        for bang in bangs {
            cache.insert(bang.trigger.clone(), bang.url_template.clone());
        }
    }
    drop(cache);
    *LAST_UPDATE.write() = Instant::now();
    debug!("Bang commands updated successfully.");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_bang() {
        // Valid bang queries
        assert_eq!(get_bang("!gh search term"), Some("!gh"));
        assert_eq!(get_bang("search !gh term"), Some("!gh"));
        assert_eq!(get_bang("!gh"), Some("!gh"));
        assert_eq!(get_bang("!multi-word"), Some("!multi-word"));
        assert_eq!(get_bang("  !gh search"), Some("!gh"));
        assert_eq!(get_bang("!g rust programming"), Some("!g"));

        // Invalid bang queries
        assert_eq!(get_bang("search!gh term"), None); // No space before !
        assert_eq!(get_bang("search! gh term"), None); // Space after !
        assert_eq!(get_bang("!"), None); // Single ! is not a bang
        assert_eq!(get_bang(""), None); // Empty string
        assert_eq!(get_bang("no bang here"), None); // No bang
        assert_eq!(get_bang("a!!gh"), None); // No space before !
    }

    #[tokio::test]
    async fn test_resolve_with_bang() {
        let config = AppConfig::default();
        if let Err(e) = update_bangs(&config).await {
            panic!("Failed to update bangs: {e}");
        }

        // Test with template that has {{{s}}}
        let result = resolve(&config, "!g rust programming");
        assert_eq!(result, "https://www.google.com/search?q=rust%20programming");

        // Test with template that doesn't have {{{s}}}
        let result = resolve(&config, "!gh rust programming");
        assert_eq!(
            result,
            "https://github.com/search?utf8=%E2%9C%93&q=rust%20programming"
        );

        // Test with bang at different position
        let result = resolve(&config, "rust !yt programming");
        assert_eq!(
            result,
            "https://www.youtube.com/results?search_query=rust%20%20programming"
        );
    }

    #[tokio::test]
    async fn test_resolve_without_bang() {
        let config = AppConfig::default();

        // Test with no bang
        let result = resolve(&config, "rust programming");
        assert_eq!(
            result,
            config.default_search.replace("{}", "rust%20programming")
        );

        // Test with non-matching bang
        let result = resolve(&config, "!nonexistent rust programming");
        assert_eq!(
            result,
            config
                .default_search
                .replace("{}", "%21nonexistent%20rust%20programming")
        );
    }

    #[tokio::test]
    async fn test_resolve_edge_cases() {
        let config = AppConfig::default();

        if let Err(e) = update_bangs(&config).await {
            panic!("Failed to update bangs: {e}");
        }

        // Empty query
        let result = resolve(&config, "");
        assert_eq!(result, config.default_search.replace("{}", ""));

        // URL encoding special chars
        let result = resolve(&config, "!g c++ & rust/wasm");
        assert_eq!(
            result,
            "https://www.google.com/search?q=c%2B%2B%20%26%20rust/wasm"
        );

        // Only a bang with no search term
        let result = resolve(&config, "!g");
        assert_eq!(result, "https://www.google.com/search?q=");
    }
}
