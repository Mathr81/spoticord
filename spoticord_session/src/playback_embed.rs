use anyhow::{anyhow, Result};
use log::{error, trace};
use poise::ChoiceParameter;
use serenity::{
    all::{
        ButtonStyle, CommandInteraction, ComponentInteraction, ComponentInteractionCollector,
        Context, CreateActionRow, CreateButton, CreateEmbed, CreateEmbedAuthor, CreateEmbedFooter,
        CreateInteractionResponse, CreateInteractionResponseFollowup,
        CreateInteractionResponseMessage, CreateMessage, EditMessage, Message, User,
    },
    futures::StreamExt,
};
use spoticord_player::{info::PlaybackInfo, PlayerHandle, PlayerState, RepeatMode, DEFAULT_VOLUME};
use spoticord_utils::discord::Colors;
use std::{ops::ControlFlow, time::Duration};
use tokio::{sync::mpsc, time::Instant};

use crate::{Session, SessionHandle};

#[derive(Debug)]
pub enum Command {
    InvokeUpdate(bool),
}

#[derive(Debug, Default, ChoiceParameter)]
pub enum UpdateBehavior {
    #[default]
    #[name = "Automatically update the embed"]
    Default,

    #[name = "Do not update the embed"]
    Static,

    #[name = "Re-send the embed after track changes"]
    Pinned,
}

impl UpdateBehavior {
    pub fn is_static(&self) -> bool {
        matches!(self, Self::Static)
    }

    pub fn is_pinned(&self) -> bool {
        matches!(self, Self::Pinned)
    }
}

pub struct PlaybackEmbed {
    id: u64,
    ctx: Context,
    session: SessionHandle,
    message: Message,

    last_update: Instant,
    update_in: Option<Duration>,
    force_edit: bool,
    update_behavior: UpdateBehavior,
    /// When set, render the full "dashboard" layout (extra controls + state line).
    full: bool,

    rx: mpsc::Receiver<Command>,
}

/// Amount a single volume button press changes the volume by (~10% of the range).
const VOLUME_STEP: u16 = u16::MAX / 10;

impl PlaybackEmbed {
    pub async fn create(
        session: &Session,
        handle: SessionHandle,
        interaction: CommandInteraction,
        update_behavior: UpdateBehavior,
        full: bool,
    ) -> Result<Option<PlaybackEmbedHandle>> {
        let ctx = session.context.clone();

        if !session.active {
            respond_not_playing(&ctx, interaction).await?;

            return Ok(None);
        }

        let owner = session.owner.to_user(&ctx).await?;

        let Some(playback_info) = session.player.playback_info().await? else {
            respond_not_playing(&ctx, interaction).await?;

            return Ok(None);
        };

        let state = if full {
            session.player.state().await.ok()
        } else {
            None
        };

        let ctx_id = interaction.id.get();

        // Send initial reply
        interaction
            .create_response(
                &ctx,
                CreateInteractionResponse::Message(
                    CreateInteractionResponseMessage::new()
                        .embed(build_embed(&playback_info, &owner, state.as_ref()))
                        .components(build_components(ctx_id, playback_info.playing(), full)),
                ),
            )
            .await?;

        // If this is a static embed, we don't need to return any handles
        if update_behavior.is_static() {
            return Ok(None);
        }

        // Retrieve message instead of editing interaction response, as those tokens are only valid for 15 minutes
        let message = interaction.get_response(&ctx).await?;

        let collector = ComponentInteractionCollector::new(&ctx)
            .filter(move |press| press.data.custom_id.starts_with(&ctx_id.to_string()))
            .timeout(Duration::from_secs(3600 * 24));

        let (tx, rx) = mpsc::channel(16);
        let this = Self {
            id: ctx_id,
            ctx,
            session: handle,
            message,
            last_update: Instant::now(),
            update_in: None,
            force_edit: false,
            update_behavior,
            full,
            rx,
        };

        tokio::spawn(this.run(collector));

        Ok(Some(PlaybackEmbedHandle { tx }))
    }

