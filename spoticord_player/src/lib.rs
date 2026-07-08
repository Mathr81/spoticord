pub mod info;

use anyhow::Result;
use info::PlaybackInfo;
use librespot::{
    connect::{ConnectConfig, Spirc},
    core::{
        cache::Cache, error::ErrorKind, http_client::HttpClientError, Session as SpotifySession,
        SessionConfig,
    },
    discovery::Credentials,
    metadata::Lyrics,
    playback::{
        config::{Bitrate, PlayerConfig, VolumeCtrl},
        mixer::{self, MixerConfig},
        player::{Player as SpotifyPlayer, PlayerEvent as SpotifyPlayerEvent},
    },
};
use log::{error, trace};
use songbird::{input::RawAdapter, tracks::TrackHandle, Call};
use spoticord_audio::{
    sink::{SinkEvent, StreamSink},
    stream::Stream,
};
use std::{
    io::Write,
    sync::{atomic::AtomicBool, Arc},
};
use tokio::sync::{mpsc, oneshot, Mutex};

/// Default playback volume (75% of `u16::MAX`), matching the value handed to
/// librespot's `ConnectConfig` when the player starts.
pub const DEFAULT_VOLUME: u16 = 49151;

/// Repeat mode, matching Spotify's three states.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RepeatMode {
    #[default]
    Off,
    /// Repeat the whole context (playlist/album/queue).
    Context,
    /// Repeat the current track.
    Track,
}

impl RepeatMode {
    /// The next mode when cycling through the dashboard button (Off → Context → Track → Off).
    pub fn next(self) -> Self {
        match self {
            Self::Off => Self::Context,
            Self::Context => Self::Track,
            Self::Track => Self::Off,
        }
    }

    /// A short human-readable label.
    pub fn label(self) -> &'static str {
        match self {
            Self::Off => "Off",
            Self::Context => "All",
            Self::Track => "One",
        }
    }
}

/// A snapshot of the controllable player state that isn't part of [`PlaybackInfo`].
///
/// These values reflect the commands Spoticord has issued; they may drift if the
/// same account is controlled from elsewhere (e.g. the Spotify app directly).
#[derive(Debug, Clone, Copy)]
pub struct PlayerState {
    pub volume: u16,
    pub shuffle: bool,
    pub repeat: RepeatMode,
}

#[derive(Debug)]
enum PlayerCommand {
    NextTrack,
    PreviousTrack,
    Pause,
    Play,
    SetVolume(u16),
    SetShuffle(bool),
    SetRepeat(RepeatMode),

    GetPlaybackInfo(oneshot::Sender<Option<PlaybackInfo>>),
    GetState(oneshot::Sender<PlayerState>),
    GetLyrics(oneshot::Sender<Option<Lyrics>>),
    CreateJam(oneshot::Sender<Result<String, String>>),

    Shutdown,
}

#[derive(Debug)]
pub enum PlayerEvent {
    Pause,
    Play,
    Stopped,
    TrackChanged(Box<PlaybackInfo>),
    ConnectionReset,
}

pub struct Player {
    session: SpotifySession,
    spirc: Spirc,
    track: TrackHandle,
    stream: Stream,

    playback_info: Option<PlaybackInfo>,
    volume: u16,
    shuffle: bool,
    repeat: RepeatMode,

    // Communication
    events: mpsc::Sender<PlayerEvent>,

    commands: mpsc::Receiver<PlayerCommand>,
    spotify_events: mpsc::UnboundedReceiver<SpotifyPlayerEvent>,
    sink_events: mpsc::UnboundedReceiver<SinkEvent>,

    /// A shared boolean that reflects whether this Player has shut down
    shutdown: Arc<AtomicBool>,
}

