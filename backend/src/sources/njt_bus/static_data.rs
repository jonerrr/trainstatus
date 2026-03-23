use std::{
    collections::{HashMap, HashSet},
    io::Cursor,
    sync::Arc,
    time::Duration,
};

use crate::{
    engines::static_cache::expand_gtfs,
    engines::valhalla::ValhallaManager,
    models::{
        route::{Route, RouteData},
        source::Source,
        stop::{NjtBusStopData, RouteStop, RouteStopData, Stop, StopData},
    },
    sources::{StaticAdapter, normalize_title, normalize_whitespace},
    stores::{route::RouteStore, static_cache::StaticCacheStore, stop::StopStore},
};
use anyhow::Context;
use async_trait::async_trait;
use geo::{Distance, Euclidean, LineString, MultiLineString, Point};
use geojson::GeoJson;
use proj4rs::{Proj, transform::transform};
use tracing::{info, warn};

use super::{NJT_BUS_ROUTES_URL, NjtApi};

const NJT_DEFAULT_COLOR: &str = "1A2B57";
const MAX_OPPOSITE_DIST: f64 = 500.0;
const NJT_GTFS_URL: &str = "https://pcsdata.njtransit.com/api/GTFSG2/getGTFS";
const NJT_ARCGIS_URL: &str = "https://services6.arcgis.com/M0t0HPE53pFK525U/arcgis/rest/services/Bus_Lines_of_NJ_Transit/FeatureServer/1/query?where=1%3D1&outFields=*&outSR=4326&f=geojson";

#[derive(Debug, Clone, serde::Deserialize)]
struct BusRoute {
    #[serde(rename = "BusRouteID")]
    bus_route_id: String,
    #[serde(rename = "BusRouteDescription")]
    bus_route_description: String,
}

pub struct NjtBusStatic {
    valhalla: Arc<ValhallaManager>,
}

impl NjtBusStatic {
    pub fn new(valhalla: Arc<ValhallaManager>) -> Self {
        Self { valhalla }
    }

    async fn snap_route_geometries(&self, route_geometries: &mut HashMap<String, MultiLineString>) {
        for (route_id, geometry) in route_geometries.iter_mut() {
            let mut snapped_lines = Vec::with_capacity(geometry.0.len());

            for (line_index, line) in geometry.0.iter().enumerate() {
                if line.0.len() < 2 {
                    snapped_lines.push(line.clone());
                    continue;
                }

                match self.valhalla.trace_route(line).await {
                    Ok(snapped_line) => snapped_lines.push(snapped_line),
                    Err(err) => {
                        tracing::warn!(
                            route_id,
                            line_index,
                            error = %err,
                            "NJT bus route snapping failed; preserving original linestring"
                        );
                        snapped_lines.push(line.clone());
                    }
                }
            }

            *geometry = MultiLineString::new(snapped_lines);
        }
    }
}

#[async_trait]
impl StaticAdapter for NjtBusStatic {
    fn source(&self) -> Source {
        Source::NjtBus
    }

    fn refresh_interval(&self) -> Duration {
        Duration::from_secs(60 * 60 * 24) // 24 hours
    }

