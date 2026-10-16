use anyhow::Result;
use log::warn;
use poise::CreateReply;
use serenity::all::{CreateAttachment, CreateEmbed};
use spoticord_utils::discord::Colors;

use crate::bot::Context;

/// Start a Spotify Jam and share the link so others can control the music
#[poise::command(slash_command, guild_only)]
pub async fn jam(ctx: Context<'_>) -> Result<()> {
    let Some(session) = super::active_session(ctx, "Cannot start a Jam").await? else {
        return Ok(());
    };

    // Talking to Spotify's API can take a moment.
    ctx.defer().await?;

    let player = session.player().await?;

    match player.create_jam().await? {
        Ok(url) => {
            let mut embed = CreateEmbed::new()
                .title("🎉 Spotify Jam")
                .description(format!(
                    "Anyone with this link can join and control the music:\n\n{url}\n\nOr scan the code below with a phone camera or the Spotify app."
                ))
                .color(Colors::Info);

            let mut reply = CreateReply::default();

            match spoticord_utils::qr_png(&url) {
                Ok(png) => {
                    embed = embed.image("attachment://jam-qr.png");
                    reply = reply.attachment(CreateAttachment::bytes(png, "jam-qr.png"));
                }
                Err(why) => warn!("Failed to render Jam QR code: {why}"),
            }

            ctx.send(reply.embed(embed)).await?;
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
