use std::time::Duration;

use anyhow::Result;
use poise::CreateReply;
use serenity::all::{
    ComponentInteractionCollector, CreateEmbed, CreateInteractionResponse,
    CreateInteractionResponseFollowup, EditMessage,
};
use serenity::futures::StreamExt;
use spoticord_session::manager::SessionQuery;
use spoticord_utils::discord::Colors;

use super::browse;
use crate::bot::Context;

/// Show the play queue, or search Spotify to add a track to it
#[poise::command(slash_command, guild_only)]
pub async fn queue(
    ctx: Context<'_>,
    #[description = "A track to search for and add to the queue"] query: Option<String>,
) -> Result<()> {
    let manager = ctx.data();
    let guild = ctx.guild_id().expect("poise lied to me");

    if manager.get_session(SessionQuery::Guild(guild)).is_none() {
        ctx.send(
            CreateReply::default()
                .embed(
                    CreateEmbed::new()
                        .title("Cannot access the queue")
                        .description("I'm currently not playing any music in this server.\nUse `/join` first.")
                        .color(Colors::Error),
                )
                .ephemeral(true),
        )
        .await?;

        return Ok(());
    }

    let Some(spotify) = manager.spotify() else {
        ctx.send(
            CreateReply::default()
                .embed(browse::not_configured_embed())
                .ephemeral(true),
        )
        .await?;

        return Ok(());
    };

    ctx.defer().await?;

    let Some(query) = query else {
        // No query: just display the current queue.
        let tracks = match spotify.get_queue().await {
            Ok(tracks) => tracks,
            Err(why) => {
                ctx.send(
                    CreateReply::default()
                        .embed(
                            CreateEmbed::new()
                                .title("Could not read the queue")
                                .description(why.to_string())
                                .color(Colors::Error),
                        )
                        .ephemeral(true),
                )
                .await?;

                return Ok(());
            }
        };

        if tracks.is_empty() {
            ctx.send(
                CreateReply::default().embed(
                    CreateEmbed::new()
                        .title("🎶 Up next")
                        .description("The queue is empty.")
                        .color(Colors::Info),
                ),
            )
            .await?;
        } else {
            ctx.send(CreateReply::default().embed(browse::queue_embed(&tracks)))
                .await?;
        }

        return Ok(());
    };

    let results = match spotify.search(&query).await {
        Ok(results) => results,
        Err(why) => {
            ctx.send(
                CreateReply::default()
                    .embed(
                        CreateEmbed::new()
                            .title("Search failed")
                            .description(why.to_string())
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
                    "🔎 Add to queue",
                    &format!("Results for **{query}** — tap to add to the queue:"),
                    &results,
                ))
                .components(browse::result_buttons(&results, "➕")),
        )
        .await?;

    let mut message = handle.into_message().await?;

    let mut collector = ComponentInteractionCollector::new(ctx.serenity_context())
        .message_id(message.id)
        .author_id(ctx.author().id)
        .timeout(Duration::from_secs(60))
        .stream();

    // Keep collecting so the user can queue several tracks from one search.
    while let Some(press) = collector.next().await {
        let Some(track) = browse::parse_index(&press.data.custom_id).and_then(|i| results.get(i))
        else {
            continue;
        };

        _ = press
            .create_response(
                ctx.serenity_context(),
                CreateInteractionResponse::Acknowledge,
            )
            .await;

        let content = match spotify.add_to_queue(&track.uri).await {
            Ok(()) => format!(
                "➕ Added **{}** — {} to the queue",
                track.name, track.artists
            ),
            Err(why) => format!("Could not add to the queue: {why}"),
        };

        _ = press
            .create_followup(
                ctx.serenity_context(),
                CreateInteractionResponseFollowup::new()
                    .content(content)
                    .ephemeral(true),
            )
            .await;
    }

    _ = message
        .edit(
            ctx.serenity_context(),
            EditMessage::new().components(vec![]),
        )
        .await;

    Ok(())
}
