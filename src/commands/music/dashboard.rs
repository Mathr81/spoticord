use anyhow::Result;
use poise::CreateReply;
use serenity::all::CreateEmbed;
use spoticord_session::{manager::SessionQuery, playback_embed::UpdateBehavior};
use spoticord_utils::discord::Colors;

use crate::bot::Context;

/// Open a full playback dashboard with media, volume, shuffle and Jam controls
#[poise::command(slash_command, guild_only)]
pub async fn dashboard(
    ctx: Context<'_>,
    #[description = "How Spoticord should update this dashboard"] update_behavior: Option<
        UpdateBehavior,
    >,
) -> Result<()> {
    let manager = ctx.data();
    let guild = ctx.guild_id().expect("poise lied to me");

    let Some(session) = manager.get_session(SessionQuery::Guild(guild)) else {
        ctx.send(
            CreateReply::default()
                .embed(
                    CreateEmbed::new()
                        .title("Cannot open dashboard")
                        .description("I'm currently not playing any music in this server.")
                        .color(Colors::Error),
                )
                .ephemeral(true),
        )
        .await?;

        return Ok(());
    };

    let Context::Application(context) = ctx else {
        panic!("Slash command is a prefix command?");
    };

    session
        .create_dashboard_embed(context.interaction, update_behavior.unwrap_or_default())
        .await?;

    Ok(())
}
