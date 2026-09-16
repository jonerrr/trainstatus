// my attempt at centralizing the static data parsing into a generalized GTFS parser
// but ended up being useless since each source needs a lot of custom mapping logic. For example, MTA bus GTFS doesn't include the shuttles, NJT bus shapes are terrible, etc.
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    io::Cursor,
    sync::Arc,
};

use anyhow::Context;
use geo::{Distance, Euclidean, LineString, MultiLineString, Point};
use gtfs_structures::{Gtfs, Shape, Stop, Trip};
use proj4rs::{Proj, transform::transform};

use crate::{
    models::{
        route::{Route, RouteData},
        source::Source,
        stop::{RouteStop, RouteStopData, Stop as StopModel, StopData},
    },
    sources::{normalize_title, normalize_whitespace},
};

const DEFAULT_MAX_OPPOSITE_DIST_METERS: f64 = 500.0;

#[derive(Debug, Clone)]
pub struct GtfsArchive {
    pub name: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct ParserOptions {
    pub compute_opposite_stop_id: bool,
    pub max_opposite_stop_distance_meters: f64,
    pub default_route_color: String,
}

impl Default for ParserOptions {
    fn default() -> Self {
        Self {
            compute_opposite_stop_id: true,
            max_opposite_stop_distance_meters: DEFAULT_MAX_OPPOSITE_DIST_METERS,
            default_route_color: "FFFFFF".to_owned(),
        }
    }
}

pub struct ParsedStaticData {
    pub routes: Vec<Route>,
    pub stops: Vec<StopModel>,
    pub route_stops: Vec<RouteStop>,
}

pub struct RouteStopContext<'a> {
    pub route_id: &'a str,
    pub stop_id: &'a str,
    pub direction: i16,
    pub headsign: &'a str,
    pub opposite_stop_id: Option<&'a str>,
}

pub trait StaticParserMapper {
    fn map_route_data(&self, route: &gtfs_structures::Route) -> anyhow::Result<RouteData>;

    fn map_stop_data(&self, stop: &gtfs_structures::Stop) -> anyhow::Result<StopData>;

