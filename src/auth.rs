use anyhow::{Context, Result};
use librespot::{
    core::{cache::Cache, SessionConfig},
    discovery::Credentials,
    oauth::OAuthClientBuilder,
};
use log::{info, warn};

/// Spotify OAuth scopes requested during the interactive login.
const OAUTH_SCOPES: &[&str] = &[
    "streaming",
    "user-read-playback-state",
    "user-modify-playback-state",
    "user-read-currently-playing",
    "user-read-private",
    "user-read-email",
];

/// Port for the local OAuth redirect server used during the one-time login.
const OAUTH_PORT: u16 = 8898;

/// Resolve the reusable Spotify credentials for the single account this bot plays
/// from.
///
/// On the first run (empty cache) this performs an interactive OAuth login: an
/// authorization URL is printed to the logs, you open it once and approve, and
/// the resulting reusable credentials are saved to the cache directory by
/// librespot the first time a session connects. Every subsequent run simply
/// reloads them from disk — no database or account-linking frontend required.
pub async fn resolve_credentials() -> Result<(Credentials, Cache)> {
    let cache = Cache::new(Some(spoticord_config::cache_dir()), None, None, None)
        .context("failed to open the credentials cache directory")?;

    if let Some(credentials) = cache.credentials() {
        info!("Loaded cached Spotify credentials");
        return Ok((credentials, cache));
    }

    warn!("No cached Spotify credentials found, starting interactive OAuth login.");
    warn!("Open the URL printed below in a browser and approve access. This is only needed once.");

    let token = OAuthClientBuilder::new(
        &SessionConfig::default().client_id,
        &format!("http://127.0.0.1:{OAUTH_PORT}/login"),
        OAUTH_SCOPES.to_vec(),
    )
    .build()
    .map_err(|why| anyhow::anyhow!("failed to build the Spotify OAuth client: {why}"))?
    .get_access_token_async()
    .await
    .map_err(|why| anyhow::anyhow!("Spotify OAuth login failed: {why}"))?;

    Ok((Credentials::with_access_token(token.access_token), cache))
}
