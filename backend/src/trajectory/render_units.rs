use super::types::{RenderUnit, Trajectory, TripSnapshot};
use crate::operations::types::position::PositionData;
use crate::source::Source;
use std::sync::Arc;

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
        _ => SourceRenderConfig {
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
) -> Vec<RenderUnit> {
    let config = source_render_config(source);
    let expanded = config.expand_consists
        && trip.consist_car_count.is_some_and(|n| n > 0)
        && trip.consist_car_length_m.is_some();
    let count = if expanded {
        trip.consist_car_count.unwrap()
    } else {
        1
    };
    let length = trip
        .consist_car_length_m
        .or_else(|| trip.consist_length_m.map(|l| l as f32))
        .unwrap_or(config.default_length_m);
    let passengers = trip
        .positions
        .iter()
        .filter(|p| matches!(p.data, PositionData::MtaBus(_)))
        .max_by_key(|p| p.updated_at)
        .and_then(|p| match &p.data {
            PositionData::MtaBus(data) => data.passengers,
            _ => None,
        });
    let trajectory = Arc::new(trajectory.clone());
    (0..count)
        .map(|index| RenderUnit {
            render_unit_id: format!("{}:{}:{}", source.as_str(), trip.trip_id, index),
            source,
            trip_id: trip.trip_id.to_string(),
            route_id: trip.route_id.clone(),
            icon_key: if index == 0 {
                config.primary_icon_key
            } else {
                config.trailing_icon_key
            }
            .into(),
            unit_index: expanded.then_some(index),
            unit_count: expanded.then_some(count),
            is_head: index == 0,
            length_m: length,
            passengers,
            color: trajectory.color,
            offset_m: if expanded {
                (index as f64 + 0.5) * length as f64
            } else {
                0.0
            },
            trajectory: trajectory.clone(),
        })
        .collect()
}
