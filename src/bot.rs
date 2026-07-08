use std::sync::Arc;

use anyhow::{anyhow, Result};
use librespot::{core::cache::Cache, discovery::Credentials};
use log::{debug, info};
use poise::{serenity_prelude, Framework, FrameworkContext, FrameworkOptions};
use serenity::all::{ActivityData, FullEvent, Ready, ShardManager};
use spoticord_session::manager::SessionManager;

use crate::commands;

pub type Context<'a> = poise::Context<'a, Data, anyhow::Error>;

type Data = SessionManager;

pub fn framework_opts() -> FrameworkOptions<Data, anyhow::Error> {
    poise::FrameworkOptions {
        commands: vec![
            #[cfg(debug_assertions)]
            commands::debug::ping(),
            commands::core::help(),
            commands::core::version(),
            commands::music::join(),
            commands::music::disconnect(),
            commands::music::stop(),
            commands::music::playing(),
            commands::music::dashboard(),
            commands::music::lyrics(),
            commands::music::jam(),
            commands::music::volume(),
            commands::music::shuffle(),
            commands::music::repeat(),
        ],
        event_handler: |ctx, event, framework, data| {
            Box::pin(event_handler(ctx, event, framework, data))
        },
        ..Default::default()
    }
}

pub async fn setup(
    ctx: &serenity_prelude::Context,
    ready: &Ready,
    framework: &Framework<Data, anyhow::Error>,
    credentials: Credentials,
    cache: Cache,
    device_name: &'static str,
) -> Result<Data> {
    info!("Successfully logged in as {}", ready.user.name);

    #[cfg(debug_assertions)]
    poise::builtins::register_in_guild(
        ctx,
        &framework.options().commands,
        std::env::var("GUILD_ID")?.parse()?,
    )
    .await?;

    #[cfg(not(debug_assertions))]
    poise::builtins::register_globally(ctx, &framework.options().commands).await?;

    let songbird = songbird::get(ctx)
        .await
        .ok_or_else(|| anyhow!("Songbird was not registered during setup"))?;

    let manager = SessionManager::new(songbird, credentials, cache, device_name);

    tokio::spawn(shutdown_handler(
        manager.clone(),
        framework.shard_manager().clone(),
    ));

    Ok(manager)
}

async fn event_handler(
    ctx: &serenity_prelude::Context,
    event: &FullEvent,
    _framework: FrameworkContext<'_, Data, anyhow::Error>,
    _data: &Data,
) -> Result<()> {
    if let FullEvent::Ready { data_about_bot } = event {
        if let Some(shard) = data_about_bot.shard {
            debug!(
                "Shard {} logged in (total shards: {})",
                shard.id.0, shard.total
            );
        }

        ctx.set_activity(Some(ActivityData::listening(spoticord_config::MOTD)));
    }

    Ok(())
}

/// Wait for an interrupt signal, then gracefully disconnect all sessions.
async fn shutdown_handler(session_manager: SessionManager, shard_manager: Arc<ShardManager>) {
    _ = tokio::signal::ctrl_c().await;

    info!("Received interrupt signal, shutting down...");

    session_manager.shutdown_all().await;
    shard_manager.shutdown_all().await;
}
