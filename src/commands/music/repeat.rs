use anyhow::Result;
use poise::{ChoiceParameter, CreateReply};
use serenity::all::CreateEmbed;
use spoticord_player::RepeatMode;
use spoticord_utils::discord::Colors;

use crate::bot::Context;

#[derive(ChoiceParameter)]
pub enum RepeatChoice {
    #[name = "Off"]
    Off,
    #[name = "Repeat the whole queue"]
    All,
    #[name = "Repeat the current track"]
    One,
}

impl From<RepeatChoice> for RepeatMode {
    fn from(choice: RepeatChoice) -> Self {
        match choice {
            RepeatChoice::Off => RepeatMode::Off,
            RepeatChoice::All => RepeatMode::Context,
            RepeatChoice::One => RepeatMode::Track,
        }
    }
}

/// Set the Spotify repeat mode
#[poise::command(slash_command, guild_only)]
pub async fn repeat(
    ctx: Context<'_>,
    #[description = "Repeat mode"] mode: RepeatChoice,
) -> Result<()> {
    let Some(session) = super::active_session(ctx, "Cannot change repeat").await? else {
        return Ok(());
    };

    let mode: RepeatMode = mode.into();

    let player = session.player().await?;
    player.set_repeat(mode).await;

    ctx.send(
        CreateReply::default().embed(
            CreateEmbed::new()
                .title("Repeat updated")
                .description(format!("Repeat mode set to **{}**.", mode.label()))
                .color(Colors::Info),
        ),
    )
    .await?;

    Ok(())
}
