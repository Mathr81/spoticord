use anyhow::Result;
use poise::CreateReply;
use serenity::all::CreateEmbed;
use spoticord_session::manager::SessionQuery;
use spoticord_utils::discord::Colors;

use crate::bot::Context;

/// Set the Spotify playback volume (0-100%)
#[poise::command(slash_command, guild_only)]
pub async fn volume(
    ctx: Context<'_>,
    #[description = "Volume percentage (0-100)"]
    #[min = 0]
    #[max = 100]
    volume: u8,
) -> Result<()> {
    let manager = ctx.data();
    let guild = ctx.guild_id().expect("poise lied to me");

    let Some(session) = manager.get_session(SessionQuery::Guild(guild)) else {
        ctx.send(
            CreateReply::default()
                .embed(
                    CreateEmbed::new()
                        .title("Cannot change volume")
                        .description("I'm currently not playing any music in this server.")
                        .color(Colors::Error),
                )
                .ephemeral(true),
        )
        .await?;

        return Ok(());
    };

    let player = session.player().await?;

    // Spotify volume is 0..=u16::MAX
    let scaled = (f32::from(volume) / 100.0 * f32::from(u16::MAX)) as u16;
    player.set_volume(scaled).await;

    ctx.send(
        CreateReply::default().embed(
            CreateEmbed::new()
                .title("Volume updated")
                .description(format!("Playback volume set to **{volume}%**"))
                .color(Colors::Info),
        ),
    )
    .await?;

    Ok(())
}
