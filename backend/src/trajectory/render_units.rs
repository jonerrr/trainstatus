use crate::models::position::PositionData;
use crate::models::source::Source;

use super::geometry::{ShapeGeometry, bearing_at_distance, distance_to_coord};
use super::types::{RenderUnit, Trajectory, TripSnapshot, compute_path_bbox};

const DEFAULT_SUBWAY_CAR_LENGTH_M: f32 = 18.288;
const DEFAULT_BUS_LENGTH_M: f32 = 12.0;

#[derive(Clone, Copy)]
pub struct SourceRenderConfig {
    pub primary_icon_key: &'static str,
    pub trailing_icon_key: &'static str,
    pub expand_consists: bool,
    pub default_length_m: f32,
}

pub fn source_render_config(source: Source) -> SourceRenderConfig {
    match source {
        Source::MtaSubway => SourceRenderConfig {
            primary_icon_key: "rail_head",
            trailing_icon_key: "rail_car",
            expand_consists: true,
            default_length_m: DEFAULT_SUBWAY_CAR_LENGTH_M,
        },
        Source::MtaBus | Source::NjtBus => SourceRenderConfig {
            primary_icon_key: "bus",
            trailing_icon_key: "bus",
            expand_consists: false,
            default_length_m: DEFAULT_BUS_LENGTH_M,
        },
    }
}

pub fn expand_render_units(
    source: Source,
    trip: &TripSnapshot,
    trajectory: &Trajectory,
    shape_geom: &ShapeGeometry,
) -> Vec<RenderUnit> {
    let config = source_render_config(source);
    let passengers = trip_passengers(trip);

    if config.expand_consists {
        if let (Some(unit_count), Some(car_length_m)) =
            (trip.consist_car_count, trip.consist_car_length_m)
        {
            let expanded = expand_consist_units(
                source,
                trip,
                trajectory,
                shape_geom,
                unit_count,
                car_length_m,
                passengers,
                config,
            );
            if !expanded.is_empty() {
                return expanded;
            }
        }
    }

    build_single_unit(source, trip, trajectory, passengers, config)
        .into_iter()
        .collect()
}

fn expand_consist_units(
    source: Source,
    _trip: &TripSnapshot,
    trajectory: &Trajectory,
    shape_geom: &ShapeGeometry,
    unit_count: i16,
    car_length_m: f32,
    passengers: Option<i32>,
    config: SourceRenderConfig,
) -> Vec<RenderUnit> {
    let mut units = Vec::new();

    for unit_index in 0..unit_count {
        let offset_m = unit_index as f64 * car_length_m as f64 + (car_length_m as f64 / 2.0);
        let mut positions = Vec::with_capacity(trajectory.path.len());
        let mut timestamps = Vec::with_capacity(trajectory.timestamps.len());
        let mut bearings = Vec::with_capacity(trajectory.bearings.len());

        for (sample_index, &head_distance) in trajectory.distances_m.iter().enumerate() {
            let center_distance = head_distance - offset_m;
            if !(0.0..=shape_geom.length_m).contains(&center_distance) {
                continue;
            }

            let Some(coord) =
                distance_to_coord(center_distance, &shape_geom.wgs84_line, &shape_geom.cum_dist)
            else {
                continue;
            };

            positions.push([coord.x, coord.y]);
            timestamps.push(trajectory.timestamps[sample_index]);
            bearings.push(
                bearing_at_distance(center_distance, &shape_geom.wgs84_line, &shape_geom.cum_dist)
                    .unwrap_or(trajectory.bearings[sample_index]),
            );
        }

        if positions.len() < 2 {
            continue;
        }

        units.push(RenderUnit {
            render_unit_id: format!("{}:{}:{}", source.as_str(), trajectory.trip_id, unit_index),
            source,
            trip_id: trajectory.trip_id.clone(),
            route_id: trajectory.route_id.clone(),
            icon_key: if unit_index == 0 {
                config.primary_icon_key.to_string()
            } else {
                config.trailing_icon_key.to_string()
            },
            unit_index: Some(unit_index),
            unit_count: Some(unit_count),
            is_head: unit_index == 0,
            length_m: car_length_m,
            passengers,
            color: trajectory.color,
            path_bbox: compute_path_bbox(&positions),
            positions,
            timestamps,
            bearings,
        });
    }

    units
}

fn build_single_unit(
    source: Source,
    trip: &TripSnapshot,
    trajectory: &Trajectory,
    passengers: Option<i32>,
    config: SourceRenderConfig,
) -> Option<RenderUnit> {
    if trajectory.path.len() < 2 || trajectory.timestamps.len() < 2 || trajectory.bearings.len() < 2 {
        return None;
    }

    Some(RenderUnit {
        render_unit_id: format!("{}:{}:0", source.as_str(), trajectory.trip_id),
        source,
        trip_id: trajectory.trip_id.clone(),
        route_id: trajectory.route_id.clone(),
        icon_key: config.primary_icon_key.to_string(),
        unit_index: None,
        unit_count: None,
        is_head: true,
        length_m: trip
            .consist_car_length_m
            .or_else(|| trip.consist_length_m.map(|len| len as f32))
            .unwrap_or(config.default_length_m),
        passengers,
        color: trajectory.color,
        positions: trajectory.path.clone(),
        timestamps: trajectory.timestamps.clone(),
        bearings: trajectory.bearings.clone(),
        path_bbox: trajectory.path_bbox,
    })
}

fn trip_passengers(trip: &TripSnapshot) -> Option<i32> {
    trip.positions.iter().find_map(|position| match &position.data {
        PositionData::MtaBus(data) => data.passengers,
        _ => None,
    })
}