    fn map_route_stop_data(&self, ctx: &RouteStopContext<'_>) -> anyhow::Result<RouteStopData>;
}

#[derive(Clone)]
struct MergedTrip {
    trip: Trip,
}

#[derive(Default)]
struct MergedGtfs {
    routes: HashMap<String, gtfs_structures::Route>,
    stops: HashMap<String, Arc<Stop>>,
    trips: Vec<MergedTrip>,
    // shape_id -> shape points
    shapes: HashMap<String, Vec<Shape>>,
}

#[derive(Default)]
struct RouteStopAccumulator {
    min_sequence: i16,
    headsign_counts: HashMap<String, usize>,
}

#[derive(Clone)]
struct DirectionalRouteStop {
    route_id: String,
    stop_id: String,
    direction: i16,
    stop_sequence: i16,
    headsign: String,
    opposite_stop_id: Option<String>,
}

pub async fn parse_archive<M: StaticParserMapper>(
    _source: Source,
    archive: GtfsArchive,
    mapper: &M,
    options: &ParserOptions,
) -> anyhow::Result<ParsedStaticData> {
    let feed_name = archive.name.clone();
    let bytes = archive.bytes;
    let gtfs = tokio::task::spawn_blocking(move || Gtfs::from_reader(Cursor::new(bytes)))
        .await
        .context("GTFS parse task panicked")?
        .with_context(|| format!("Failed to parse GTFS archive {feed_name}"))?;

    let merged = merge_feed(&gtfs);

    let routes = build_routes(&merged, mapper, options)?;
    let stops = build_stops(&merged, mapper)?;
    let route_stops = build_route_stops(&merged, mapper, options)?;

    Ok(ParsedStaticData {
        routes,
        stops,
        route_stops,
    })
}

fn merge_feed(feed: &Gtfs) -> MergedGtfs {
    let mut merged = MergedGtfs::default();

    for route in feed.routes.values() {
        merged.routes.insert(route.id.to_uppercase(), route.clone());
    }

    for (stop_id, stop) in &feed.stops {
        merged.stops.insert(stop_id.to_uppercase(), stop.clone());
    }

    for trip in feed.trips.values() {
        merged.trips.push(MergedTrip { trip: trip.clone() });
    }

    merged.shapes = feed.shapes.clone();

    merged
}

fn build_routes<M: StaticParserMapper>(
    merged: &MergedGtfs,
    mapper: &M,
    options: &ParserOptions,
) -> anyhow::Result<Vec<Route>> {
    let mut routes = merged
        .routes
        .values()
        .map(|route| {
            let color = route
                .color
                .map(rgb_to_hex)
                .filter(|hex| hex != "000000")
                .unwrap_or_else(|| options.default_route_color.clone());

            Ok(Route {
                id: route.id.clone(),
                long_name: route
                    .long_name
                    .as_deref()
                    .map(normalize_whitespace)
                    .unwrap_or_default(),
                short_name: route
                    .short_name
                    .as_deref()
                    .map(normalize_whitespace)
                    .unwrap_or_default(),
                color,
                data: mapper.map_route_data(route)?,
            })
        })
        .collect::<anyhow::Result<Vec<_>>>()?;

    routes.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(routes)
}

fn build_stops<M: StaticParserMapper>(
    merged: &MergedGtfs,
    mapper: &M,
) -> anyhow::Result<Vec<StopModel>> {
    let mut values = merged
        .stops
        .values()
        .filter_map(|stop| {
            let lat = stop.latitude?;
            let lon = stop.longitude?;
            Some((stop, lat, lon))
        })
        .map(|(stop, lat, lon)| {
            let name = stop.name.as_deref().unwrap_or(&stop.id);
            Ok(StopModel {
                id: stop.id.clone(),
                name: normalize_title(name),
                geom: Point::new(lon, lat).into(),
                transfers: vec![],
                routes: vec![],
                data: mapper.map_stop_data(stop)?,
            })
        })
        .collect::<anyhow::Result<Vec<_>>>()?;

    values.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(values)
}

fn build_route_stops<M: StaticParserMapper>(
    merged: &MergedGtfs,
    mapper: &M,
    options: &ParserOptions,
) -> anyhow::Result<Vec<RouteStop>> {
    let stop_geom_map = projected_stop_points(&merged.stops);

    let mut accum: HashMap<(String, String, i16), RouteStopAccumulator> = HashMap::new();

    for merged_trip in &merged.trips {
        let direction = merged_trip.trip.direction_id.map(|d| d as i16).unwrap_or(0);

        let trip_headsign = merged_trip.trip.trip_headsign.as_deref().unwrap_or("");

        for stop_time in &merged_trip.trip.stop_times {
            let route_id = merged_trip.trip.route_id.to_uppercase();
            let stop_id = stop_time.stop.id.to_uppercase();
            let sequence = stop_time.stop_sequence as i16;

            let raw_headsign = stop_time
                .stop_headsign
                .as_deref()
                .filter(|s| !s.trim().is_empty())
                .unwrap_or(trip_headsign);
            let normalized_headsign = normalize_whitespace(raw_headsign);

            let key = (route_id, stop_id, direction);
            let entry = accum.entry(key).or_insert_with(|| RouteStopAccumulator {
                min_sequence: sequence,
                headsign_counts: HashMap::new(),
            });

            if sequence < entry.min_sequence {
                entry.min_sequence = sequence;
            }

            if !normalized_headsign.is_empty() {
                *entry
                    .headsign_counts
                    .entry(normalized_headsign)
                    .or_insert(0) += 1;
            }
        }
    }

    let mut by_route_direction: BTreeMap<(String, i16), Vec<(String, i16)>> = BTreeMap::new();
    for ((route_id, stop_id, direction), acc) in &accum {
        by_route_direction
            .entry((route_id.clone(), *direction))
            .or_default()
            .push((stop_id.clone(), acc.min_sequence));
    }

    let opposite_map = if options.compute_opposite_stop_id {
        compute_route_opposites(
            &by_route_direction,
            &stop_geom_map,
            options.max_opposite_stop_distance_meters,
        )
    } else {
        HashMap::new()
    };

    let mut directional_rows = accum
        .into_iter()
        .map(|((route_id, stop_id, direction), acc)| {
            let opposite_stop_id = opposite_map
                .get(&(route_id.clone(), stop_id.clone(), direction))
                .cloned();
            DirectionalRouteStop {
                route_id,
                stop_id,
                direction,
                stop_sequence: acc.min_sequence,
                headsign: choose_headsign(acc.headsign_counts),
                opposite_stop_id,
            }
        })
        .collect::<Vec<_>>();

    directional_rows.sort_by(|a, b| {
        (
            &a.route_id,
            &a.stop_id,
            a.direction,
            a.stop_sequence,
            &a.headsign,
        )
            .cmp(&(
                &b.route_id,
                &b.stop_id,
                b.direction,
                b.stop_sequence,
                &b.headsign,
            ))
    });

    let mut deduped: HashMap<(String, String), DirectionalRouteStop> = HashMap::new();
    for row in directional_rows {
        let key = (row.route_id.clone(), row.stop_id.clone());
        deduped
            .entry(key)
            .and_modify(|existing| {
                if row.stop_sequence < existing.stop_sequence
                    || (row.stop_sequence == existing.stop_sequence
                        && row.direction < existing.direction)
                    || (row.stop_sequence == existing.stop_sequence
                        && row.direction == existing.direction
                        && row.headsign < existing.headsign)
                {
                    *existing = row.clone();
                }
            })
            .or_insert(row);
    }

    let mut route_stops = deduped
        .into_values()
        .map(|row| {
            let data = {
                let ctx = RouteStopContext {
                    route_id: &row.route_id,
                    stop_id: &row.stop_id,
                    direction: row.direction,
                    headsign: &row.headsign,
                    opposite_stop_id: row.opposite_stop_id.as_deref(),
                };
                mapper.map_route_stop_data(&ctx)?
            };

            Ok(RouteStop {
                route_id: row.route_id,
                stop_id: row.stop_id,
                stop_sequence: row.stop_sequence,
                data,
            })
        })
        .collect::<anyhow::Result<Vec<_>>>()?;

    route_stops.sort_by(|a, b| (&a.route_id, &a.stop_id).cmp(&(&b.route_id, &b.stop_id)));

    Ok(route_stops)
}

fn build_route_geometries(merged: &MergedGtfs) -> HashMap<String, MultiLineString> {
    let mut per_route_direction: HashMap<(String, i16), Vec<LineString>> = HashMap::new();
    let mut seen_shapes: HashMap<(String, i16), HashSet<String>> = HashMap::new();

    for merged_trip in &merged.trips {
        let Some(shape_id) = merged_trip.trip.shape_id.as_ref() else {
            continue;
        };

        let Some(shape_points) = merged.shapes.get(shape_id) else {
            continue;
        };

        let line = shape_points_to_linestring(shape_points);
        if line.0.len() < 2 {
            continue;
        }

        let route_id = merged_trip.trip.route_id.to_uppercase();
        let direction = merged_trip.trip.direction_id.map(|d| d as i16).unwrap_or(0);

        let fingerprint = linestring_fingerprint(&line);
        let key = (route_id.clone(), direction);
        let seen = seen_shapes.entry(key.clone()).or_default();
        if seen.insert(fingerprint) {
            per_route_direction.entry(key).or_default().push(line);
        }
    }

    let mut by_route: HashMap<String, Vec<LineString>> = HashMap::new();
    for ((route_id, _direction), mut lines) in per_route_direction {
        by_route.entry(route_id).or_default().append(&mut lines);
    }

    by_route
        .into_iter()
        .map(|(route_id, lines)| (route_id, MultiLineString::new(lines)))
        .collect()
}

fn shape_points_to_linestring(points: &[Shape]) -> LineString {
    let mut sorted = points.to_vec();
    sorted.sort_by_key(|p| p.sequence);

    let mut coords = Vec::with_capacity(sorted.len());
    for point in sorted {
        if coords
            .last()
            .is_some_and(|prev: &geo::Coord| prev.x == point.longitude && prev.y == point.latitude)
        {
            continue;
        }
        coords.push(geo::Coord {
            x: point.longitude,
            y: point.latitude,
        });
    }

    LineString::new(coords)
}

fn linestring_fingerprint(line: &LineString) -> String {
    line.0
        .iter()
        .map(|c| format!("{:.6}:{:.6}", c.x, c.y))
        .collect::<Vec<_>>()
        .join("|")
}

fn projected_stop_points(stops: &HashMap<String, Arc<Stop>>) -> HashMap<String, Point<f64>> {
    let proj_wgs84 = Proj::from_epsg_code(4326).expect("Failed to create WGS84 proj");
    let proj_ny = Proj::from_epsg_code(6538).expect("Failed to create NY proj");

    stops
        .values()
        .filter_map(|stop| {
            let lat = stop.latitude?;
            let lon = stop.longitude?;
            let mut point = Point::new(lon.to_radians(), lat.to_radians());
            transform(&proj_wgs84, &proj_ny, &mut point).ok()?;
            Some((stop.id.to_uppercase(), point))
        })
        .collect()
}

fn compute_route_opposites(
    by_route_direction: &BTreeMap<(String, i16), Vec<(String, i16)>>,
    stop_geom_map: &HashMap<String, Point<f64>>,
    max_dist: f64,
) -> HashMap<(String, String, i16), String> {
    let mut result = HashMap::new();

    let route_ids: HashSet<String> = by_route_direction
        .keys()
        .map(|(route_id, _)| route_id.clone())
        .collect();

    for route_id in route_ids {
        let mut dir0 = by_route_direction
            .get(&(route_id.clone(), 0))
            .cloned()
            .unwrap_or_default();
        let mut dir1 = by_route_direction
            .get(&(route_id.clone(), 1))
            .cloned()
            .unwrap_or_default();

        dir0.sort_by_key(|(_, seq)| *seq);
        dir1.sort_by_key(|(_, seq)| *seq);

        let dir0_ids = dir0.into_iter().map(|(id, _)| id).collect::<Vec<_>>();
        let dir1_ids = dir1.into_iter().map(|(id, _)| id).collect::<Vec<_>>();

        let opposites = compute_opposite_stops(&dir0_ids, &dir1_ids, stop_geom_map, max_dist);

        for (stop_id, opposite_stop_id) in opposites {
            let direction = if dir0_ids.contains(&stop_id) { 0 } else { 1 };
            result.insert((route_id.clone(), stop_id, direction), opposite_stop_id);
        }
    }

    result
}

fn compute_opposite_stops(
    dir0_ids: &[String],
    dir1_ids: &[String],
    stop_geom_map: &HashMap<String, Point<f64>>,
    max_dist: f64,
) -> HashMap<String, String> {
    let mut opposite_map: HashMap<String, String> = HashMap::new();

    for stop_id in dir0_ids {
        let Some(p0) = stop_geom_map.get(stop_id) else {
            continue;
        };
        let best = dir1_ids
            .iter()
            .filter_map(|opp_id| {
                let p1 = stop_geom_map.get(opp_id)?;
                let dist = Euclidean.distance(p0, p1);
                (dist <= max_dist).then_some((opp_id, dist))
            })
            .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
        if let Some((opp_id, _)) = best {
            opposite_map.insert(stop_id.clone(), opp_id.clone());
        }
    }

    for stop_id in dir1_ids {
        let Some(p1) = stop_geom_map.get(stop_id) else {
            continue;
        };
        let best = dir0_ids
            .iter()
            .filter_map(|opp_id| {
                let p0 = stop_geom_map.get(opp_id)?;
                let dist = Euclidean.distance(p1, p0);
                (dist <= max_dist).then_some((opp_id, dist))
            })
            .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
        if let Some((opp_id, _)) = best {
            opposite_map.insert(stop_id.clone(), opp_id.clone());
        }
    }

    opposite_map
}

fn choose_headsign(counts: HashMap<String, usize>) -> String {
    if counts.is_empty() {
        return "Unknown".to_owned();
    }

    let mut pairs = counts.into_iter().collect::<Vec<_>>();
    pairs.sort_by(|a, b| {
        // highest count first; lexicographic asc for deterministic tie-break
        b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0))
    });

    pairs[0].0.clone()
}

