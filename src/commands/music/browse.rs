//! Shared helpers for the search-driven `/play` and `/queue` commands.

use serenity::all::{ButtonStyle, CreateActionRow, CreateButton, CreateEmbed};
use spoticord_spotify::TrackResult;
use spoticord_utils::discord::Colors;

/// Format a track duration (in milliseconds) as `m:ss`.
fn duration(ms: u32) -> String {
    spoticord_utils::time_to_string(ms / 1000)
}

/// Parse the selected result index out of a `trk-{index}` button custom id.
pub fn parse_index(custom_id: &str) -> Option<usize> {
    custom_id.strip_prefix("trk-")?.parse().ok()
}

/// Embed shown when the Spotify Web API (search/queue) isn't configured.
pub fn not_configured_embed() -> CreateEmbed {
    CreateEmbed::new()
        .title("Spotify search isn't set up")
        .description(
            "`/play` and `/queue` need your own Spotify app.\n\
             Set `SPOTIFY_CLIENT_ID` and `SPOTIFY_CLIENT_SECRET` in the bot's environment, \
             then restart it once to authorize.",
        )
        .color(Colors::Error)
}

/// Build an embed listing search results, prefixed with a subtitle.
pub fn results_embed(title: &str, subtitle: &str, results: &[TrackResult]) -> CreateEmbed {
    let mut description = format!("{subtitle}\n\n");

    for (index, track) in results.iter().enumerate() {
        description += &format!(
            "**{}.** {} — {} · `{}`\n",
            index + 1,
            track.name,
            track.artists,
            duration(track.duration_ms)
        );
    }

    CreateEmbed::new()
        .title(title)
        .description(description)
        .color(Colors::Info)
}

/// Build a single row of numbered buttons (one per result), each labelled with
/// `emoji` and its position.
pub fn result_buttons(results: &[TrackResult], emoji: &str) -> Vec<CreateActionRow> {
    let buttons = results
        .iter()
        .enumerate()
        .map(|(index, _)| {
            CreateButton::new(format!("trk-{index}"))
                .style(ButtonStyle::Primary)
                .label(format!("{emoji} {}", index + 1))
        })
        .collect();

    vec![CreateActionRow::Buttons(buttons)]
}

/// Build the "now playing" confirmation embed shown after a track is picked.
pub fn now_playing_embed(track: &TrackResult) -> CreateEmbed {
    CreateEmbed::new()
        .title("▶️ Now playing")
        .description(format!(
            "**{}** — {}\nAlbum: {}",
            track.name, track.artists, track.album
        ))
        .color(Colors::Info)
}

/// Build an embed listing the upcoming tracks in the queue.
pub fn queue_embed(tracks: &[TrackResult]) -> CreateEmbed {
    let mut description = String::new();

    for (index, track) in tracks.iter().take(15).enumerate() {
        description += &format!(
            "**{}.** {} — {} · `{}`\n",
            index + 1,
            track.name,
            track.artists,
            duration(track.duration_ms)
        );
    }

    CreateEmbed::new()
        .title("🎶 Up next")
        .description(description)
        .color(Colors::Info)
}
