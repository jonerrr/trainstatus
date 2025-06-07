use super::bus::{DecodeError, de_remove_underscore_prefix};
use crate::api_key;
use chrono::{DateTime, Utc};
use serde::Deserialize;

pub async fn decode() -> Result<VehicleMonitoringDelivery, DecodeError> {
    let siri_res = reqwest::Client::new()
        .get("https://api.prod.obanyc.com/api/siri/vehicle-monitoring.json")
        .query(&[("key", api_key()), ("version", "2")])
        .send()
        .await?
        .json::<serde_json::Value>()
        .await?;

    // error is usually due to 30s timeout
    let service_delivery: ServiceDelivery =
        serde_json::from_value(siri_res["Siri"]["ServiceDelivery"].to_owned())?;

    service_delivery
        .vehicle_monitoring_delivery
        .into_iter()
        .next()
        .ok_or(DecodeError::NoVehicles)
}

// let mut progresses = Vec::new();
// // let mut statuses = Vec::new();
// for vehicle_monitoring_delivery in service_delivery.vehicle_monitoring_delivery {
//     for vehicle_activity in vehicle_monitoring_delivery.vehicle_activity {
//         let monitored_vehicle_journey = vehicle_activity.monitored_vehicle_journey;
//         let progress_rate = monitored_vehicle_journey
//             .progress_status
//             .and_then(|s| s.into_iter().nth(0));

//         // if progress_rate == "noProgress" {
//         //     dbg!(&monitored_vehicle_journey.progress_status);
//         // }

//         if !progresses.contains(&progress_rate) {
//             progresses.push(progress_rate.clone());
//         }

//         // monitored_vehicle_journey.progress_status.map(|status| {
//         //     for s in status {
//         //         if s == "layover" {
//         //             if progress_rate != "noProgress" {
//         //                 println!("layover with progress rate: {}", progress_rate);
//         //             }
//         //         } else if s == "spooking" {
//         //             if progress_rate != "unknown" {
//         //                 println!("spooking without unknown: {}", progress_rate);
//         //             }
//         //         }

//         //         if !statuses.contains(&s) {
//         //             statuses.push(s);
//         //         }
//         //     }
//         // });
//     }
// }

// println!("Unique Progress Rates: {:?}", progresses);
// // println!("Unique Progress Statuses: {:?}", statuses);

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct ServiceDelivery {
    vehicle_monitoring_delivery: Vec<VehicleMonitoringDelivery>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct VehicleMonitoringDelivery {
    pub vehicle_activity: Vec<VehicleActivity>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct VehicleActivity {
    pub monitored_vehicle_journey: MonitoredVehicleJourney,
    pub recorded_at_time: DateTime<Utc>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct MonitoredVehicleJourney {
    // #[serde(deserialize_with = "de_remove_underscore_prefix")]
    // line_ref: String,
    // #[serde(deserialize_with = "de_str_to_i16")]
    // direction_ref: i16,
    // pub framed_vehicle_journey_ref: JourneyRef,
    // should be only 1 in vec
    // published_line_name: Vec<String>,
    #[serde(deserialize_with = "de_remove_underscore_prefix")]
    pub vehicle_ref: String,
    // progress_rate: String,
    // pub progress_status: Option<Vec<String>>,
    // pub monitored_call: Option<MonitoredCall>,
}

// #[derive(Deserialize)]
// #[serde(rename_all = "PascalCase")]
// pub struct JourneyRef {
//     // data_frame_ref: chrono::NaiveDate,
//     #[serde(deserialize_with = "de_remove_underscore_prefix")]
//     pub dated_vehicle_journey_ref: String,
// }

// #[derive(Deserialize)]
// #[serde(rename_all = "PascalCase")]
// pub struct MonitoredCall {
//     pub extensions: Option<Extensions>,
// }

// #[derive(Deserialize)]
// #[serde(rename_all = "PascalCase")]
// pub struct Extensions {
//     pub capacities: Capacities,
// }

// #[derive(Deserialize)]
// #[serde(rename_all = "PascalCase")]
// pub struct Capacities {
//     pub estimated_passenger_count: i32,
//     pub estimated_passenger_capacity: i32,
// }
