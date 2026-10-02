use super::{
    TrajectoryCache,
    geometry::{geom_to_linestring, shape_key_from_line},
    types::{TrajectoryStop, TripSnapshot},
};
use crate::{
    models::{
        position::VehiclePosition,
        source::Source,
        trip::{StopTime, Trip, TripData},
    },
    static_index::StaticTransitRevision,
};
use anyhow::{Context, ensure};
use chrono::{DateTime, Utc};
use geo::{Coord, LineString};

fn connected(a: Coord<f64>, b: Coord<f64>) -> bool {
    a == b
}

/// Helium static segment IDs encode a directed station leg (for example 76-75
/// or 254-255). Geometry endpoints at the
/// same station need not coincide, so identity, not a distance cutoff, determines
/// whether consecutive provider segments belong to one ordered path.
fn subway_leg_stations(id: &str) -> Option<(&str, &str)> {
    let (from, to) = id.split_once('-')?;
    (!from.is_empty()
        && !to.is_empty()
        && from.bytes().all(|c| c.is_ascii_digit())
        && to.bytes().all(|c| c.is_ascii_digit()))
    .then_some((from, to))
}

fn append_segment(coords: &mut Vec<Coord<f64>>, segment: Vec<Coord<f64>>) {
    // ST_MakeLine collapses an exactly repeated first LineString node. Distinct
    // junction endpoints must both survive, including legitimate station gaps.
    let shared = coords.last() == segment.first();
    coords.extend(segment.into_iter().skip(usize::from(shared)));
}

/// Feed subway shape IDs are ordered station-to-station segments. Topological
/// chains preserve the provider's geometry order, bridging station junction gaps.
/// Shapes without station identity can only join at exact shared endpoints and
/// are oriented there. Bus IDs refer to one resolved whole-route shape.
fn trip_shape(trip: &Trip, revision: &StaticTransitRevision) -> anyhow::Result<LineString<f64>> {
    ensure!(!trip.shape_ids.is_empty(), "Trip has no resolved shape");
    if revision.source != Source::MtaSubway {
        ensure!(
            trip.shape_ids.len() == 1,
            "Bus trip must have one resolved whole-route shape"
        );
    }
    let mut segments = trip
        .shape_ids
        .iter()
        .map(|id| {
            let line =
                geom_to_linestring(revision.shapes.get(id).context("Missing resolved shape")?)
                    .context("Shape is not a LineString")?;
            ensure!(
                line.0.len() >= 2 && line.0.iter().all(|c| c.x.is_finite() && c.y.is_finite()),
                "Invalid shape coordinates"
            );
            Ok(line.0)
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    if revision.source == Source::MtaSubway
        && let Some(stations) = trip
            .shape_ids
            .iter()
            .map(|id| subway_leg_stations(id))
            .collect::<Option<Vec<_>>>()
    {
        ensure!(
            stations.windows(2).all(|legs| legs[0].1 == legs[1].0),
            "Disconnected shape topology"
        );
        let mut coords = Vec::new();
        for segment in segments {
            append_segment(&mut coords, segment);
        }
        return Ok(LineString::new(coords));
    }
    if segments.len() > 1 {
        let first = &segments[0];
        let next = &segments[1];
        if !connected(*first.last().unwrap(), next[0])
            && !connected(*first.last().unwrap(), *next.last().unwrap())
        {
            ensure!(
                connected(first[0], next[0]) || connected(first[0], *next.last().unwrap()),
                "Disconnected shape segments"
            );
            segments[0].reverse();
        }
    }
    let mut coords = segments.remove(0);
    for mut segment in segments {
        let end = *coords.last().unwrap();
        if !connected(end, segment[0]) {
            ensure!(
                connected(end, *segment.last().unwrap()),
                "Disconnected shape segments"
            );
            segment.reverse();
        }
        append_segment(&mut coords, segment);
    }
    Ok(LineString::new(coords))
}

/// Build inputs entirely from a committed source snapshot and its pinned static
/// revision. Stop projection uses the same meter-based geometry as interpolation.
pub async fn snapshot_from_persisted_trip(
    trip: &Trip,
    mut stop_times: Vec<StopTime>,
    positions: Vec<VehiclePosition>,
    revision: &StaticTransitRevision,
    cache: &TrajectoryCache,
    as_of: DateTime<Utc>,
) -> anyhow::Result<TripSnapshot> {
    let shape = trip_shape(trip, revision)?;
    let shape_key = shape_key_from_line(&shape);
    let geometry = cache
        .get_shape_geometry(revision.source, &shape_key, &shape)
        .await
        .context("Shape projection failed")?;
    ensure!(
        geometry.length_m.is_finite() && geometry.length_m > 0.0,
        "Invalid projected shape length"
    );
    let route = revision
        .routes
        .get(&trip.route_id)
        .context("Missing static route")?;
    // TODO: shouldn't stop times already be sorted?
    stop_times.sort_by(|a, b| {
        a.arrival
            .cmp(&b.arrival)
            .then_with(|| a.departure.cmp(&b.departure))
            .then_with(|| a.stop_id.cmp(&b.stop_id))
    });
    let mut stops = Vec::with_capacity(stop_times.len());
    for stop_time in stop_times {
        ensure!(stop_time.trip_id == trip.id, "Stop belongs to another trip");
        let stop = revision
            .stops
            .get(&stop_time.stop_id)
            .context("Missing static stop")?;
        let geo::Geometry::Point(point) = stop.geom.0 else {
            anyhow::bail!("Stop geometry is not a point");
        };
        ensure!(
            point.x().is_finite() && point.y().is_finite(),
            "Non-finite stop point"
        );
        let distance = cache
            .get_stop_projection(
                revision.source,
                &shape_key,
                &stop_time.stop_id,
                point,
                &geometry,
            )
            .await
            .context("Stop projection failed")?;
        ensure!(distance.is_finite(), "Non-finite stop distance");
        stops.push(TrajectoryStop {
            stop_id: stop_time.stop_id,
            arrival_unix: stop_time.arrival.timestamp() as f64,
            departure_unix: stop_time.departure.timestamp() as f64,
            stop_distance_m: distance,
            stop_time_data: stop_time.data,
            stop_data: stop.data.clone(),
        });
    }
    for position in &positions {
        if let Some(geom) = &position.geom {
            let geo::Geometry::Point(point) = geom.0 else {
                anyhow::bail!("Position geometry is not a point");
            };
            ensure!(
                point.x().is_finite() && point.y().is_finite(),
                "Non-finite position point"
            );
        }
    }
    let consist = match &trip.data {
        TripData::MtaSubway(data) => data.consist.as_ref(),
        _ => None,
    };
    Ok(TripSnapshot {
        trip_id: trip.id,
        route_id: trip.route_id.clone(),
        route_color: route.color.clone(),
        direction: trip.direction,
        shape,
        shape_key,
        shape_length_m: geometry.length_m,
        consist_length_m: consist.map(|c| c.car_count as f64 * c.car_length_feet as f64 * 0.3048),
        consist_car_count: consist.map(|c| c.car_count as i16),
        consist_car_length_m: consist.map(|c| c.car_length_feet as f32 * 0.3048),
        stops,
        positions,
        as_of,
    })
}

pub fn source_supports_trajectories(source: Source) -> bool {
    matches!(source, Source::MtaSubway | Source::MtaBus | Source::NjtBus)
}
