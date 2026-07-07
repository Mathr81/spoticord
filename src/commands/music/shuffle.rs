use anyhow::Result;
use poise::CreateReply;
use serenity::all::CreateEmbed;
use spoticord_session::manager::SessionQuery;
use spoticord_utils::discord::Colors;

use crate::bot::Context;

/// Toggle Spotify shuffle mode
#[poise::command(slash_command, guild_only)]
pub async fn shuffle(
    ctx: Context<'_>,
    #[description = "Enable or disable shuffle"] enabled: bool,
) -> Result<()> {
    let manager = ctx.data();
    let guild = ctx.guild_id().expect("poise lied to me");

    let Some(session) = manager.get_session(SessionQuery::Guild(guild)) else {
        ctx.send(
            CreateReply::default()
                .embed(
                    CreateEmbed::new()
                        .title("Cannot change shuffle")
                        .description("I'm currently not playing any music in this server.")
                        .color(Colors::Error),
                )
                .ephemeral(true),
        )
        .await?;

        return Ok(());
    };

    let player = session.player().await?;
    player.set_shuffle(enabled).await;

    ctx.send(
        CreateReply::default().embed(
            CreateEmbed::new()
                .title(if enabled {
                    "Shuffle enabled"
                } else {
                    "Shuffle disabled"
                })
                .description(if enabled {
                    "Playback is now shuffled."
                } else {
                    "Playback is no longer shuffled."
                })
                .color(Colors::Info),
        ),
    )
    .await?;

    Ok(())
}
