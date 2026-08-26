use std::{
    collections::{HashMap, HashSet},
    io::Cursor,
    time::Duration,
};

use crate::{
    models::{
        route::{Route, RouteData},
        source::Source,
        static_dataset::StaticDataset,
        stop::{NjtBusStopData, RouteStop, RouteStopData, Stop, StopData},
    },
    sources::{StaticAdapter, normalize_title, normalize_whitespace},
    stores::{route::RouteStore, static_cache::StaticCacheStore, stop::StopStore},
    utils::static_cache::expand_gtfs,
};
use anyhow::Context;
use async_trait::async_trait;
use geo::{Distance, Euclidean, LineString, MultiLineString, Point};
use geojson::GeoJson;
use proj4rs::{Proj, transform::transform};
use tracing::{info, warn};

use super::{NJT_BUS_ROUTES_URL, NjtApi};

// TODO: use the per-service dataset instead since we are now storing route geometries as shapes
const NJT_DEFAULT_COLOR: &str = "1A2B57";
const MAX_OPPOSITE_DIST: f64 = 500.0;
const NJT_GTFS_URL: &str = "https://pcsdata.njtransit.com/api/GTFSG2/getGTFS";
const NJT_ARCGIS_URL: &str = "https://services6.arcgis.com/M0t0HPE53pFK525U/arcgis/rest/services/Bus_Lines_of_NJ_Transit/FeatureServer/1/query?where=1%3D1&outFields=*&outSR=4326&f=geojson";

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
struct BusRoute {
    #[serde(rename = "BusRouteID")]
    bus_route_id: String,
    #[serde(rename = "BusRouteDescription")]
    bus_route_description: String,
}

// TODO: retest using valhalla to snap routes since I think they fixed some issues with overlapping roads in the latest version of valhalla
pub struct NjtBusStatic;

// TODO: setup shape insertion
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

        let mut cached_trips = expand_gtfs(Source::NjtBus, &gtfs);

        // Collapse parent/child gate stops that share a public `stop_code` into a
        // single canonical stop, and build the `child_stop_id -> canonical_id`
        // remap used to rewrite route_stops, cached trips, and realtime feeds.
        let (stops, stop_remap) = collapse_stops(&gtfs);

        // Rewrite cached trip stop ids so any consumer joining to `static.stop`
        // resolves to the canonical (collapsed) stop.
        for trip in cached_trips.iter_mut() {
            for st in trip.stop_times.iter_mut() {
                if let Some(canonical) = stop_remap.get(&st.stop_id) {
                    st.stop_id = canonical.clone();
                }
            }
        }

        let bus_routes_token = super::get_token(NjtApi::BusDv2)
            .await
            .context("NJT BUSDV2 authentication failed")?;

        let bus_routes = fetch_bus_routes(&bus_routes_token)
            .await
            .context("Failed to fetch BUSDV2 route metadata")?;

        let route_geom_map = fetch_route_geometries(&bus_routes, &gtfs)
            .await
            .context("Failed to fetch NJT route geometries")?;

        let dataset =
            build_static_dataset(&gtfs, &route_geom_map, cached_trips, stops, &stop_remap);

        // TODO: move this to a standardized print method in StaticDataset
        #[cfg(debug_assertions)]
        {
            // additional sanity check: look for duplicates in the raw vector (before storing)
            let mut seen: HashMap<(String, String), usize> = HashMap::new();
            for rs in &dataset.route_stops {
                let key = (rs.route_id.clone(), rs.stop_id.clone());
                *seen.entry(key).or_insert(0) += 1;
            }
            for ((rid, sid), count) in seen {
                if count > 1 {
                    tracing::warn!(
                        route_id = %rid,
                        stop_id = %sid,
                        count,
                        "Raw route_stops vector contains duplicate entries"
                    );
                }
            }
        }

        dataset
            .persist(route_store, stop_store, static_cache_store)
            .await
            .context("Failed to persist NJT static dataset")?;

        // Publish the remap only after the canonical stops are persisted, so the
        // realtime pipeline never remaps to a stop that isn't in the DB yet.
        static_cache_store.set_stop_remap(Source::NjtBus, stop_remap);

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

