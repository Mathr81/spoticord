# Changelog

## 3.3.0 | July 8th 2026

- Added **`/dashboard`**: a rich, auto-updating playback dashboard. It shows the
  track, a **live progress bar that advances on its own** (the embed re-renders
  every few seconds while playing), the album art and the current volume, and
  adds a full row of interactive buttons:
  - ⏮ / Play-Pause / ⏭ media controls,
  - 🔉 / 🔊 volume down/up (±10% per press),
  - 🔀 shuffle and 🔁 repeat, whose **on/off state is shown by button colour**
    (green = on) rather than text — the repeat button also switches between 🔁
    (all) and 🔂 (one),
  - 🎉 Jam, which starts (or fetches) a Spotify Jam and replies ephemerally with
    the join link and a scannable QR code.

  Like `/playing` it accepts an update-behavior option (auto-update, static or
  pinned). The `/playing` embed also gained the smoother, self-advancing progress
  bar.
- Added **`/repeat`** (Off / All / One), mirroring the new dashboard repeat
  button. The player now tracks volume, shuffle and repeat state so the dashboard
  can display it.
- Added **`/play <query>`**: searches Spotify and shows the top results with a row
  of numbered buttons — click one to play it immediately.
- Added **`/queue`**: with a query it searches and shows numbered buttons that add
  a track to the play queue (you can add several from one search); without a query
  it shows the current "up next" list.
- `/jam` (and the dashboard's 🎉 Jam button) now also attaches a **scannable QR
  code** of the Jam link, like the Spotify app's share screen.

  Search, queueing and playback are driven through Spotify's Web API. The token is
  obtained via librespot's **login5** flow — Spotify has disabled the legacy
  keymaster token endpoint (it now returns `403 Invalid request`), so the older
  `token_provider` path no longer works. These features require the bot to be the
  active Spotify Connect device (i.e. selected as the playback target), which is
  the normal way Spoticord is used.
- CI: removed the redundant `build.yml` workflow. It ran on both `push` and
  `pull_request` (so it fired twice per PR push) and only re-compiled what the
  Docker image build already does, producing a binary artifact that isn't used
  now that deployment is via the published image. Linting (`cargo-clippy`) and the
  Docker build (`build-push`) remain.

## 3.2.0 | July 7th 2026

- Bumped Spotify streaming quality from 160kbps to **320kbps** (the highest
  Spotify offers; Premium only). Note the audio Discord actually receives is
  still capped by the voice channel's bitrate.
- Added **`/jam`**: starts (or fetches) a Spotify Jam for the bot's device and
  returns a shareable join link, so anyone can hop in and control the music.
  Uses Spotify's undocumented `social-connect` API.
- Added **`/volume`** (0-100%) and **`/shuffle`** (on/off), both synced with
  Spotify via the Connect controls.

## 3.1.0 | July 7th 2026

- Upgraded `librespot` from 0.5.0 to 0.8.0. 0.5.0 is too old for Spotify's
  current backend, which caused the bot to connect and accept transport
  controls but play no audio (upstream fixed this by switching to
  `get_extended_metadata` for audio files, acquiring HTTP access tokens via
  `login5`, and falling back across CDN URLs). This is the change that makes
  playback actually work.
- Migrated the affected API surface to librespot 0.8:
  - `connect::{ConnectConfig, Spirc}` import paths; `ConnectConfig.initial_volume`
    is now a plain `u16`.
  - The mixer factory now returns a `Result`.
  - `AudioItem.track_id` is now a `SpotifyUri` (was `SpotifyId`); handle the new
    `UniqueFields::Local` variant and use `SpotifyUri::to_id()` for base62 ids.
  - OAuth login moved to `OAuthClientBuilder` (the old `get_access_token` is
    deprecated).
- Enabled librespot's `rustls-tls-webpki-roots` feature (0.8 requires an explicit
  TLS backend when default features are off).

## 3.0.0 | July 7th 2026

Stripped Spoticord down to a **single-account, self-hostable** build for personal
use. This is a breaking change: the multi-user model is gone.

- Removed the `spoticord_database` crate and the entire PostgreSQL dependency.
  There is no more per-user account storage.
- Removed the `spoticord_stats` crate and its Redis dependency.
- Removed the `/link`, `/unlink` and `/rename` commands and the dependency on the
  external `spoticord-link` frontend. Also removed the `/token` debug command.
- The bot now plays from a single Spotify account. Credentials are obtained once
  through librespot's interactive OAuth flow (an authorization URL is printed on
  first launch) and cached to disk (`CACHE_DIR`, default `/data`), so subsequent
  launches sign in automatically.
- New configuration: only `DISCORD_TOKEN` is required; `DEVICE_NAME` and
  `CACHE_DIR` are optional. Removed `DATABASE_URL`, `LINK_URL`, `KV_URL`,
  `SPOTIFY_CLIENT_ID` and `SPOTIFY_CLIENT_SECRET`.
- Simplified the Dockerfile (no more libpq / PostgreSQL cross-compilation) and
  the `docker-compose.yml` (just the bot plus a volume for the credentials cache).

## 2.3.0 | July 6th 2026

- Updated `songbird` from 0.4.4 to 0.6.0, adding support for Discord's mandatory
  DAVE (Audio & Video End-to-End Encryption) protocol. This is the change that
  allows Spoticord to participate in voice calls again after Discord made DAVE
  mandatory on March 1st 2026.