fn rgb_to_hex(color: rgb::RGB<u8>) -> String {
    format!("{:02X}{:02X}{:02X}", color.r, color.g, color.b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chooses_headsign_with_lex_tie_breaker() {
        let mut counts = HashMap::new();
        counts.insert("Downtown".to_string(), 3);
        counts.insert("Uptown".to_string(), 3);

        let winner = choose_headsign(counts);
        assert_eq!(winner, "Downtown");
    }

    #[test]
    fn opposite_stop_matching_is_bidirectional() {
        let mut stop_geom_map = HashMap::new();
        stop_geom_map.insert("A0".to_string(), Point::new(0.0, 0.0));
        stop_geom_map.insert("A1".to_string(), Point::new(100.0, 0.0));
        stop_geom_map.insert("B0".to_string(), Point::new(2.0, 0.0));
        stop_geom_map.insert("B1".to_string(), Point::new(98.0, 0.0));

        let dir0 = vec!["A0".to_string(), "A1".to_string()];
        let dir1 = vec!["B0".to_string(), "B1".to_string()];

        let result = compute_opposite_stops(&dir0, &dir1, &stop_geom_map, 10.0);

        assert_eq!(result.get("A0"), Some(&"B0".to_string()));
        assert_eq!(result.get("A1"), Some(&"B1".to_string()));
        assert_eq!(result.get("B0"), Some(&"A0".to_string()));
        assert_eq!(result.get("B1"), Some(&"A1".to_string()));
    }

    #[test]
    fn shape_to_linestring_sorts_and_dedupes_points() {
        let shape = vec![
            Shape {
                id: "s1".to_string(),
                sequence: 2,
                latitude: 0.0,
                longitude: 1.0,
                ..Default::default()
            },
            Shape {
                id: "s1".to_string(),
                sequence: 1,
                latitude: 0.0,
                longitude: 0.0,
                ..Default::default()
            },
            Shape {
                id: "s1".to_string(),
                sequence: 3,
                latitude: 0.0,
                longitude: 1.0,
                ..Default::default()
            },
        ];

        let line = shape_points_to_linestring(&shape);
        assert_eq!(line.0.len(), 2);
        assert_eq!(line.0[0].x, 0.0);
        assert_eq!(line.0[1].x, 1.0);
    }
}