#[cfg(feature = "fixture-capture")]
pub async fn capture_fixtures() -> anyhow::Result<std::collections::BTreeMap<String, Vec<u8>>> {
    let gtfs_token = super::get_token(NjtApi::GtfsG2)
        .await
        .context("NJT GTFS authentication failed")?;
    let bus_routes_token = super::get_token(NjtApi::BusDv2)
        .await
        .context("NJT BUSDV2 authentication failed")?;

    let gtfs_zip = download_gtfs(&gtfs_token).await?;
    let bus_routes = fetch_bus_routes(&bus_routes_token).await?;
    let route_geometries = reqwest::Client::new()
        .get(NJT_ARCGIS_URL)
        .send()
        .await?
        .error_for_status()?
        .bytes()
        .await?;

    let mut fixtures = std::collections::BTreeMap::new();
    fixtures.insert("gtfs.zip".to_string(), gtfs_zip);
    fixtures.insert(
        "bus_routes.json".to_string(),
        serde_json::to_vec_pretty(&bus_routes)?,
    );
    fixtures.insert(
        "route_geometries.geojson".to_string(),
        route_geometries.to_vec(),
    );
    Ok(fixtures)
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
                line_string_id
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
    _geom_map: &HashMap<String, MultiLineString>,
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
                text_color: "#FFFFFF".into(),
                data: RouteData::NjtBus,
            }
        })
        .collect()
}

fn build_static_dataset(
    gtfs: &gtfs_structures::Gtfs,
    route_geom_map: &HashMap<String, MultiLineString>,
    cached_trips: Vec<crate::models::static_cache::CachedTrip>,
    stops: Vec<Stop>,
    stop_remap: &HashMap<String, String>,
) -> StaticDataset {
    StaticDataset {
        source: Source::NjtBus,
        routes: build_routes(gtfs, route_geom_map),
        stops,
        route_stops: build_route_stops(gtfs, stop_remap),
        shapes: vec![],
        cached_trips,
    }
}

// ── Build stops ───────────────────────────────────────────────────────────────

/// Collapse the NJT parent/child stop family (a parent station of
/// `location_type=1`, its gate children of `location_type=0`, and a "no-gate"
/// child) into a single canonical stop per public `stop_code`.
///
/// `stop_times.txt` (and the realtime feed) only ever reference the
/// `location_type=0` children, never the parent station, so we pick a
/// representative id per group and remap every other member to it. Returns the
/// canonical stops plus a `child_stop_id -> representative_id` remap (identity
/// members are omitted).
fn collapse_stops(gtfs: &gtfs_structures::Gtfs) -> (Vec<Stop>, HashMap<String, String>) {
    // Group members by public stop_code (fall back to id, matching legacy behavior).
    let mut groups: HashMap<String, Vec<&gtfs_structures::Stop>> = HashMap::new();
    for s in gtfs.stops.values() {
        let code = s.code.clone().unwrap_or_else(|| s.id.clone());
        groups.entry(code).or_default().push(s.as_ref());
    }

    let mut stops: Vec<Stop> = Vec::with_capacity(groups.len());
    let mut remap: HashMap<String, String> = HashMap::new();

    for (stop_code, members) in groups {
        let rep = choose_representative(&members);

        // All members of a group share coordinates, but the representative (or a
        // parent station) can occasionally lack them; fall back to any member.
        let coords = rep
            .longitude
            .zip(rep.latitude)
            .or_else(|| members.iter().find_map(|m| m.longitude.zip(m.latitude)));
        let Some((lon, lat)) = coords else {
            // No geometry anywhere in the group — skip (matches legacy filter_map).
            continue;
        };

        let raw_name = rep.name.as_deref().unwrap_or(&rep.id);
        let name = normalize_title(raw_name);

        for m in &members {
            if m.id != rep.id {
                remap.insert(m.id.clone(), rep.id.clone());
            }
        }

        stops.push(Stop {
            id: rep.id.clone(),
            name,
            geom: Point::new(lon, lat).into(),
            transfers: vec![],
            routes: vec![],
            data: StopData::NjtBus(NjtBusStopData { stop_code }),
        });
    }

    // Defensive: the validator and `StopStore::save_all` key stops on the
    // uppercased id, so a case-insensitive collision would bail the whole import.
    // Drop such duplicates here (keep the first) rather than crash.
    let mut seen: HashSet<String> = HashSet::new();
    stops.retain(|s| {
        if seen.insert(s.id.to_uppercase()) {
            true
        } else {
            warn!(stop_id = %s.id, "Dropping NJT stop with case-insensitive duplicate id");
            false
        }
    });

    (stops, remap)
}