    async fn import(
        &self,
        route_store: &RouteStore,
        stop_store: &StopStore,
        static_cache_store: &StaticCacheStore,
    ) -> anyhow::Result<()> {
        let token = super::get_token(NjtApi::GtfsG2)
            .await
            .context("NJT authentication failed")?;

        let gtfs_bytes = download_gtfs(&token)
            .await
            .context("NJT GTFS download failed")?;

        // Parse GTFS in a blocking task
        let gtfs = tokio::task::spawn_blocking(move || {
            gtfs_structures::Gtfs::from_reader(Cursor::new(gtfs_bytes))
        })
        .await
        .context("GTFS parse task panicked")?
        .context("Failed to parse NJT GTFS")?;

        tracing::info!(
            "NJT GTFS parsed: {} routes, {} stops, {} trips",
            gtfs.routes.len(),
            gtfs.stops.len(),
            gtfs.trips.len()
        );

        // Expand and cache trips for 48 hours in Redis
        let cached_trips = expand_gtfs(Source::NjtBus, &gtfs);
        static_cache_store
            .cache_trips(Source::NjtBus, &cached_trips)
            .await
            .context("Failed to cache NJT trips in Redis")?;

        let bus_routes_token = super::get_token(NjtApi::BusDv2)
            .await
            .context("NJT BUSDV2 authentication failed")?;

        let bus_routes = fetch_bus_routes(&bus_routes_token)
            .await
            .context("Failed to fetch BUSDV2 route metadata")?;

        // TODO: either fix or remove valhalla route snapping for njt bus
        let mut route_geom_map = fetch_route_geometries(&bus_routes, &gtfs)
            .await
            .context("Failed to fetch NJT route geometries")?;

        // Keep Valhalla warm while this import's snapping pass is active.
        let _import_snap_usage = match self.valhalla.acquire_usage().await {
            Ok(lease) => Some(lease),
            Err(err) => {
                tracing::warn!(
                    error = %err,
                    "Unable to acquire Valhalla import usage lease; will continue with per-request fallback"
                );
                None
            }
        };

        // self.snap_route_geometries(&mut route_geom_map).await;

        // Build all entities
        let routes = build_routes(&gtfs, &route_geom_map);
        let stops = build_stops(&gtfs);
        let route_stops = build_route_stops(&gtfs);

        tracing::info!(
            "NJT: built {} routes, {} stops, {} route_stops",
            routes.len(),
            stops.len(),
            route_stops.len()
        );

        #[cfg(debug_assertions)]
        {
            // additional sanity check: look for duplicates in the raw vector (before storing)
            let mut seen: HashMap<(String, String), usize> = HashMap::new();
            for rs in &route_stops {
                let key = (rs.route_id.clone(), rs.stop_id.clone());
                *seen.entry(key).or_insert(0) += 1;
            }
            for ((rid, sid), count) in seen {
                if count > 1 {
                    tracing::warn!(
                        "raw route_stops vector contains duplicate entries for route_id={} stop_id={} count={}",
                        rid,
                        sid,
                        count
                    );
                }
            }
        }

        route_store
            .save_all(Source::NjtBus, &routes)
            .await
            .context("Failed to save NJT routes to database")?;
        stop_store
            .save_all(Source::NjtBus, &stops)
            .await
            .context("Failed to save NJT stops to database")?;
        stop_store
            .save_all_route_stops(Source::NjtBus, &route_stops)
            .await
            .context("Failed to save NJT route_stops to database")?;

        Ok(())
    }
}

// ── GTFS download ─────────────────────────────────────────────────────────────

async fn download_gtfs(token: &str) -> anyhow::Result<Vec<u8>> {
    let form = reqwest::multipart::Form::new().text("token", token.to_owned());

    let bytes = reqwest::Client::new()
        .post(NJT_GTFS_URL)
        .multipart(form)
        .send()
        .await?
        .error_for_status()?
        .bytes()
        .await?;

    Ok(bytes.to_vec())
}

async fn fetch_bus_routes(token: &str) -> anyhow::Result<Vec<BusRoute>> {
    let form = reqwest::multipart::Form::new()
        .text("token", token.to_owned())
        .text("mode", "ALL");

    let routes = reqwest::Client::new()
        .post(NJT_BUS_ROUTES_URL)
        .multipart(form)
        .send()
        .await?
        .error_for_status()?
        .json::<Vec<BusRoute>>()
        .await?;

    Ok(routes)
}

// ── Route geometry (ArcGIS) ───────────────────────────────────────────────────