    async fn run(mut self, collector: ComponentInteractionCollector) {
        let mut stream = collector.stream();

        loop {
            tokio::select! {
                opt_command = self.rx.recv() => {
                    let Some(command) = opt_command else {
                        break;
                    };

                    if self.handle_command(command).await.is_break() {
                        break;
                    }
                },

                opt_press = stream.next() => {
                    let Some(press) = opt_press else {
                        break;
                    };

                    self.handle_press(press).await;
                }

                _ = async {
                    if let Some(update_in) = self.update_in.take()
                    {
                        tokio::time::sleep(update_in).await;
                    }
                }, if self.update_in.is_some() => {
                    if self.update_embed(self.force_edit).await.is_break() {
                        break;
                    }
                }
            }
        }
    }

    async fn handle_command(&mut self, command: Command) -> ControlFlow<(), ()> {
        trace!("Received command: {command:?}");

        match command {
            Command::InvokeUpdate(force_edit) => {
                if self.last_update.elapsed() < Duration::from_secs(2) {
                    if self.update_in.is_some() {
                        return ControlFlow::Continue(());
                    }

                    self.update_in = Some(Duration::from_secs(2) - self.last_update.elapsed());
                    self.force_edit = force_edit;
                } else {
                    self.update_embed(force_edit).await?;
                }
            }
        }

        ControlFlow::Continue(())
    }

    async fn handle_press(&self, press: ComponentInteraction) {
        trace!("Received button press: {press:?}");

        let Ok((player, playback_info, owner)) = self.get_info().await else {
            _ = press
                .create_followup(
                    &self.ctx,
                    CreateInteractionResponseFollowup::new()
                        .embed(
                            CreateEmbed::new()
                                .title("Cannot perform action")
                                .description("I'm currently not playing any music in this server"),
                        )
                        .ephemeral(true),
                )
                .await;

            return;
        };

        if press.user.id != owner.id {
            _ = press
                .create_followup(
                    &self.ctx,
                    CreateInteractionResponseFollowup::new()
                        .embed(
                            CreateEmbed::new()
                                .title("Cannot perform action")
                                .description("Only the host may use the media buttons"),
                        )
                        .ephemeral(true),
                )
                .await;

            return;
        }

        match press.data.custom_id.split('-').next_back() {
            Some("next") => player.next_track().await,
            Some("prev") => player.previous_track().await,
            Some("pause") => {
                if playback_info.playing() {
                    player.pause().await
                } else {
                    player.play().await
                }
            }
            Some("shuffle") => {
                let shuffle = player.state().await.map(|state| !state.shuffle).unwrap_or(true);
                player.set_shuffle(shuffle).await;

                acknowledge(&self.ctx, &press).await;
                ephemeral_followup(
                    &self.ctx,
                    &press,
                    format!("🔀 Shuffle **{}**", if shuffle { "on" } else { "off" }),
                )
                .await;
                return;
            }
            Some("repeat") => {
                let mode = player
                    .state()
                    .await
                    .map(|state| state.repeat.next())
                    .unwrap_or(RepeatMode::Context);
                player.set_repeat(mode).await;

                acknowledge(&self.ctx, &press).await;
                ephemeral_followup(&self.ctx, &press, format!("🔁 Repeat **{}**", mode.label()))
                    .await;
                return;
            }
            Some(action @ ("voldown" | "volup")) => {
                let current = player
                    .state()
                    .await
                    .map(|state| state.volume)
                    .unwrap_or(DEFAULT_VOLUME);
                let volume = if action == "volup" {
                    current.saturating_add(VOLUME_STEP)
                } else {
                    current.saturating_sub(VOLUME_STEP)
                };
                player.set_volume(volume).await;

                let percent = u32::from(volume) * 100 / u32::from(u16::MAX);
                acknowledge(&self.ctx, &press).await;
                ephemeral_followup(&self.ctx, &press, format!("🔊 Volume **{percent}%**")).await;
                return;
            }
            Some("jam") => {
                acknowledge(&self.ctx, &press).await;

                let message = match player.create_jam().await {
                    Ok(Ok(url)) => format!(
                        "🎉 **Spotify Jam** — anyone with this link can join and control the music:\n{url}"
                    ),
                    Ok(Err(why)) => why,
                    Err(why) => format!("Failed to start a Jam: {why}"),
                };

                ephemeral_followup(&self.ctx, &press, message).await;
                return;
            }

            _ => {}
        }

        acknowledge(&self.ctx, &press).await;
    }