impl Player {
    pub async fn create(
        credentials: Credentials,
        cache: Cache,
        call: Arc<Mutex<Call>>,
        device_name: impl Into<String>,
    ) -> Result<(PlayerHandle, mpsc::Receiver<PlayerEvent>), librespot::core::Error> {
        let (event_tx, event_rx) = mpsc::channel(16);

        let mut call_lock = call.lock().await;
        let stream = Stream::new();

        // Create songbird audio track
        let adapter = RawAdapter::new(stream.clone(), 44100, 2);
        let track = call_lock.play_only_input(adapter.into());
        _ = track.pause();

        // Free call lock before creating session
        drop(call_lock);

        // Create librespot audio streamer. Passing the cache lets librespot persist
        // the reusable credentials to disk, so we only need to authenticate once.
        let session = SpotifySession::new(SessionConfig::default(), Some(cache));
        let mixer = (mixer::find(Some("softvol")).expect("missing softvol mixer"))(MixerConfig {
            volume_ctrl: VolumeCtrl::Log(VolumeCtrl::DEFAULT_DB_RANGE),
            ..Default::default()
        })
        .expect("failed to open softvol mixer");

        let (tx_sink, rx_sink) = mpsc::unbounded_channel();
        let player = SpotifyPlayer::new(
            PlayerConfig {
                // Highest quality Spotify offers (320kbps OGG Vorbis, Premium only).
                bitrate: Bitrate::Bitrate320,
                ..Default::default()
            },
            session.clone(),
            mixer.get_soft_volume(),
            {
                let stream = stream.clone();
                move || Box::new(StreamSink::new(stream, tx_sink))
            },
        );
        let rx_player = player.get_player_event_channel();

        let device_name = device_name.into();
        let mut tries = 0;

        let (spirc, spirc_task) = loop {
            match Spirc::new(
                ConnectConfig {
                    name: device_name.clone(),
                    initial_volume: DEFAULT_VOLUME,
                    ..Default::default()
                },
                session.clone(),
                credentials.clone(),
                player.clone(),
                mixer.clone(),
            )
            .await
            {
                Ok(spirc) => break spirc,
                Err(why) => {
                    // Instantly return if authentication was rejected. librespot resolves
                    // transient AP issues (e.g. TryAnotherAP) internally, so a
                    // PermissionDenied error surfacing here means the login itself failed
                    // (e.g. expired or invalid credentials).
                    if why.kind == ErrorKind::PermissionDenied {
                        return Err(why);
                    }

                    tries += 1;
                    if tries > 3 {
                        error!("Failed to connect to Spirc: {why}");

                        return Err(why);
                    }

                    continue;
                }
            }
        };

        // librespot persists the reusable credentials to the cache on connect, so
        // there is no auth data to hand back to the caller.

        let shutdown = Arc::new(AtomicBool::new(false));
        let (tx, rx) = mpsc::channel(16);
        let player = Self {
            session,
            spirc,
            track,
            stream,

            playback_info: None,
            volume: DEFAULT_VOLUME,
            shuffle: false,
            repeat: RepeatMode::Off,

            events: event_tx.clone(),

            commands: rx,
            spotify_events: rx_player,
            sink_events: rx_sink,

            shutdown: shutdown.clone(),
        };

        // Launch it all!
        tokio::spawn(async move {
            spirc_task.await;

            // If the shutdown flag isn't set, we most likely lost connection to the Spotify AP
            if !shutdown.load(std::sync::atomic::Ordering::SeqCst) {
                _ = event_tx.send(PlayerEvent::ConnectionReset).await;
            }
        });
        tokio::spawn(player.run());

        Ok((PlayerHandle { commands: tx }, event_rx))
    }

    async fn run(mut self) {
        loop {
            tokio::select! {
                opt_command = self.commands.recv() => {
                    let command = match opt_command {
                        Some(command) => command,
                        None => break,
                    };

                    self.handle_command(command).await;
                },

                Some(event) = self.spotify_events.recv() => {
                    self.handle_spotify_event(event).await;
                },

                Some(event) = self.sink_events.recv() => {
                    self.handle_sink_event(event).await;
                }

                else => break,
            }
        }

        self.shutdown
            .store(true, std::sync::atomic::Ordering::SeqCst);

        trace!("End of Player::run");
    }