/// Fetch GeoJSON from NJT ArcGIS and return a map of route_id → MultiLineString.
async fn fetch_route_geometries(
    bus_routes: &[BusRoute],
    gtfs: &gtfs_structures::Gtfs,
) -> anyhow::Result<HashMap<String, MultiLineString>> {
    let text = reqwest::Client::new()
        .get(NJT_ARCGIS_URL)
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;

    let geojson: GeoJson = text.parse().context("Failed to parse ArcGIS GeoJSON")?;

    let fc = match geojson {
        GeoJson::FeatureCollection(fc) => fc,
        _ => anyhow::bail!("Expected a GeoJSON FeatureCollection from ArcGIS"),
    };

    // Group line segments by route_id (LINE_STRING property)
    let mut route_lines: HashMap<String, Vec<LineString>> = HashMap::new();

    let bus_route_by_id: HashMap<String, String> = bus_routes
        .iter()
        .map(|route| {
            (
                route.bus_route_id.clone(),
                route.bus_route_description.clone(),
            )
        })
        .collect();

    let gtfs_route_ids: HashSet<String> =
        gtfs.routes.values().map(|route| route.id.clone()).collect();

    let mut gtfs_route_ids_by_long_name: HashMap<String, Vec<String>> = HashMap::new();
    for route in gtfs.routes.values() {
        let Some(long_name) = route.long_name.as_deref() else {
            continue;
        };

        gtfs_route_ids_by_long_name
            .entry(route_name_key(long_name))
            .or_default()
            .push(route.id.clone());
    }

    let mut mapped_via_bus_api = 0usize;
    let mut unresolved_line_string_ids = 0usize;

    for feature in fc.features {
        let Some(props) = &feature.properties else {
            continue;
        };
        let Some(val) = props.get("LINE") else {
            continue;
        };

        let Some(line_string_id) = val.as_u64().map(|v| v.to_string()) else {
            warn!(value = ?val, "Unexpected non-string LINE property in ArcGIS feature; skipping");
            continue;
        };

        // this seems to get most of the routes (newark light rail, etc). but theres still a couple gtfs routes that don't have matching geometry
        let route_id = match map_arcgis_route_id_to_gtfs_route_id(
            &line_string_id,
            &bus_route_by_id,
            &gtfs_route_ids_by_long_name,
            &gtfs_route_ids,
        ) {
            Some(gtfs_route_id) => {
                if gtfs_route_id != line_string_id {
                    mapped_via_bus_api += 1;
                }
                gtfs_route_id
            }
            None => {
                unresolved_line_string_ids += 1;
                line_string_id.into()
            }
        };

        let Some(ref geom) = feature.geometry else {
            continue;
        };

        match &geom.value {
            geojson::GeometryValue::LineString { coordinates } => {
                route_lines
                    .entry(route_id)
                    .or_default()
                    .push(positions_to_linestring(coordinates));
            }
            geojson::GeometryValue::MultiLineString { coordinates: multi } => {
                let lines = route_lines.entry(route_id).or_default();
                for positions in multi {
                    lines.push(positions_to_linestring(positions));
                }
            }
            _ => {}
        }
    }

    if mapped_via_bus_api > 0 {
        info!(
            mapped_via_bus_api,
            "Mapped ArcGIS LINE_STRING IDs to GTFS route IDs using BUSDV2 route descriptions"
        );
    }

    if unresolved_line_string_ids > 0 {
        warn!(
            unresolved_line_string_ids,
            "ArcGIS features could not be mapped via BUSDV2 metadata; keeping original LINE_STRING value"
        );
    }

    Ok(route_lines
        .into_iter()
        .map(|(id, lines)| (id, MultiLineString::new(lines)))
        .collect())
}

fn map_arcgis_route_id_to_gtfs_route_id(
    line_string_id: &str,
    bus_route_by_id: &HashMap<String, String>,
    gtfs_route_ids_by_long_name: &HashMap<String, Vec<String>>,
    gtfs_route_ids: &HashSet<String>,
) -> Option<String> {
    if gtfs_route_ids.contains(line_string_id) {
        return Some(line_string_id.into());
    }

    let description = bus_route_by_id.get(line_string_id)?;
    let candidates = gtfs_route_ids_by_long_name.get(&route_name_key(description))?;

    if candidates.len() == 1 {
        return candidates.first().cloned();
    }

    // TODO: check if this actually happens
    candidates
        .iter()
        .find(|route_id| route_id.eq_ignore_ascii_case(line_string_id))
        .cloned()
        .or_else(|| candidates.first().cloned())
}

fn route_name_key(value: &str) -> String {
    normalize_whitespace(value).to_ascii_lowercase()
}

fn positions_to_linestring(positions: &[geojson::Position]) -> LineString {
    LineString::new(
        positions
            .iter()
            .map(|p| geo::Coord { x: p[0], y: p[1] })
            .collect(),
    )
}

// ── Build routes ──────────────────────────────────────────────────────────────

fn build_routes(
    gtfs: &gtfs_structures::Gtfs,
    geom_map: &HashMap<String, MultiLineString>,
) -> Vec<Route> {
    gtfs.routes
        .values()
        .map(|r| {
            let color = r
                .color
                .map(|c| format!("{:02X}{:02X}{:02X}", c.r, c.g, c.b))
                .filter(|hex| hex != "000000")
                .unwrap_or_else(|| NJT_DEFAULT_COLOR.to_owned());

            Route {
                id: r.id.clone(),
                long_name: r
                    .long_name
                    .as_deref()
                    .map(normalize_whitespace)
                    .unwrap_or_default(),
                short_name: r
                    .short_name
                    .as_deref()
                    .map(normalize_whitespace)
                    .unwrap_or_default(),
                color,
                data: RouteData::NjtBus,
                geom: geom_map.get(&r.id).cloned().map(Into::into),
            }
        })
        .collect()
}

// ── Build stops ───────────────────────────────────────────────────────────────