    async fn get_info(&self) -> Result<(PlayerHandle, PlaybackInfo, User)> {
        let player = self.session.player().await?;
        let owner = self.session.owner().await?.to_user(&self.ctx).await?;
        let playback_info = player
            .playback_info()
            .await?
            .ok_or_else(|| anyhow!("No playback info present"))?;

        Ok((player, playback_info, owner))
    }

    async fn update_embed(&mut self, force_edit: bool) -> ControlFlow<(), ()> {
        self.update_in = None;

        let Ok(owner) = self.session.owner().await else {
            _ = self.update_not_playing().await;

            return ControlFlow::Break(());
        };

        let Ok(player) = self.session.player().await else {
            _ = self.update_not_playing().await;

            return ControlFlow::Break(());
        };

        let Ok(Some(playback_info)) = player.playback_info().await else {
            _ = self.update_not_playing().await;

            return ControlFlow::Break(());
        };

        let owner = match owner.to_user(&self.ctx).await {
            Ok(owner) => owner,
            Err(why) => {
                error!("Failed to resolve owner: {why}");

                return ControlFlow::Break(());
            }
        };

        let state = if self.full {
            player.state().await.ok()
        } else {
            None
        };

        let should_pin = !force_edit && self.update_behavior.is_pinned();

        if should_pin {
            self.message.delete(&self.ctx).await.ok();

            match self
                .message
                .channel_id
                .send_message(
                    &self.ctx,
                    CreateMessage::new()
                        .embed(build_embed(&playback_info, &owner, state.as_ref()))
                        .components(build_components(self.id, playback_info.playing(), self.full)),
                )
                .await
            {
                Ok(message) => self.message = message,
                Err(why) => {
                    error!("Failed to update playback embed: {why}");

                    return ControlFlow::Break(());
                }
            };
        } else if let Err(why) = self
            .message
            .edit(
                &self.ctx,
                EditMessage::new()
                    .embed(build_embed(&playback_info, &owner, state.as_ref()))
                    .components(build_components(self.id, playback_info.playing(), self.full)),
            )
            .await
        {
            error!("Failed to update playback embed: {why}");

            return ControlFlow::Break(());
        }

        self.last_update = Instant::now();

        ControlFlow::Continue(())
    }

    async fn update_not_playing(&mut self) -> Result<()> {
        // If pinned, try to delete old message and send new one
        if self.update_behavior.is_pinned() {
            self.message.delete(&self.ctx).await.ok();
            self.message = self
                .message
                .channel_id
                .send_message(&self.ctx, CreateMessage::new().embed(not_playing_embed()))
                .await?;

            return Ok(());
        }

        self.message
            .edit(&self.ctx, EditMessage::new().embed(not_playing_embed()))
            .await?;

        Ok(())
    }
}

pub struct PlaybackEmbedHandle {
    tx: mpsc::Sender<Command>,
}

impl PlaybackEmbedHandle {
    pub fn is_valid(&self) -> bool {
        !self.tx.is_closed()
    }

    pub async fn invoke_update(&self, force_edit: bool) -> Result<()> {
        self.tx.send(Command::InvokeUpdate(force_edit)).await?;

        Ok(())
    }
}

async fn respond_not_playing(context: &Context, interaction: CommandInteraction) -> Result<()> {
    interaction
        .create_response(
            context,
            CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new()
                    .embed(not_playing_embed())
                    .ephemeral(true),
            ),
        )
        .await?;

    Ok(())
}

fn not_playing_embed() -> CreateEmbed {
    CreateEmbed::new()
        .title("Cannot display song details")
        .description("I'm currently not playing any music in this server.")
        .color(Colors::Error)
}

async fn acknowledge(ctx: &Context, press: &ComponentInteraction) {
    _ = press
        .create_response(ctx, CreateInteractionResponse::Acknowledge)
        .await;
}

async fn ephemeral_followup(
    ctx: &Context,
    press: &ComponentInteraction,
    content: impl Into<String>,
) {
    _ = press
        .create_followup(
            ctx,
            CreateInteractionResponseFollowup::new()
                .content(content)
                .ephemeral(true),
        )
        .await;
}

