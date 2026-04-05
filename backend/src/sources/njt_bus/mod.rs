pub mod alerts;
pub mod realtime;
pub mod static_data;

use anyhow::Context;
use std::sync::OnceLock;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

use crate::integrations::gtfs_realtime::FeedFuture;

pub(super) const NJT_GTFS_AUTH_URL: &str =
    "https://pcsdata.njtransit.com/api/GTFSG2/authenticateUser";
pub(super) const NJT_BUSDV2_AUTH_URL: &str =
    "https://pcsdata.njtransit.com/api/BUSDV2/authenticateUser";
pub(super) const NJT_TRIP_UPDATES_URL: &str =
    "https://pcsdata.njtransit.com/api/GTFSG2/getTripUpdates";
pub(super) const NJT_VEHICLE_POSITIONS_URL: &str =
    "https://pcsdata.njtransit.com/api/GTFSG2/getVehiclePositions";
pub(super) const NJT_ALERTS_URL: &str = "https://pcsdata.njtransit.com/api/GTFSG2/getAlerts";
pub(super) const NJT_BUS_ROUTES_URL: &str = "https://pcsdata.njtransit.com/api/BUSDV2/getBusRoutes";

#[derive(Clone, Copy, Debug)]
pub(super) enum NjtApi {
    GtfsG2,
    BusDv2,
}

impl NjtApi {
    fn auth_url(self) -> &'static str {
        match self {
            Self::GtfsG2 => NJT_GTFS_AUTH_URL,
            Self::BusDv2 => NJT_BUSDV2_AUTH_URL,
        }
    }
}

#[derive(serde::Deserialize)]
struct AuthResponse {
    #[serde(rename = "UserToken")]
    user_token: String,
}

/// Cached (token, acquired_at). Refreshed after 23 hours.
static NJT_GTFS_TOKEN: OnceLock<Mutex<Option<(String, Instant)>>> = OnceLock::new();
static NJT_BUSDV2_TOKEN: OnceLock<Mutex<Option<(String, Instant)>>> = OnceLock::new();

/// Returns a valid NJT API token, re-authenticating if the cached one is stale.
pub(super) async fn get_token(api: NjtApi) -> anyhow::Result<String> {
    let mutex = match api {
        NjtApi::GtfsG2 => NJT_GTFS_TOKEN.get_or_init(|| Mutex::new(None)),
        NjtApi::BusDv2 => NJT_BUSDV2_TOKEN.get_or_init(|| Mutex::new(None)),
    };
    let mut guard = mutex.lock().await;

    if let Some((ref token, acquired_at)) = *guard
        && acquired_at.elapsed() < Duration::from_secs(23 * 60 * 60) {
            return Ok(token.clone());
        }

    let token = authenticate(api)
        .await
        .context("NJT re-authentication failed")?;
    *guard = Some((token.clone(), Instant::now()));
    Ok(token)
}

async fn authenticate(api: NjtApi) -> anyhow::Result<String> {
    let username = std::env::var("NJT_USERNAME").context("NJT_USERNAME env var not set")?;
    let password = std::env::var("NJT_PASSWORD").context("NJT_PASSWORD env var not set")?;

    let form = reqwest::multipart::Form::new()
        .text("username", username)
        .text("password", password);

    let resp: AuthResponse = reqwest::Client::new()
        .post(api.auth_url())
        .multipart(form)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    Ok(resp.user_token)
}

/// Returns a [`FeedFuture`] that POSTs to an NJT GTFS-RT endpoint with a form token.
pub(super) fn njt_post_future(url: &'static str, token: String) -> FeedFuture {
    Box::pin(async move {
        let form = reqwest::multipart::Form::new().text("token", token);
        Ok(reqwest::Client::new()
            .post(url)
            .multipart(form)
            .send()
            .await?
            .error_for_status()?
            .bytes()
            .await?)
    })
}