fn build_stops(gtfs: &gtfs_structures::Gtfs) -> Vec<Stop> {
    gtfs.stops
        .values()
        .filter_map(|s| {
            let lat = s.latitude?;
            let lon = s.longitude?;
            let stop_code = s.code.clone().unwrap_or_else(|| s.id.clone());
            let raw_name = s.name.as_deref().unwrap_or(&s.id);
            let name = normalize_title(raw_name);

            Some(Stop {
                id: s.id.clone(),
                name,
                geom: Point::new(lon, lat).into(),
                transfers: vec![],
                routes: vec![],
                data: StopData::NjtBus(NjtBusStopData { stop_code }),
            })
        })
        .collect()
}

// ── Build route_stops ─────────────────────────────────────────────────────────

/// Deduplication key: (route_id, stop_id, direction_id)
type RouteStopKey = (String, String, i16);

struct Accumulator {
    min_sequence: i16,
    /// headsign text → occurrence count
    headsign_counts: HashMap<String, usize>,
}

fn build_route_stops(gtfs: &gtfs_structures::Gtfs) -> Vec<RouteStop> {
    let proj_wgs84 = Proj::from_epsg_code(4326).expect("Failed to create WGS84 proj");
    let proj_ny = Proj::from_epsg_code(6538).expect("Failed to create NY proj");

    // Build a map of stop_id -> projected Point
    let stop_geom_map: HashMap<String, Point<f64>> = gtfs
        .stops
        .values()
        .filter_map(|s| {
            let lat = s.latitude?;
            let lon = s.longitude?;
            let mut point = Point::new(lon.to_radians(), lat.to_radians());
            transform(&proj_wgs84, &proj_ny, &mut point).ok()?;
            Some((s.id.clone(), point))
        })
        .collect();

    // Map: route_id -> (Dir0 stops, Dir1 stops)
    let mut route_dir_stops: HashMap<String, (Vec<String>, Vec<String>)> = HashMap::new();
    let mut accum: HashMap<RouteStopKey, Accumulator> = HashMap::new();

    for trip in gtfs.trips.values() {
        let direction: i16 = trip.direction_id.map(|d| d as i16).unwrap_or(0);
        let trip_headsign = trip.trip_headsign.as_deref().unwrap_or("");

        let (dir0, dir1) = route_dir_stops
            .entry(trip.route_id.clone())
            .or_insert_with(|| (Vec::new(), Vec::new()));

        for st in &trip.stop_times {
            // Collect stops for opposite-stop matching
            if direction == 0 {
                if !dir0.contains(&st.stop.id) {
                    dir0.push(st.stop.id.clone());
                }
            } else if direction == 1 {
                if !dir1.contains(&st.stop.id) {
                    dir1.push(st.stop.id.clone());
                }
            }

            // Prefer per-stop headsign, fall back to trip headsign
            let raw_headsign = st
                .stop_headsign
                .as_deref()
                .filter(|s| !s.trim().is_empty())
                .unwrap_or(trip_headsign);
            let headsign = normalize_headsign(&trip.route_id, raw_headsign);

            let key: RouteStopKey = (trip.route_id.clone(), st.stop.id.clone(), direction);
            let sequence = st.stop_sequence as i16;

            let entry = accum.entry(key).or_insert_with(|| Accumulator {
                min_sequence: sequence,
                headsign_counts: HashMap::new(),
            });

            if sequence < entry.min_sequence {
                entry.min_sequence = sequence;
            }
            if !headsign.is_empty() {
                *entry.headsign_counts.entry(headsign).or_insert(0) += 1;
            }
        }
    }

    // Map: route_id -> (stop_id -> opposite_stop_id)
    let opposite_maps: HashMap<String, HashMap<String, String>> = route_dir_stops
        .into_iter()
        .map(|(route_id, (dir0, dir1))| {
            let map = compute_opposite_stops(&dir0, &dir1, &stop_geom_map, MAX_OPPOSITE_DIST);
            (route_id, map)
        })
        .collect();

    let result: Vec<RouteStop> = accum
        .into_iter()
        .map(|((route_id, stop_id, direction), acc)| {
            let headsign = acc
                .headsign_counts
                .into_iter()
                .max_by_key(|(_, count)| *count)
                .map(|(h, _)| h)
                .unwrap_or_else(|| "Unknown".into());

            let opposite_stop_id = opposite_maps
                .get(&route_id)
                .and_then(|m| m.get(&stop_id).cloned());

            RouteStop {
                route_id,
                stop_id,
                stop_sequence: acc.min_sequence,
                data: RouteStopData::NjtBus {
                    headsign,
                    direction,
                    opposite_stop_id,
                },
            }
        })
        .collect();

    // TODO: figure out what to do about bus stops where the same route id but different direction have the same stop_id
    // for example route_id=605 stop_id=15142 count=2 examples=["(route_id=605, stop_id=15142, direction=1, headsign=Quaker Br Mall via Griggs Frm)", "(route_id=605, stop_id=15142, direction=0, headsign=Princeton Montgomery Twp via Griggs Frm)"]

    // TODO: remove this after testing route stop directions are working
    #[cfg(debug_assertions)]
    {
        let mut norm_map: HashMap<(String, String), Vec<&RouteStop>> = HashMap::new();
        for rs in &result {
            let key = (rs.route_id.to_uppercase(), rs.stop_id.to_uppercase());
            norm_map.entry(key).or_default().push(rs);
        }
        for ((rid, sid), list) in norm_map {
            if list.len() > 1 {
                let examples: Vec<_> = list
                    .iter()
                    .take(5)
                    .map(|rs| match &rs.data {
                        RouteStopData::NjtBus {
                            headsign,
                            direction,
                            ..
                        } => format!(
                            "(route_id={}, stop_id={}, direction={}, headsign={})",
                            rs.route_id, rs.stop_id, direction, headsign
                        ),
                        other => format!(
                            "(route_id={}, stop_id={}, data={:?})",
                            rs.route_id, rs.stop_id, other
                        ),
                    })
                    .collect();
                tracing::warn!(
                    "duplicate route_stop after uppercase normalization: route_id={} stop_id={} count={} examples={:?}",
                    rid,
                    sid,
                    list.len(),
                    examples
                );
            }
        }
    }

    // The database schema only permits one row per (route_id, stop_id), so we
    // need to collapse any entries that differ only by `direction`.  When two
    // directions are present we arbitrarily keep the one with the smaller
    // `stop_sequence` (matching what `StopStore` will do when deduplicating).
    //
    // The debug check above will still warn about duplicates, but this step
    // prevents the insertion error when running the importer.

    // perform deduplication in-place to avoid large allocations
    let mut deduped: HashMap<(String, String), RouteStop> = HashMap::new();
    for rs in result.into_iter() {
        let key = (rs.route_id.to_uppercase(), rs.stop_id.to_uppercase());
        deduped
            .entry(key)
            .and_modify(|existing| {
                if rs.stop_sequence < existing.stop_sequence {
                    *existing = rs.clone();
                }
            })
            .or_insert(rs);
    }

    deduped.into_values().collect()
}

