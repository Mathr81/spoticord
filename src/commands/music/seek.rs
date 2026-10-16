use anyhow::Result;
use poise::CreateReply;
use serenity::all::CreateEmbed;
use spoticord_utils::discord::Colors;

use crate::bot::Context;

/// Jump to a position in the current track
#[poise::command(slash_command, guild_only)]
pub async fn seek(
    ctx: Context<'_>,
    #[description = "Position, e.g. 1:30, 1:02:15 or 90 (seconds)"] position: String,
) -> Result<()> {
    let Some(session) = super::active_session(ctx, "Cannot seek").await? else {
        return Ok(());
    };

    let Some(position_ms) = parse_position(&position) else {
        ctx.send(
            CreateReply::default()
                .embed(
                    CreateEmbed::new()
                        .title("Invalid position")
                        .description("Use `m:ss`, `h:mm:ss` or a number of seconds, e.g. `1:30`.")
                        .color(Colors::Error),
                )
                .ephemeral(true),
        )
        .await?;

        return Ok(());
    };

    let player = session.player().await?;
    player.seek(position_ms).await;

    ctx.send(
        CreateReply::default().embed(
            CreateEmbed::new()
                .title("Seeked")
                .description(format!(
                    "Jumped to **{}**",
                    spoticord_utils::time_to_string(position_ms / 1000)
                ))
                .color(Colors::Info),
        ),
    )
    .await?;

    Ok(())
}

/// Parse `90`, `1:30` or `1:02:15` into milliseconds.
fn parse_position(input: &str) -> Option<u32> {
    let mut seconds: u32 = 0;

    for (index, part) in input.trim().split(':').enumerate() {
        let value: u32 = part.parse().ok()?;

        // Every field after the first is minutes or seconds
        if index > 0 && value >= 60 {
            return None;
        }

        seconds = seconds.checked_mul(60)?.checked_add(value)?;
    }

    // Spotify positions are u32 milliseconds; more than 3 fields is not a time
    (input.split(':').count() <= 3)
        .then(|| seconds.checked_mul(1000))
        .flatten()
}

#[cfg(test)]
mod tests {
    use super::parse_position;

    #[test]
    fn parses_positions() {
        assert_eq!(parse_position("90"), Some(90_000));
        assert_eq!(parse_position("1:30"), Some(90_000));
        assert_eq!(parse_position(" 1:02:15 "), Some(3_735_000));
        assert_eq!(parse_position("1:75"), None);
        assert_eq!(parse_position("abc"), None);
        assert_eq!(parse_position("1:2:3:4"), None);
        assert_eq!(parse_position(""), None);
    }
}
