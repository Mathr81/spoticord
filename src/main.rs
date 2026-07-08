mod auth;
mod bot;
mod commands;

use log::{error, info};
use poise::Framework;
use serenity::all::ClientBuilder;
use songbird::SerenityInit;

#[tokio::main]
async fn main() {
    // Force aws-lc-rs as default crypto provider
    // Since multiple dependencies either enable aws_lc_rs or ring, they cause a clash, so we have to
    // explicitly tell rustls to use the aws-lc-rs provider
    _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

    // Setup logging
    if std::env::var("RUST_LOG").is_err() {
        #[cfg(debug_assertions)]
        std::env::set_var("RUST_LOG", "spoticord");

        #[cfg(not(debug_assertions))]
        std::env::set_var("RUST_LOG", "spoticord=info");
    }

    env_logger::init();

    info!("Today is a good day!");
    info!(" - Spoticord");

    dotenvy::dotenv().ok();

    // Resolve the single Spotify account's reusable credentials (from the cache,
    // or a one-time interactive OAuth login). No database or link frontend needed.
    let (credentials, cache) = match auth::resolve_credentials().await {
        Ok(result) => result,
        Err(why) => {
            error!("Failed to obtain Spotify credentials: {why:?}");
            return;
        }
    };

    // Optionally set up the Spotify Web API client (search + queue), which needs
    // your own Spotify Developer app. When the app credentials aren't configured,
    // `/play` and `/queue` are simply unavailable.
    let spotify = match (
        spoticord_config::spotify_client_id(),
        spoticord_config::spotify_client_secret(),
    ) {
        (Some(client_id), Some(client_secret)) => {
            match spoticord_spotify::WebApi::init(
                client_id.to_string(),
                client_secret.to_string(),
                spoticord_config::spotify_redirect_uri().to_string(),
                std::path::PathBuf::from(spoticord_config::cache_dir()),
            )
            .await
            {
                Ok(web_api) => Some(web_api),
                Err(why) => {
                    error!("Failed to initialize the Spotify Web API client: {why:?}");
                    error!("/play and /queue will be unavailable. Check SPOTIFY_CLIENT_ID/SECRET and the redirect URI.");
                    None
                }
            }
        }
        _ => {
            info!(
                "No Spotify app configured (SPOTIFY_CLIENT_ID/SECRET); /play and /queue are disabled."
            );
            None
        }
    };

    // Set up bot
    let framework = Framework::builder()
        .setup(move |ctx, ready, framework| {
            Box::pin(bot::setup(
                ctx,
                ready,
                framework,
                credentials,
                cache,
                spoticord_config::device_name(),
                spotify,
            ))
        })
        .options(bot::framework_opts())
        .build();

    let mut client = match ClientBuilder::new(
        spoticord_config::discord_token(),
        spoticord_config::discord_intents(),
    )
    .framework(framework)
    .register_songbird_from_config(songbird::Config::default().use_softclip(false))
    .await
    {
        Ok(client) => client,
        Err(why) => {
            error!("Fatal error when building Serenity client: {why}");
            return;
        }
    };

    if let Err(why) = client.start_autosharded().await {
        error!("Fatal error occured during bot operations: {why}");
        error!("Bot will now shut down!");
    }
}
