//! A small Spotify Web API client for search and queue control.
//!
//! librespot's own token only works against Spotify's *internal* API; the public
//! Web API (`api.spotify.com`) rejects it (403/429). So this client uses your own
//! Spotify Developer app: it runs a one-time Authorization Code login, caches the
//! resulting user token (with its refresh token) to disk, and refreshes it as
//! needed. It talks HTTP through librespot's `HttpClient` to reuse the same TLS
//! stack the rest of the bot already uses.

use std::{
    path::PathBuf,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{anyhow, bail, Context, Result};
use base64::Engine;
use bytes::Bytes;
use http::{Method, Request};
use librespot::core::http_client::HttpClient;
use log::{info, warn};
use serde::{Deserialize, Serialize};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    sync::Mutex,
};
use url::Url;

const AUTH_URL: &str = "https://accounts.spotify.com/authorize";
const TOKEN_URL: &str = "https://accounts.spotify.com/api/token";
const API_BASE: &str = "https://api.spotify.com/v1";
const SCOPES: &str = "user-modify-playback-state user-read-playback-state";

/// A single track returned by a search or found in the play queue.
#[derive(Clone, Debug)]
pub struct TrackResult {
    pub name: String,
    pub artists: String,
    pub album: String,
    pub uri: String,
    pub duration_ms: u32,
}

#[derive(Serialize, Deserialize, Clone)]
struct CachedToken {
    access_token: String,
    refresh_token: String,
    /// Unix timestamp (seconds) at which `access_token` expires.
    expires_at: u64,
}

/// Authenticated Spotify Web API client.
pub struct WebApi {
    client_id: String,
    client_secret: String,
    http: HttpClient,
    cache_path: PathBuf,
    token: Mutex<CachedToken>,
}

impl WebApi {
    /// Initialise the client: load the cached token, or run the one-time
    /// interactive authorization if there isn't one yet.
    pub async fn init(
        client_id: String,
        client_secret: String,
        redirect_uri: String,
        cache_dir: PathBuf,
    ) -> Result<Arc<WebApi>> {
        let http = HttpClient::new(None);
        let cache_path = cache_dir.join("webapi_token.json");

        let token = match load_token(&cache_path) {
            Some(token) => {
                info!("Loaded cached Spotify Web API token");
                token
            }
            None => {
                warn!("No cached Spotify Web API token found, starting authorization.");
                let code = authorize(&client_id, &redirect_uri).await?;
                let token =
                    exchange_code(&http, &client_id, &client_secret, &code, &redirect_uri).await?;
                save_token(&cache_path, &token)?;
                info!("Spotify Web API authorized and token cached");
                token
            }
        };

        Ok(Arc::new(WebApi {
            client_id,
            client_secret,
            http,
            cache_path,
            token: Mutex::new(token),
        }))
    }

    /// Return a valid access token, refreshing it first if it's about to expire.
    async fn access_token(&self) -> Result<String> {
        let mut token = self.token.lock().await;

        if now() + 60 < token.expires_at {
            return Ok(token.access_token.clone());
        }

        let refreshed = refresh(
            &self.http,
            &self.client_id,
            &self.client_secret,
            &token.refresh_token,
        )
        .await
        .context("failed to refresh the Spotify Web API token")?;

        *token = refreshed;
        save_token(&self.cache_path, &token)?;

        Ok(token.access_token.clone())
    }

    /// Search Spotify for tracks matching `query` (top 5 results).
    pub async fn search(&self, query: &str) -> Result<Vec<TrackResult>> {
        let url = format!(
            "{API_BASE}/search?type=track&limit=5&q={}",
            urlencoding::encode(query)
        );

        let json = self.request_json(Method::GET, &url).await?;
        let items = json
            .get("tracks")
            .and_then(|tracks| tracks.get("items"))
            .and_then(|items| items.as_array())
            .ok_or_else(|| anyhow!("unexpected search response from Spotify"))?;

        Ok(items.iter().filter_map(parse_track).collect())
    }

    /// Add a track to the play queue by its `spotify:track:...` URI.
    pub async fn add_to_queue(&self, uri: &str) -> Result<()> {
        let url = format!(
            "{API_BASE}/me/player/queue?uri={}",
            urlencoding::encode(uri)
        );

        self.request_json(Method::POST, &url).await?;

        Ok(())
    }

    /// Fetch the upcoming tracks in the play queue.
    pub async fn get_queue(&self) -> Result<Vec<TrackResult>> {
        let json = self
            .request_json(Method::GET, &format!("{API_BASE}/me/player/queue"))
            .await?;

        let queue = json
            .get("queue")
            .and_then(|queue| queue.as_array())
            .ok_or_else(|| anyhow!("unexpected queue response from Spotify"))?;

        Ok(queue.iter().filter_map(parse_track).collect())
    }