/// Pick the canonical member of a `stop_code` group:
/// 1. the `location_type=1` parent station, if present;
/// 2. otherwise, among plain stops with no `parent_station`, the smallest id;
/// 3. otherwise the smallest id overall.
fn choose_representative<'a>(members: &[&'a gtfs_structures::Stop]) -> &'a gtfs_structures::Stop {
    if let Some(parent) = members
        .iter()
        .copied()
        .find(|m| m.location_type == gtfs_structures::LocationType::StopArea)
    {
        return parent;
    }

    let no_parent: Vec<&'a gtfs_structures::Stop> = members
        .iter()
        .copied()
        .filter(|m| m.parent_station.is_none())
        .collect();
    let pool = if no_parent.is_empty() {
        members.to_vec()
    } else {
        no_parent
    };

    pool.into_iter()
        .min_by(|a, b| stop_id_sort_key(&a.id).cmp(&stop_id_sort_key(&b.id)))
        .expect("stop_code group always has at least one member")
}

/// Deterministic, numeric-aware ordering for stop ids (NJT ids are usually
/// numeric; non-numeric ids sort last, tie-broken lexicographically).
fn stop_id_sort_key(id: &str) -> (u64, &str) {
    (id.parse::<u64>().unwrap_or(u64::MAX), id)
}

// ── Build route_stops ─────────────────────────────────────────────────────────

/// Deduplication key: (route_id, stop_id, direction_id)
type RouteStopKey = (String, String, i16);

struct Accumulator {
    min_sequence: i16,
    /// headsign text → occurrence count
    headsign_counts: HashMap<String, usize>,
}