// --- Opposite-stop matching ---
// TODO: combine mta_bus and njt_bus logic into a common module
/// For each stop in `dir0_ids`, find the nearest stop in `dir1_ids` (and vice-versa) whose
/// projected distance is within `max_dist` (EPSG:6538 meters).
/// Returns a map of `stop_id → opposite_stop_id`.
fn compute_opposite_stops(
    dir0_ids: &[String],
    dir1_ids: &[String],
    stop_geom_map: &HashMap<String, Point<f64>>,
    max_dist: f64,
) -> HashMap<String, String> {
    let mut opposite_map: HashMap<String, String> = HashMap::new();

    // dir0 → nearest dir1 match
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

    // dir1 → nearest dir0 match
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

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Normalize a destination headsign by removing a repeated route-id prefix and
/// converting the remaining text to title case.
fn normalize_headsign(route_id: &str, headsign: &str) -> String {
    let compact = normalize_whitespace(headsign);
    let stripped = strip_route_id_prefix(route_id, &compact);
    normalize_title(stripped)
}

/// Remove a route-id token from the start of a headsign when present.
///
/// Examples:
/// - "509 ATLANTIC CITY" -> "ATLANTIC CITY"
/// - "509-ATLANTIC CITY" -> "ATLANTIC CITY"
fn strip_route_id_prefix<'a>(route_id: &str, headsign: &'a str) -> &'a str {
    let trimmed = headsign.trim();
    if route_id.is_empty() || trimmed.is_empty() {
        return trimmed;
    }

    let first_token_end = trimmed
        .find(|c: char| c.is_whitespace() || matches!(c, '-' | ':' | '/' | '.'))
        .unwrap_or(trimmed.len());

    let first_token = &trimmed[..first_token_end];
    if !first_token.eq_ignore_ascii_case(route_id) {
        return trimmed;
    }

    trimmed[first_token_end..]
        .trim_start_matches(|c: char| c.is_whitespace() || matches!(c, '-' | ':' | '/' | '.'))
        .trim()
}
