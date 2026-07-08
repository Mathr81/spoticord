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
