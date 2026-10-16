mod browse;
mod dashboard;
mod disconnect;
mod jam;
mod join;
mod lyrics;
mod play;
mod playing;
mod queue;
mod repeat;
mod seek;
mod shuffle;
mod stop;
mod volume;

pub use dashboard::*;
pub use disconnect::*;
pub use jam::*;
pub use join::*;
pub use lyrics::*;
pub use play::*;
pub use playing::*;
pub use queue::*;
pub use repeat::*;
pub use seek::*;
pub use shuffle::*;
pub use stop::*;
pub use volume::*;

use anyhow::Result;
use poise::CreateReply;
use serenity::all::CreateEmbed;
use spoticord_session::{manager::SessionQuery, SessionHandle};
use spoticord_utils::discord::Colors;

use crate::bot::Context;

/// The session in this server, if its player is running. Otherwise replies with an
/// error titled `title` and returns `None`: after `/stop`, or once the host left, the
/// session lingers in the call but commands sent to its player would silently do nothing.
async fn active_session(ctx: Context<'_>, title: &str) -> Result<Option<SessionHandle>> {
    let guild = ctx.guild_id().expect("poise lied to me");

    if let Some(session) = ctx.data().get_session(SessionQuery::Guild(guild)) {
        if session.active().await.unwrap_or(false) {
            return Ok(Some(session));
        }
    }

    ctx.send(
        CreateReply::default()
            .embed(
                CreateEmbed::new()
                    .title(title)
                    .description(
                        "I'm currently not playing any music in this server.\nUse `/join` first.",
                    )
                    .color(Colors::Error),
            )
            .ephemeral(true),
    )
    .await?;

    Ok(None)
}