    async fn handle_command(&mut self, command: PlayerCommand) {
        match command {
            PlayerCommand::NextTrack => _ = self.spirc.next(),
            PlayerCommand::PreviousTrack => _ = self.spirc.prev(),
            PlayerCommand::Pause => _ = self.spirc.pause(),
            PlayerCommand::Play => _ = self.spirc.play(),
            PlayerCommand::SetVolume(volume) => {
                self.volume = volume;
                _ = self.spirc.set_volume(volume);
            }
            PlayerCommand::SetShuffle(shuffle) => {
                self.shuffle = shuffle;
                _ = self.spirc.shuffle(shuffle);
            }
            PlayerCommand::SetRepeat(mode) => {
                self.repeat = mode;
                match mode {
                    RepeatMode::Off => {
                        _ = self.spirc.repeat_track(false);
                        _ = self.spirc.repeat(false);
                    }
                    RepeatMode::Context => {
                        _ = self.spirc.repeat_track(false);
                        _ = self.spirc.repeat(true);
                    }
                    RepeatMode::Track => {
                        _ = self.spirc.repeat_track(true);
                    }
                }
            }

            PlayerCommand::GetPlaybackInfo(tx) => _ = tx.send(self.playback_info.clone()),
            PlayerCommand::GetState(tx) => {
                _ = tx.send(PlayerState {
                    volume: self.volume,
                    shuffle: self.shuffle,
                    repeat: self.repeat,
                })
            }
            PlayerCommand::GetLyrics(tx) => self.get_lyrics(tx).await,
            PlayerCommand::CreateJam(tx) => _ = tx.send(self.create_jam().await),

            PlayerCommand::Shutdown => self.commands.close(),
        };
    }

    async fn handle_spotify_event(&mut self, event: SpotifyPlayerEvent) {
        trace!("Spotify event received: {event:#?}");

        match event {
            SpotifyPlayerEvent::PositionCorrection { position_ms, .. }
            | SpotifyPlayerEvent::Seeked { position_ms, .. } => {
                if let Some(playback_info) = self.playback_info.as_mut() {
                    playback_info.update_playback(position_ms, true);
                }
            }
            SpotifyPlayerEvent::Playing { position_ms, .. } => {
                _ = self.events.send(PlayerEvent::Play).await;

                if let Some(playback_info) = self.playback_info.as_mut() {
                    playback_info.update_playback(position_ms, true);
                }
            }
            SpotifyPlayerEvent::Paused { position_ms, .. } => {
                _ = self.events.send(PlayerEvent::Pause).await;

                if let Some(playback_info) = self.playback_info.as_mut() {
                    playback_info.update_playback(position_ms, false);
                }
            }
            SpotifyPlayerEvent::Stopped { .. } | SpotifyPlayerEvent::SessionDisconnected { .. } => {
                if let Err(why) = self.track.pause() {
                    error!("Failed to pause songbird track: {why}");
                }

                _ = self.events.send(PlayerEvent::Pause).await;

                self.playback_info = None;
            }
            SpotifyPlayerEvent::TrackChanged { audio_item } => {
                if let Some(playback_info) = self.playback_info.as_mut() {
                    playback_info.update_track(*audio_item);
                } else {
                    self.playback_info = Some(PlaybackInfo::new(*audio_item, 0, false));
                }

                _ = self
                    .events
                    .send(PlayerEvent::TrackChanged(Box::new(
                        self.playback_info.clone().expect("playback info is None"),
                    )))
                    .await;
            }
            _ => {}
        }
    }

    async fn handle_sink_event(&self, event: SinkEvent) {
        if let SinkEvent::Start = event {
            if let Err(why) = self.track.play() {
                error!("Failed to resume songbird track: {why}");
            }
        }
    }

    /// Grab the lyrics for the current active track from Spotify.
    ///
    /// This might return None if nothing is being played, or the current song does not have any lyrics.
    async fn get_lyrics(&self, tx: oneshot::Sender<Option<Lyrics>>) {
        let Some(playback_info) = &self.playback_info else {
            _ = tx.send(None);
            return;
        };

        let lyrics = match Lyrics::get(&self.session, &playback_info.track_id()).await {
            Ok(lyrics) => lyrics,
            Err(why) => {
                // Ignore 404 errors
                match why.error.downcast_ref::<HttpClientError>() {
                    Some(HttpClientError::StatusCode(code)) if code.as_u16() == 404 => {}
                    _ => error!("Failed to get lyrics: {why}"),
                }

                _ = tx.send(None);
                return;
            }
        };

        _ = tx.send(Some(lyrics));
    }