    /// Perform an authenticated Web API request and parse the JSON body (or
    /// `Null` for empty `204` responses).
    async fn request_json(&self, method: Method, url: &str) -> Result<serde_json::Value> {
        let token = self.access_token().await?;

        let request = Request::builder()
            .method(method)
            .uri(url)
            .header("Authorization", format!("Bearer {token}"))
            .body(Bytes::new())?;

        let bytes = self
            .http
            .request_body(request)
            .await
            .map_err(|why| anyhow!("Spotify API request failed: {why}"))?;

        if bytes.is_empty() {
            return Ok(serde_json::Value::Null);
        }

        Ok(serde_json::from_slice(&bytes)?)
    }
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn load_token(path: &PathBuf) -> Option<CachedToken> {
    let data = std::fs::read(path).ok()?;
    serde_json::from_slice(&data).ok()
}

fn save_token(path: &PathBuf, token: &CachedToken) -> Result<()> {
    let data = serde_json::to_vec_pretty(token)?;
    std::fs::write(path, data).context("failed to write the Web API token cache")?;
    Ok(())
}

/// Run the one-time interactive authorization: print the consent URL, wait on the
/// local redirect for Spotify to hand back the authorization code.
async fn authorize(client_id: &str, redirect_uri: &str) -> Result<String> {
    let url = format!(
        "{AUTH_URL}?client_id={}&response_type=code&redirect_uri={}&scope={}",
        urlencoding::encode(client_id),
        urlencoding::encode(redirect_uri),
        urlencoding::encode(SCOPES),
    );

    let port = Url::parse(redirect_uri)
        .ok()
        .and_then(|parsed| parsed.port())
        .ok_or_else(|| anyhow!("SPOTIFY_REDIRECT_URI must include a port, e.g. :8898"))?;

    warn!("Open this URL in a browser to authorize Spotify Web API access (search / queue):");
    warn!("{url}");

    wait_for_code(port).await
}

/// Accept a single request on the redirect port and extract the `code`.
async fn wait_for_code(port: u16) -> Result<String> {
    let listener = TcpListener::bind(("0.0.0.0", port))
        .await
        .with_context(|| format!("failed to bind the redirect port {port}"))?;

    loop {
        let (mut stream, _) = listener.accept().await?;

        let mut buffer = vec![0u8; 4096];
        let read = stream.read(&mut buffer).await?;
        let request = String::from_utf8_lossy(&buffer[..read]);

        let path = request
            .lines()
            .next()
            .and_then(|line| line.split_whitespace().nth(1))
            .unwrap_or("");

        let Some(query) = path.split_once('?').map(|(_, query)| query) else {
            // Not the redirect (e.g. a favicon request): acknowledge and keep waiting.
            _ = stream
                .write_all(
                    b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                )
                .await;
            continue;
        };

        let mut code = None;
        let mut error = None;
        for pair in query.split('&') {
            match pair.split_once('=') {
                Some(("code", value)) => {
                    code = Some(urlencoding::decode(value)?.into_owned());
                }
                Some(("error", value)) => {
                    error = Some(urlencoding::decode(value)?.into_owned());
                }
                _ => {}
            }
        }

        let body = "<html><body><h2>Spotify authorization complete.</h2>You can close this tab.</body></html>";
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        _ = stream.write_all(response.as_bytes()).await;

        if let Some(error) = error {
            bail!("Spotify authorization was denied: {error}");
        }

        if let Some(code) = code {
            return Ok(code);
        }
    }
}

async fn exchange_code(
    http: &HttpClient,
    client_id: &str,
    client_secret: &str,
    code: &str,
    redirect_uri: &str,
) -> Result<CachedToken> {
    let body = format!(
        "grant_type=authorization_code&code={}&redirect_uri={}",
        urlencoding::encode(code),
        urlencoding::encode(redirect_uri),
    );

    token_request(http, client_id, client_secret, body, None).await
}

async fn refresh(
    http: &HttpClient,
    client_id: &str,
    client_secret: &str,
    refresh_token: &str,
) -> Result<CachedToken> {
    let body = format!(
        "grant_type=refresh_token&refresh_token={}",
        urlencoding::encode(refresh_token),
    );

    token_request(http, client_id, client_secret, body, Some(refresh_token)).await
}

/// POST to the token endpoint with HTTP Basic client authentication and parse the
/// returned token. `existing_refresh` is kept when the response omits a new one
/// (as refresh responses do).
async fn token_request(
    http: &HttpClient,
    client_id: &str,
    client_secret: &str,
    body: String,
    existing_refresh: Option<&str>,
) -> Result<CachedToken> {
    let basic =
        base64::engine::general_purpose::STANDARD.encode(format!("{client_id}:{client_secret}"));

    let request = Request::builder()
        .method(Method::POST)
        .uri(TOKEN_URL)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .header("Authorization", format!("Basic {basic}"))
        .body(Bytes::from(body))?;

    let bytes = http
        .request_body(request)
        .await
        .map_err(|why| anyhow!("Spotify token request failed: {why}"))?;

    let json: serde_json::Value = serde_json::from_slice(&bytes)?;

    let access_token = json
        .get("access_token")
        .and_then(|value| value.as_str())
        .ok_or_else(|| anyhow!("Spotify token response had no access_token: {json}"))?
        .to_owned();

    let expires_in = json
        .get("expires_in")
        .and_then(|value| value.as_u64())
        .unwrap_or(3600);

    let refresh_token = json
        .get("refresh_token")
        .and_then(|value| value.as_str())
        .map(str::to_owned)
        .or_else(|| existing_refresh.map(str::to_owned))
        .ok_or_else(|| anyhow!("Spotify token response had no refresh_token"))?;

    Ok(CachedToken {
        access_token,
        refresh_token,
        expires_at: now() + expires_in,
    })
}

/// Parse a Spotify Web API track object into a [`TrackResult`].
fn parse_track(item: &serde_json::Value) -> Option<TrackResult> {
    let name = item.get("name")?.as_str()?.to_owned();
    let uri = item.get("uri")?.as_str()?.to_owned();

    let artists = item
        .get("artists")?
        .as_array()?
        .iter()
        .filter_map(|artist| artist.get("name").and_then(|name| name.as_str()))
        .collect::<Vec<_>>()
        .join(", ");

    let album = item
        .get("album")
        .and_then(|album| album.get("name"))
        .and_then(|name| name.as_str())
        .unwrap_or_default()
        .to_owned();

    let duration_ms = item
        .get("duration_ms")
        .and_then(|duration| duration.as_u64())
        .unwrap_or(0) as u32;

    Some(TrackResult {
        name,
        artists,
        album,
        uri,
        duration_ms,
    })
}