fn build_embed(playback_info: &PlaybackInfo, owner: &User, state: Option<&PlayerState>) -> CreateEmbed {
    let mut description = String::new();

    description += &format!("## [{}]({})\n", playback_info.name(), playback_info.url());

    if let Some(artists) = playback_info.artists() {
        let artists = artists
            .iter()
            .map(|artist| {
                format!(
                    "[{}](https://open.spotify.com/artist/{})",
                    artist.name,
                    artist.id.to_id().unwrap_or_default()
                )
            })
            .collect::<Vec<_>>()
            .join(", ");

        description += &format!("By {artists}\n");
    }

    if let Some(album_name) = playback_info.album_name() {
        description += &format!("Album: **{album_name}**\n");
    }

    if let Some(show_name) = playback_info.show_name() {
        description += &format!("On {show_name}\n");
    }

    description += "\n";

    let position = playback_info.current_position();
    let index = position * 20 / playback_info.duration();

    description += if playback_info.playing() {
        "▶️ "
    } else {
        "⏸️ "
    };

    for i in 0..20 {
        if i == index {
            description.push('🔵');
        } else {
            description.push('▬');
        }
    }

    description += "\n:alarm_clock: ";
    description += &format!(
        "{} / {}",
        spoticord_utils::time_to_string(position / 1000),
        spoticord_utils::time_to_string(playback_info.duration() / 1000)
    );

    // Dashboard-only line showing volume and shuffle state.
    if let Some(state) = state {
        let percent = u32::from(state.volume) * 100 / u32::from(u16::MAX);
        description += &format!(
            "\n:level_slider: {percent}%\u{2003}:twisted_rightwards_arrows: {}\u{2003}:repeat: {}",
            if state.shuffle { "On" } else { "Off" },
            state.repeat.label()
        );
    }

    CreateEmbed::new()
        .author(
            CreateEmbedAuthor::new("Currently Playing")
                .icon_url("https://spoticord.com/spotify-logo.png"),
        )
        .description(description)
        .thumbnail(playback_info.thumbnail())
        .footer(
            CreateEmbedFooter::new(owner.global_name.as_ref().unwrap_or(&owner.name))
                .icon_url(owner.face()),
        )
        .color(Colors::Info)
}

/// Build the message components (button rows). The compact layout has a single
/// media-control row; the full dashboard adds a second row with shuffle, volume
/// and Jam controls.
fn build_components(id: u64, playing: bool, full: bool) -> Vec<CreateActionRow> {
    let prev_button = CreateButton::new(format!("{id}-prev"))
        .style(ButtonStyle::Primary)
        .label("⏮");

    let next_button = CreateButton::new(format!("{id}-next"))
        .style(ButtonStyle::Primary)
        .label("⏭");

    let pause_button = CreateButton::new(format!("{id}-pause"))
        .style(if playing {
            ButtonStyle::Danger
        } else {
            ButtonStyle::Success
        })
        .label(if playing { "Pause" } else { "Play" });

    let media_row = CreateActionRow::Buttons(vec![prev_button, pause_button, next_button]);

    if !full {
        return vec![media_row];
    }

    let vol_down_button = CreateButton::new(format!("{id}-voldown"))
        .style(ButtonStyle::Secondary)
        .label("🔉");

    let vol_up_button = CreateButton::new(format!("{id}-volup"))
        .style(ButtonStyle::Secondary)
        .label("🔊");

    let shuffle_button = CreateButton::new(format!("{id}-shuffle"))
        .style(ButtonStyle::Secondary)
        .label("🔀");

    let repeat_button = CreateButton::new(format!("{id}-repeat"))
        .style(ButtonStyle::Secondary)
        .label("🔁");

    let jam_button = CreateButton::new(format!("{id}-jam"))
        .style(ButtonStyle::Success)
        .label("🎉 Jam");

    let extra_row = CreateActionRow::Buttons(vec![
        vol_down_button,
        vol_up_button,
        shuffle_button,
        repeat_button,
        jam_button,
    ]);

    vec![media_row, extra_row]
}