- Removed the now-nonexistent `simd-json` feature from the `songbird` dependency
  (its JSON backend is no longer feature-gated in 0.6.0).
- Bumped the Docker builder image to `rust:1.94-slim` to satisfy `songbird`
  0.6.0's minimum supported Rust version (1.83.0).
- Bumped project MSRV to 1.83.0.
- Switched the `librespot` dependency from the (no longer reachable)
  `SpoticordMusic/librespot` git fork to the upstream `librespot` 0.5.0 crate
  published on crates.io. This unblocks builds, which were failing because the
  fork could no longer be cloned. The only source change required was replacing
  two references to librespot's now-private `core::connection` module with a
  check on `librespot::core::error::ErrorKind::PermissionDenied` (librespot
  handles transient AP errors such as `TryAnotherAP` internally, so a
  `PermissionDenied` error surfacing to Spoticord means the login itself failed).
- Committed a fully crates.io-resolvable `Cargo.lock` (with `vergen` pinned to
  9.0.6 to avoid a broken `vergen`/`vergen-lib` build-script combination).
- Added a `docker-compose.yml` and documented `.env.example` for self-hosting
  (bot + PostgreSQL + Redis), and made the `build.yml` workflow upload the
  compiled release binary as a downloadable artifact.

## 2.2.6 | November 13th 2024

- Updated voice module to support Discord's new mandatory voice encryption

## 2.2.5 | October 18th 2024

- Updated librespot to rel 0.5.0 (was: 0.5.0-dev)
- Fixed an issue where Spoticord would lose connection to Spotify servers (fixed by librespot upgrade)
- Reworked authentication logic, hopefully reducing the amount of "suspicious login" forced password resets

## 2.2.4 | September 30th 2024

- Added a message for if the Spotify AP connection drops
- Added additional timeouts to credential retrieval
- Removed multiple points of failure in `librespot` that could shut down the bot
- Fixed an issue where non-premium users could crash the bot for everyone (See point 3)

## 2.2.3 | September 20th 2024

- Made backend changes to librespot to prevent deadlocking by not waiting for thread shutdowns
- Added retrying to Spotify login logic to reduce the chance of the bot failing to connect

## 2.2.2 | September 2nd 2024

- Added backtrace logging to player creation to figure out some mystery crashes

## 2.2.1 | August 22nd 2024

- Added new option: `/playing` can now receive an updating behavior parameter
- Added album name to `/playing` embed
- Fixed a bug where uncached guilds would panic the bot
- Fixed small issue with embed styling
- Updated to Rust 1.80.1 (from 1.79.0)
- Updated `diesel` and addons to latest versions
- Removed `lazy_static` in favor of `LazyLock` (Rust 1.80.0+ feature)
- Bumped MSRV to 1.80.0 due to the introduction of `LazyLock`

## 2.2.0 | August 13th 2024

### Changes

- Rewrote the entire bot (again)
- Updated librespot from v0.4.2 to v0.5.0-dev
- Added `/lyrics`, which provides the user with an auto-updating lyrics embed
- Added `/stop`, which disconnects the bot from Spotify without leaving the call (will still leave after 5 minutes)
- Changed `/playing` to automatically update the embed accordingly
- Renamed `/leave` to `/disconnect`
- Removed the Database API, replaced with direct connection to a Postgres database

**Full Changelog** (good luck): https://github.com/SpoticordMusic/spoticord/compare/v2.1.2..v2.2.0

## 2.1.2 | September 28th 2023

### Changes

- Removed OpenSSL dependency
- Added aarch64 support
- Added cross compilation to Github Actions
- Added `dev` branch to Github Actions
- Removed hardcoded URL in the /join command
- Fixed an issue in /playing where the bot showed it was playing even though it was paused

**Full Changelog**: https://github.com/SpoticordMusic/spoticord/compare/v2.1.1...v2.1.2

## 2.1.1 | September 23rd 2023

Reduced the amount of CPU that the bot uses from ~15%-25% per user to 1%-2% per user (percentage per core, benched on an AMD Ryzen 9 5950X).

### Changes

- Fixed issue #20

**Full Changelog**: https://github.com/SpoticordMusic/spoticord/compare/v2.1.0...v2.1.1

## 2.1.0 | September 20th 2023

So, it's been a while since I worked on this project, and some bugs have since been discovered.
The main focus for this version is to stop using multiple processes for every player, and instead do everything in threads.

### Changes

- Remove metrics, as I wasn't using this feature anyways
- Bring back KV for storing total/active sessions, as prometheus is no longer being used
- Allocate new players in-memory, instead of using subprocesses
- Fix issue #17
- Fix some issues with the auto-disconnect
- Removed the automatic device switching on bot join, which was causing some people to not be able to use the bot
- Force communication through the closest Spotify AP, reducing latency
- Potential jitter reduction
- Enable autoplay
- After skipping a song, you will no longer hear a tiny bit of the previous song after the silence

**Full Changelog**: https://github.com/SpoticordMusic/spoticord/compare/v2.0.0...v2.1.0

### Issues

- Currently, the CPU usage is much higher than it used to be. I really wanted to push this update out before taking the time to do some optimizations, as the bot and server are still easily able to hold up the limited amount of Spoticord users (and v2.0.0 was just falling apart). Issue is being tracked in #20

## 2.0.0 | June 8th 2023

- Initial Release
