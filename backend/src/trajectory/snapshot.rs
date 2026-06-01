use crate::models::geom::Geom;
use crate::models::position::VehiclePosition;
use crate::models::trip::StopTimeData;
use crate::models::{source::Source, stop::StopData};
use crate::stores::trip::TrajectoryInputRow;

use super::geometry::{geom_to_linestring, shape_key_from_line};
use super::types::{TrajectoryStop, TripSnapshot};

pub fn snapshot_from_rows(
    trip_id: uuid::Uuid,
    route_id: String,
    route_color: String,
    direction: i16,
    trip_geom_raw: Geom,
    shape_length_m: f64,
    rows: Vec<TrajectoryInputRow>,
    positions: Vec<VehiclePosition>,
    as_of: chrono::DateTime<chrono::Utc>,
) -> anyhow::Result<TripSnapshot> {
    let shape = match geom_to_linestring(&trip_geom_raw) {
        Some(ls) => ls,
        None => return Err(anyhow::anyhow!("Trip geometry is not a LineString")),
    };
    let shape_key = shape_key_from_line(&shape);

    let consist_length_m = rows.iter().find_map(|row| {
        row.car_count.and_then(|count| {
            row.car_length_feet
                .map(|length| count as f64 * length as f64 * 0.3048)
        })
    });
    let consist_car_count = rows
        .iter()
        .find_map(|row| row.car_count.map(|count| count as i16));
    let consist_car_length_m = rows
        .iter()
        .find_map(|row| row.car_length_feet.map(|length| length as f32 * 0.3048));

    let mut stops = Vec::new();
    for row in rows {
        let stop_data: StopData = serde_json::from_value(row.stop_data)
            .map_err(|e| anyhow::anyhow!("Failed to deserialize stop data: {e}"))?;
        let stop_time_data: StopTimeData = serde_json::from_value(row.stop_time_data)
            .map_err(|e| anyhow::anyhow!("Failed to deserialize stop_time data: {e}"))?;
        stops.push(TrajectoryStop {
            stop_id: row.stop_id,
            arrival_unix: row.arrival_unix,
            departure_unix: row.departure_unix,
            stop_distance_m: row.stop_distance_m,
            stop_time_data,
            stop_data,
        });
    }

    Ok(TripSnapshot {
        trip_id,
        route_id,
        route_color,
        direction,
        shape,
        shape_key,
        shape_length_m,
        consist_length_m,
        consist_car_count,
        consist_car_length_m,
        stops,
        positions,
        as_of,
    })
}

pub fn source_supports_trajectories(source: Source) -> bool {
    matches!(source, Source::MtaSubway | Source::MtaBus | Source::NjtBus)
}