    /// Create (or fetch the existing) Spotify Jam for the bot's playback device and
    /// return a shareable join link.
    ///
    /// This uses Spotify's undocumented `social-connect` API, so it may break if
    /// Spotify changes it. On success others can open the link to join and control
    /// playback.
    async fn create_jam(&self) -> Result<String, String> {
        let device_id = self.session.device_id();
        let endpoint =
            format!("/social-connect/v2/sessions/current_or_new?local_device_id={device_id}");

        let bytes = self
            .session
            .spclient()
            .request_as_json(&http::Method::GET, &endpoint, None, None)
            .await
            .map_err(|why| format!("Spotify rejected the Jam request: {why}"))?;

        let json: serde_json::Value = serde_json::from_slice(&bytes)
            .map_err(|why| format!("Could not parse Spotify's Jam response: {why}"))?;

        trace!("Jam response: {json}");

        // The shareable link is https://open.spotify.com/socialsession/{token}.
        // Spotify returns the token either directly as `join_session_token`, or as
        // the last path segment of an internal `hm://.../sessions/join/{token}` URI
        // (exposed as `join_session_url` / `join_session_uri`).
        let token = json
            .get("join_session_token")
            .and_then(|value| value.as_str())
            .map(str::to_owned)
            .or_else(|| {
                json.get("join_session_url")
                    .or_else(|| json.get("join_session_uri"))
                    .and_then(|value| value.as_str())
                    .and_then(|uri| uri.rsplit('/').next())
                    .map(str::to_owned)
            });

        match token {
            Some(token) if !token.is_empty() => {
                Ok(format!("https://open.spotify.com/socialsession/{token}"))
            }
            _ => {
                error!("Unexpected Jam response from Spotify: {json}");
                Err("Spotify did not return a Jam link. Your account may not support Jams."
                    .to_owned())
            }
        }
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        _ = self.spirc.shutdown();
        _ = self.stream.flush();
    }
}

#[derive(Clone, Debug)]
pub struct PlayerHandle {
    commands: mpsc::Sender<PlayerCommand>,
}

impl PlayerHandle {
    pub fn is_valid(&self) -> bool {
        !self.commands.is_closed()
    }

    pub async fn next_track(&self) {
        _ = self.commands.send(PlayerCommand::NextTrack).await;
    }

    pub async fn previous_track(&self) {
        _ = self.commands.send(PlayerCommand::PreviousTrack).await;
    }

    pub async fn pause(&self) {
        _ = self.commands.send(PlayerCommand::Pause).await;
    }

    pub async fn play(&self) {
        _ = self.commands.send(PlayerCommand::Play).await;
    }

    /// Set the Spotify playback volume. `volume` is `0..=u16::MAX`.
    pub async fn set_volume(&self, volume: u16) {
        _ = self.commands.send(PlayerCommand::SetVolume(volume)).await;
    }

    /// Toggle Spotify shuffle mode.
    pub async fn set_shuffle(&self, shuffle: bool) {
        _ = self.commands.send(PlayerCommand::SetShuffle(shuffle)).await;
    }

    /// Set the Spotify repeat mode.
    pub async fn set_repeat(&self, mode: RepeatMode) {
        _ = self.commands.send(PlayerCommand::SetRepeat(mode)).await;
    }

    pub async fn playback_info(&self) -> Result<Option<PlaybackInfo>> {
        let (tx, rx) = oneshot::channel();
        self.commands
            .send(PlayerCommand::GetPlaybackInfo(tx))
            .await?;

        Ok(rx.await?)
    }

    /// Retrieve the current volume and shuffle state.
    pub async fn state(&self) -> Result<PlayerState> {
        let (tx, rx) = oneshot::channel();
        self.commands.send(PlayerCommand::GetState(tx)).await?;

        Ok(rx.await?)
    }

    pub async fn get_lyrics(&self) -> Result<Option<Lyrics>> {
        let (tx, rx) = oneshot::channel();
        self.commands.send(PlayerCommand::GetLyrics(tx)).await?;

        Ok(rx.await?)
    }

    /// Create (or fetch) a Spotify Jam and return a shareable join link.
    ///
    /// The inner `Result` carries a user-facing error message when Spotify
    /// refuses the request (e.g. the account does not support Jams).
    pub async fn create_jam(&self) -> Result<std::result::Result<String, String>> {
        let (tx, rx) = oneshot::channel();
        self.commands.send(PlayerCommand::CreateJam(tx)).await?;

        Ok(rx.await?)
    }

    pub async fn shutdown(&self) {
        _ = self.commands.send(PlayerCommand::Shutdown).await;
    }
}
