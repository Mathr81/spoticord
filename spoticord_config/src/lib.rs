mod env;

use serenity::all::GatewayIntents;

#[cfg(not(debug_assertions))]
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(debug_assertions)]
pub const VERSION: &str = concat!(env!("CARGO_PKG_VERSION"), "-dev");

/// The "listening to" message that shows up under the Spoticord bot user
pub const MOTD: &str = "some good 'ol music";

/// The time it takes (in seconds) for Spoticord to disconnect when no music is being played
pub const DISCONNECT_TIME: u64 = 5 * 60;

pub fn discord_token() -> &'static str {
    &env::DISCORD_TOKEN
}

pub fn discord_intents() -> GatewayIntents {
    GatewayIntents::GUILDS | GatewayIntents::GUILD_VOICE_STATES
}

/// The Spotify Connect device name the bot advertises.
pub fn device_name() -> &'static str {
    &env::DEVICE_NAME
}

/// Directory where the reusable Spotify credentials are cached.
pub fn cache_dir() -> &'static str {
    &env::CACHE_DIR
}

/// Your Spotify Developer app's client id (for the Web API), if configured.
pub fn spotify_client_id() -> Option<&'static str> {
    env::SPOTIFY_CLIENT_ID.as_deref()
}

/// Your Spotify Developer app's client secret (for the Web API), if configured.
pub fn spotify_client_secret() -> Option<&'static str> {
    env::SPOTIFY_CLIENT_SECRET.as_deref()
}

/// The redirect URI used for the one-time Web API authorization.
pub fn spotify_redirect_uri() -> &'static str {
    &env::SPOTIFY_REDIRECT_URI
}
