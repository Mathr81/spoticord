use anyhow::Result;
use poise::CreateReply;
use serenity::all::CreateEmbed;
use spoticord_session::manager::SessionQuery;
use spoticord_utils::discord::Colors;

use crate::bot::Context;

/// Start a Spotify Jam and share the link so others can control the music
#[poise::command(slash_command, guild_only)]
pub async fn jam(ctx: Context<'_>) -> Result<()> {
    let manager = ctx.data();
    let guild = ctx.guild_id().expect("poise lied to me");

    let Some(session) = manager.get_session(SessionQuery::Guild(guild)) else {
        ctx.send(
            CreateReply::default()
                .embed(
                    CreateEmbed::new()
                        .title("Cannot start a Jam")
                        .description(
                            "I'm currently not playing any music in this server.\nUse `/join` first.",
                        )
                        .color(Colors::Error),
                )
                .ephemeral(true),
        )
        .await?;

        return Ok(());
    };

    // Talking to Spotify's API can take a moment.
    ctx.defer().await?;

    let player = session.player().await?;

    match player.create_jam().await? {
        Ok(url) => {
            ctx.send(
                CreateReply::default().embed(
                    CreateEmbed::new()
                        .title("🎉 Spotify Jam")
                        .description(format!(
                            "Anyone with this link can join and control the music:\n\n{url}"
                        ))
                        .color(Colors::Info),
                ),
            )
            .await?;
        }
        Err(why) => {
            ctx.send(
                CreateReply::default()
                    .embed(
                        CreateEmbed::new()
                            .title("Could not start a Jam")
                            .description(why)
                            .color(Colors::Error),
                    )
                    .ephemeral(true),
            )
            .await?;
        }
    }

    Ok(())
}
