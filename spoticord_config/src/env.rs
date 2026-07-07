use std::sync::LazyLock;

pub static DISCORD_TOKEN: LazyLock<String> = LazyLock::new(|| {
    std::env::var("DISCORD_TOKEN").expect("missing DISCORD_TOKEN environment variable")
});

/// The Spotify Connect device name the bot advertises. Defaults to "Spoticord".
pub static DEVICE_NAME: LazyLock<String> =
    LazyLock::new(|| std::env::var("DEVICE_NAME").unwrap_or_else(|_| "Spoticord".to_string()));

/// Directory where librespot stores the reusable Spotify credentials
/// (`credentials.json`). Defaults to "/data".
pub static CACHE_DIR: LazyLock<String> =
    LazyLock::new(|| std::env::var("CACHE_DIR").unwrap_or_else(|_| "/data".to_string()));