fn build_route_stops(
    gtfs: &gtfs_structures::Gtfs,
    stop_remap: &HashMap<String, String>,
) -> Vec<RouteStop> {
    // Resolve a raw GTFS stop id to its canonical (collapsed) stop id.
    let canonical = |id: &str| -> String {
        stop_remap
            .get(id)
            .cloned()
            .unwrap_or_else(|| id.to_string())
    };

    let proj_wgs84 = Proj::from_epsg_code(4326).expect("Failed to create WGS84 proj");
    let proj_ny = Proj::from_epsg_code(6538).expect("Failed to create NY proj");

    // Build a map of canonical stop_id -> projected Point. Members of a collapsed
    // group share coordinates, so keying by canonical id is unambiguous.
    let stop_geom_map: HashMap<String, Point<f64>> = gtfs
        .stops
        .values()
        .filter_map(|s| {
            let lat = s.latitude?;
            let lon = s.longitude?;
            let mut point = Point::new(lon.to_radians(), lat.to_radians());
            transform(&proj_wgs84, &proj_ny, &mut point).ok()?;
            Some((canonical(&s.id), point))
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
            let stop_id = canonical(&st.stop.id);

            // Collect stops for opposite-stop matching
            if direction == 0 {
                if !dir0.contains(&stop_id) {
                    dir0.push(stop_id.clone());
                }
            } else if direction == 1 && !dir1.contains(&stop_id) {
                dir1.push(stop_id.clone());
            }

            // Prefer per-stop headsign, fall back to trip headsign
            let raw_headsign = st
                .stop_headsign
                .as_deref()
                .filter(|s| !s.trim().is_empty())
                .unwrap_or(trip_headsign);
            let headsign = normalize_headsign(&trip.route_id, raw_headsign);

            let key: RouteStopKey = (trip.route_id.clone(), stop_id, direction);
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

    // A route serving the same stop in both directions produces two entries that
    // differ only by `direction`; the dedup below collapses them. This is normal
    // and expected, so just tally how many pairs get collapsed (one summary line)
    // rather than emitting a WARN per pair — which floods the terminal on a feed
    // the size of NJT's.
    #[cfg(debug_assertions)]
    {
        let mut counts: HashMap<(String, String), usize> = HashMap::new();
        for rs in &result {
            let key = (rs.route_id.to_uppercase(), rs.stop_id.to_uppercase());
            *counts.entry(key).or_insert(0) += 1;
        }
        let collapsed = counts.values().filter(|&&c| c > 1).count();
        if collapsed > 0 {
            tracing::debug!(
                collapsed_pairs = collapsed,
                "njt_bus route_stops with the same (route_id, stop_id) across directions; collapsing to one row each"
            );
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::stop::StopData;
    use gtfs_structures::{Gtfs, LocationType, Stop as GtfsStop};
    use std::sync::Arc;

    fn make_stop(
        id: &str,
        code: &str,
        location_type: LocationType,
        parent: Option<&str>,
    ) -> GtfsStop {
        GtfsStop {
            id: id.to_string(),
            code: Some(code.to_string()),
            name: Some(format!("STOP {id}")),
            location_type,
            parent_station: parent.map(str::to_string),
            longitude: Some(-73.9392),
            latitude: Some(40.84899),
            ..Default::default()
        }
    }

    fn gtfs_with(stops: Vec<GtfsStop>) -> Gtfs {
        let mut gtfs = Gtfs::default();
        for s in stops {
            gtfs.stops.insert(s.id.clone(), Arc::new(s));
        }
        gtfs
    }

    fn stop_code_of(stop: &Stop) -> &str {
        match &stop.data {
            StopData::NjtBus(d) => &d.stop_code,
            other => panic!("expected NjtBus stop data, got {other:?}"),
        }
    }

    #[test]
    fn collapses_parent_child_family_to_representative_parent() {
        // GW Bridge terminal: parent station + a gate child + a "no-gate" child,
        // all sharing stop_code 32640.
        let gtfs = gtfs_with(vec![
            make_stop("16339", "32640", LocationType::StopArea, None),
            make_stop("16957", "32640", LocationType::StopPoint, Some("16339")),
            make_stop("16969", "32640", LocationType::StopPoint, None),
        ]);

        let (stops, remap) = collapse_stops(&gtfs);

        assert_eq!(stops.len(), 1, "family collapses to a single stop");
        assert_eq!(stops[0].id, "16339", "representative is the parent station");
        assert_eq!(stop_code_of(&stops[0]), "32640");
        // Both children remap to the parent; realtime feeds reference these.
        assert_eq!(remap.get("16957"), Some(&"16339".to_string()));
        assert_eq!(remap.get("16969"), Some(&"16339".to_string()));
        // The representative itself is identity (absent from the remap).
        assert!(remap.get("16339").is_none());
    }

    #[test]
    fn collapses_parentless_group_to_smallest_numeric_id() {
        // Two plain stops (no parent station) sharing a stop_code.
        let gtfs = gtfs_with(vec![
            make_stop("15372", "30539", LocationType::StopPoint, None),
            make_stop("1", "30539", LocationType::StopPoint, None),
        ]);

        let (stops, remap) = collapse_stops(&gtfs);

        assert_eq!(stops.len(), 1);
        assert_eq!(stops[0].id, "1", "smallest numeric id wins");
        assert_eq!(remap.get("15372"), Some(&"1".to_string()));
    }

    #[test]
    fn single_id_stop_is_untouched_and_absent_from_remap() {
        let gtfs = gtfs_with(vec![make_stop(
            "500",
            "40000",
            LocationType::StopPoint,
            None,
        )]);

        let (stops, remap) = collapse_stops(&gtfs);

        assert_eq!(stops.len(), 1);
        assert_eq!(stops[0].id, "500");
        assert!(remap.is_empty(), "1:1 stops need no remap entry");
    }

    #[test]
    fn stop_id_sort_key_orders_numerically() {
        assert!(stop_id_sort_key("2") < stop_id_sort_key("10"));
        // Non-numeric ids sort after numeric ones.
        assert!(stop_id_sort_key("999999") < stop_id_sort_key("A1"));
    }

    /// End-to-end against the captured NJT GTFS feed. Ignored by default because
    /// it parses a ~55MB zip; run with `cargo test -- --ignored njt_fixture`.
    #[test]
    #[ignore]
    fn collapse_and_route_stops_against_real_fixture() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/njt_bus/static/basic/raw/gtfs.zip"
        );
        let gtfs = Gtfs::from_path(path).expect("parse fixture gtfs");

        let (stops, remap) = collapse_stops(&gtfs);

        // Every collapsed stop id is unique (no duplicate crash).
        let stop_ids: HashSet<&str> = stops.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(stop_ids.len(), stops.len(), "canonical stop ids are unique");

        // The GW Bridge terminal family (stop_code 32640) collapses to a single stop.
        let gw: Vec<&Stop> = stops
            .iter()
            .filter(|s| stop_code_of(s) == "32640")
            .collect();
        assert_eq!(gw.len(), 1, "stop_code 32640 collapses to one stop");
        assert_eq!(gw[0].id, "16339", "representative is the parent station");
        assert_eq!(remap.get("16957"), Some(&"16339".to_string()));

        // Key invariant: every route_stop references a canonical stop that exists
        // (the old importer left parent stations orphaned / referenced raw children).
        let route_stops = build_route_stops(&gtfs, &remap);
        assert!(!route_stops.is_empty());
        for rs in &route_stops {
            assert!(
                stop_ids.contains(rs.stop_id.as_str()),
                "route_stop references unknown stop {}",
                rs.stop_id
            );
        }
        // The collapsed GW terminal now carries routes (it previously had none).
        assert!(
            route_stops.iter().any(|rs| rs.stop_id == "16339"),
            "collapsed GW terminal should have route_stops"
        );
    }
}
