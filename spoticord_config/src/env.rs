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

/// Client id of your own Spotify Developer app, used for the Web API (search and
/// queue). Optional; when unset, `/play` and `/queue` are disabled.
pub static SPOTIFY_CLIENT_ID: LazyLock<Option<String>> = LazyLock::new(|| {
    std::env::var("SPOTIFY_CLIENT_ID")
        .ok()
        .filter(|s| !s.is_empty())
});

/// Client secret of your own Spotify Developer app (see `SPOTIFY_CLIENT_ID`).
pub static SPOTIFY_CLIENT_SECRET: LazyLock<Option<String>> = LazyLock::new(|| {
    std::env::var("SPOTIFY_CLIENT_SECRET")
        .ok()
        .filter(|s| !s.is_empty())
});

/// Redirect URI registered on your Spotify app for the one-time Web API
/// authorization. Must match exactly. Defaults to `http://127.0.0.1:8898/callback`.
pub static SPOTIFY_REDIRECT_URI: LazyLock<String> = LazyLock::new(|| {
    std::env::var("SPOTIFY_REDIRECT_URI")
        .unwrap_or_else(|_| "http://127.0.0.1:8898/callback".to_string())
});

/// Initial playback volume, as a percentage (0-100). Defaults to 75.
pub static DEFAULT_VOLUME: LazyLock<u8> = LazyLock::new(|| {
    std::env::var("DEFAULT_VOLUME")
        .ok()
        .and_then(|v| v.parse::<u8>().ok())
        .map(|v| v.min(100))
        .unwrap_or(75)
});

/// Seconds of inactivity before the bot leaves the voice channel. Defaults to 300.
pub static DISCONNECT_TIME: LazyLock<u64> = LazyLock::new(|| {
    std::env::var("DISCONNECT_TIME")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(5 * 60)
});
