use anyhow::Result;
use poise::CreateReply;
use serenity::all::CreateEmbed;
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
    let Some(session) = super::active_session(ctx, "Cannot change volume").await? else {
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
