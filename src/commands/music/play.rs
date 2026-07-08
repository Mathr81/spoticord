use std::time::Duration;

use anyhow::Result;
use poise::CreateReply;
use serenity::all::{
    ComponentInteractionCollector, CreateEmbed, CreateInteractionResponse, EditMessage,
};
use serenity::futures::StreamExt;
use spoticord_session::manager::SessionQuery;
use spoticord_utils::discord::Colors;

use super::browse;
use crate::bot::Context;

/// Search Spotify and play a track
#[poise::command(slash_command, guild_only)]
pub async fn play(
    ctx: Context<'_>,
    #[description = "What to search for"] query: String,
) -> Result<()> {
    let manager = ctx.data();
    let guild = ctx.guild_id().expect("poise lied to me");

    let Some(session) = manager.get_session(SessionQuery::Guild(guild)) else {
        ctx.send(
            CreateReply::default()
                .embed(
                    CreateEmbed::new()
                        .title("Cannot play")
                        .description("I'm currently not playing any music in this server.\nUse `/join` first.")
                        .color(Colors::Error),
                )
                .ephemeral(true),
        )
        .await?;

        return Ok(());
    };

    ctx.defer().await?;

    let player = session.player().await?;

    let results = match player.search(&query).await? {
        Ok(results) => results,
        Err(why) => {
            ctx.send(
                CreateReply::default()
                    .embed(
                        CreateEmbed::new()
                            .title("Search failed")
                            .description(why)
                            .color(Colors::Error),
                    )
                    .ephemeral(true),
            )
            .await?;

            return Ok(());
        }
    };

    if results.is_empty() {
        ctx.send(
            CreateReply::default()
                .embed(
                    CreateEmbed::new()
                        .title("No results")
                        .description(format!("Nothing on Spotify matched **{query}**."))
                        .color(Colors::Warning),
                )
                .ephemeral(true),
        )
        .await?;

        return Ok(());
    }

    let handle = ctx
        .send(
            CreateReply::default()
                .embed(browse::results_embed(
                    "🔎 Search results",
                    &format!("Results for **{query}** — pick a track to play:"),
                    &results,
                ))
                .components(browse::result_buttons(&results, "▶️")),
        )
        .await?;

    let mut message = handle.into_message().await?;

    let mut collector = ComponentInteractionCollector::new(ctx.serenity_context())
        .message_id(message.id)
        .author_id(ctx.author().id)
        .timeout(Duration::from_secs(60))
        .stream();

    if let Some(press) = collector.next().await {
        if let Some(track) = browse::parse_index(&press.data.custom_id).and_then(|i| results.get(i))
        {
            player.play_uri(track.uri.clone()).await;

            _ = press
                .create_response(ctx.serenity_context(), CreateInteractionResponse::Acknowledge)
                .await;
            _ = message
                .edit(
                    ctx.serenity_context(),
                    EditMessage::new()
                        .embed(browse::now_playing_embed(track))
                        .components(vec![]),
                )
                .await;

            return Ok(());
        }
    }

    // Timed out without a selection: drop the now-dead buttons.
    _ = message
        .edit(ctx.serenity_context(), EditMessage::new().components(vec![]))
        .await;

    Ok(())
}
