use crate::debug_rt_data;
use anyhow::bail;
use serde::{Deserialize, Serialize};
use tokio::fs::{create_dir, write};
use tracing::error;
// TODO: combine other oba fetchers (like static mta bus data) into a common module

/// Generic OBA API fetch function that works with any OneBusAway-compatible API
pub async fn fetch_vehicles(url: &str, api_key: &str) -> anyhow::Result<Vec<VehicleStatus>> {
    let res = reqwest::Client::new()
        .get(url)
        .query(&[("key", api_key)])
        .send()
        .await?
        .json::<Response>()
        .await?;

    if res.data.limit_exceeded {
        bail!("OBA API limit exceeded");
    }
    if res.data.out_of_range {
        bail!("OBA API request out of range");
    }
    // TODO: put gtfs and oba data in one common directory (and maybe common format)
    if *debug_rt_data() {
        create_dir("./oba").await.ok();

        let json_path = "./oba/vehicles.json";
        let json_str = serde_json::to_string_pretty(&res)?;
        if let Err(e) = write(json_path, json_str).await {
            error!(json_path, %e, "Failed to write OBA debug output");
        }
    }

    Ok(res.data.list)
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Response {
    // pub code: u16,
    // pub text: String,
    // #[serde(with = "chrono::serde::ts_milliseconds")]
    // pub current_time: DateTime<Utc>,
    pub data: VehicleData,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VehicleData {
    pub limit_exceeded: bool,
    pub out_of_range: bool,
    pub list: Vec<VehicleStatus>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VehicleStatus {
    // last_location_update_time is always the same as last_update_time
    // #[serde(with = "chrono::serde::ts_milliseconds")]
    // pub last_location_update_time: DateTime<Utc>,
    // #[serde(with = "chrono::serde::ts_milliseconds")]
    // pub last_update_time: DateTime<Utc>,
    // pub location: Location,
    pub occupancy_capacity: Option<i32>,
    pub occupancy_count: Option<i32>,
    // pub occupancy_status: Option<i32>,
    pub phase: String,
    pub status: String,
    pub trip_id: Option<String>,
    pub vehicle_id: String,
}
