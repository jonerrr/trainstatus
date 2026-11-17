use crate::network::projection::project_stop;
use crate::network::types::stop::{NearbyStop, Stop, StopData};
use crate::source::Source;
use geo::{Centroid, Geometry, Point};
use std::collections::HashMap;

const NEARBY_RADIUS_M: f64 = 150.0;

#[derive(Clone)]
struct ProjectedStop {
    source: Source,
    id: String,
    point: Point<f64>,
    boardable: bool,
    index: usize,
}

pub fn attach_nearby_stops(stops_by_source: &mut HashMap<Source, Vec<Stop>>) {
    let mut projected = Vec::new();
    let mut grid: HashMap<(i32, i32), Vec<usize>> = HashMap::new();
    for (source, stops) in stops_by_source.iter() {
        for stop in stops.iter() {
            let point = stop_point(&stop.geom.0);
            let Some(projected_point) = project_stop(&point) else {
                continue;
            };
            let cell = cell_coords(&projected_point);
            let index = projected.len();
            projected.push(ProjectedStop {
                source: *source,
                id: stop.id.clone(),
                point: projected_point,
                boardable: is_boardable(*source, stop),
                index,
            });
            grid.entry(cell).or_default().push(index);
        }
    }
    let nearby = compute_nearby_grid(&projected, &grid);
    for (source, stops) in stops_by_source {
        for stop in stops.iter_mut() {
            let key = (stop.id.clone(), *source);
            stop.nearby_stops = nearby.get(&key).cloned().unwrap_or_default();
        }
    }
}

fn cell_coords(point: &Point<f64>) -> (i32, i32) {
    (
        (point.x() / NEARBY_RADIUS_M).floor() as i32,
        (point.y() / NEARBY_RADIUS_M).floor() as i32,
    )
}

fn compute_nearby_grid(
    stops: &[ProjectedStop],
    grid: &HashMap<(i32, i32), Vec<usize>>,
) -> HashMap<(String, Source), Vec<NearbyStop>> {
    let mut nearby: HashMap<(String, Source), Vec<NearbyStop>> = HashMap::new();
    for stop in stops {
        let (cx, cy) = cell_coords(&stop.point);
        for dx in -1..=1 {
            for dy in -1..=1 {
                let Some(candidates) = grid.get(&(cx + dx, cy + dy)) else {
                    continue;
                };
                for &candidate in candidates {
                    let other = &stops[candidate];
                    if stop.index == other.index {
                        continue;
                    }
                    if stop.index > other.index {
                        continue;
                    }
                    if stop.source == other.source && stop.id.eq_ignore_ascii_case(&other.id) {
                        continue;
                    }
                    let dx = stop.point.x() - other.point.x();
                    let dy = stop.point.y() - other.point.y();
                    let distance_m = (dx * dx + dy * dy).sqrt();
                    if distance_m > NEARBY_RADIUS_M {
                        continue;
                    }
                    if other.boardable {
                        nearby
                            .entry((stop.id.clone(), stop.source))
                            .or_default()
                            .push(NearbyStop {
                                stop_id: other.id.clone(),
                                source: other.source,
                                distance_m,
                            });
                    }
                    if stop.boardable {
                        nearby
                            .entry((other.id.clone(), other.source))
                            .or_default()
                            .push(NearbyStop {
                                stop_id: stop.id.clone(),
                                source: stop.source,
                                distance_m,
                            });
                    }
                }
            }
        }
    }
    for entries in nearby.values_mut() {
        entries.sort_by(|left, right| {
            left.distance_m
                .total_cmp(&right.distance_m)
                .then_with(|| left.source.as_str().cmp(right.source.as_str()))
                .then_with(|| left.stop_id.cmp(&right.stop_id))
        });
    }
    nearby
}

fn is_boardable(source: Source, stop: &Stop) -> bool {
    if matches!(source, Source::MtaBus) {
        let StopData::MtaBus(data) = &stop.data else {
            return false;
        };
        if !data.is_boardable {
            return false;
        }
    }
    !stop.routes.is_empty()
}

fn stop_point(geometry: &Geometry<f64>) -> Point<f64> {
    match geometry {
        Geometry::Point(point) => *point,
        other => Centroid::centroid(other).unwrap_or_else(|| Point::new(0., 0.)),
    }
}
