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
