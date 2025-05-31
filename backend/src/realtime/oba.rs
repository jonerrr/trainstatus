use super::bus::{DecodeError, de_remove_underscore_prefix};
use crate::api_key;
use chrono::{DateTime, Utc};
use serde::Deserialize;

const AGENCIES: [&str; 2] = ["MTABC", "MTA NYCT"];

pub async fn decode() -> Result<Vec<VehicleStatus>, DecodeError> {
    let mut vehicles = vec![];
    for agency in AGENCIES {
        let res = reqwest::Client::new()
            .get(format!(
                "https://bustime.mta.info/api/where/vehicles-for-agency/{}.json",
                agency
            ))
            .query(&[("key", api_key())])
            .send()
            .await?
            .json::<AgencyVehicleResponse>()
            .await?;

        if res.data.limit_exceeded {
            return Err(DecodeError::LimitExceeded);
        }
        if res.data.out_of_range {
            return Err(DecodeError::OutOfRange);
        }
        // TODO: maybe check text is "OK" or if code is 200

        vehicles.extend(res.data.list);
    }

    // might not need to check this
    if vehicles.is_empty() {
        return Err(DecodeError::NoVehicles);
    }

    Ok(vehicles)
}

// pub async fn insert(
//     pool: &sqlx::PgPool,
//     vehicles: Vec<VehicleStatus>,
// ) -> Result<(), DecodeError> {
//     if vehicles.is_empty() {
//         tracing::warn!("No vehicles to insert");
//         return Ok(());
//     }

//     Ok(())
// }

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgencyVehicleResponse {
    // pub code: u16,
    // pub text: String,
    // #[serde(with = "chrono::serde::ts_milliseconds")]
    // pub current_time: DateTime<Utc>,
    pub data: VehicleData,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VehicleData {
    pub limit_exceeded: bool,
    pub out_of_range: bool,
    pub list: Vec<VehicleStatus>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VehicleStatus {
    // last_location_update_time is always the same as last_update_time
    // #[serde(with = "chrono::serde::ts_milliseconds")]
    // pub last_location_update_time: DateTime<Utc>,
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub last_update_time: DateTime<Utc>,
    // pub location: Location,
    pub occupancy_capacity: Option<i32>,
    pub occupancy_count: Option<i32>,
    // pub occupancy_status: Option<i32>,
    pub phase: String,
    // pub status: String,
    #[serde(deserialize_with = "de_trip_id")]
    pub trip_id: Option<String>,
    #[serde(deserialize_with = "de_remove_underscore_prefix")]
    pub vehicle_id: String,
}

fn de_trip_id<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s: Option<String> = Option::deserialize(deserializer)?;
    match s {
        Some(val) => {
            let trimmed_val = val.trim();
            if trimmed_val.is_empty() {
                Ok(None)
            } else {
                Ok(Some(
                    trimmed_val
                        .split_once('_')
                        .map(|(first, _)| first)
                        .unwrap_or(trimmed_val)
                        .to_string(),
                ))
            }
        }
        None => Ok(None),
    }
}

// #[derive(Debug, Deserialize)]
// #[serde(rename_all = "camelCase")]
// pub struct Location {
//     pub lat: f64,
//     pub lon: f64,
// }
