use std::{collections::HashMap, io::Cursor, time::Duration};

use anyhow::Context;
use async_trait::async_trait;
use geo::{LineString, MultiLineString, Point};
use geojson::GeoJson;
use regex::Regex;

use crate::{
    models::{
        route::{Route, RouteData},
        source::Source,
        stop::{NjtBusStopData, RouteStop, RouteStopData, Stop, StopData},
    },
    sources::StaticAdapter,
    stores::{route::RouteStore, stop::StopStore},
};

const NJT_DEFAULT_COLOR: &str = "0033A0";
const NJT_GTFS_URL: &str = "https://pcsdata.njtransit.com/api/GTFSG2/getGTFS";
const NJT_ARCGIS_URL: &str = "https://services6.arcgis.com/M0t0HPE53pFK525U/arcgis/rest/services/Bus_Lines_of_NJ_Transit/FeatureServer/1/query?where=1%3D1&outFields=*&outSR=4326&f=geojson";

pub struct NjtBusStatic;

#[async_trait]
impl StaticAdapter for NjtBusStatic {
    fn source(&self) -> Source {
        Source::NjtBus
    }

    fn refresh_interval(&self) -> Duration {
        Duration::from_secs(60 * 60 * 24) // 24 hours
    }

    async fn import(&self, route_store: &RouteStore, stop_store: &StopStore) -> anyhow::Result<()> {
        // 1. Authenticate
        let token = super::get_token()
            .await
            .context("NJT authentication failed")?;

        // 2. Download GTFS zip as raw bytes
        let gtfs_bytes = download_gtfs(&token)
            .await
            .context("NJT GTFS download failed")?;

        // 3. Parse GTFS in a blocking task (synchronous CPU/IO work)
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

        // 4. Fetch route geometries from NJT ArcGIS REST endpoint
        let route_geom_map = fetch_route_geometries()
            .await
            .context("Failed to fetch NJT route geometries")?;

        // 5–7. Build all entities
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

        // 8. Persist
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

// ── Route geometry (ArcGIS) ───────────────────────────────────────────────────

/// Fetch GeoJSON from NJT ArcGIS and return a map of route_id → MultiLineString.
async fn fetch_route_geometries() -> anyhow::Result<HashMap<String, MultiLineString>> {
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

    for feature in fc.features {
        let Some(props) = &feature.properties else {
            continue;
        };
        let Some(val) = props.get("LINE_STRING") else {
            continue;
        };
        let route_id = match val.as_str() {
            Some(s) => s.to_owned(),
            None => val.to_string().trim_matches('"').to_owned(),
        };

        let Some(ref geom) = feature.geometry else {
            continue;
        };

        match &geom.value {
            geojson::Value::LineString(positions) => {
                route_lines
                    .entry(route_id)
                    .or_default()
                    .push(positions_to_linestring(positions));
            }
            geojson::Value::MultiLineString(multi) => {
                let lines = route_lines.entry(route_id).or_default();
                for positions in multi {
                    lines.push(positions_to_linestring(positions));
                }
            }
            _ => {}
        }
    }

    Ok(route_lines
        .into_iter()
        .map(|(id, lines)| (id, MultiLineString::new(lines)))
        .collect())
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
                long_name: r.long_name.clone().unwrap_or_default(),
                short_name: r.short_name.clone().unwrap_or_default(),
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
            let name = s
                .name
                .as_deref()
                .map(title_case)
                .unwrap_or_else(|| s.id.clone());

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
    let mut accum: HashMap<RouteStopKey, Accumulator> = HashMap::new();

    for trip in gtfs.trips.values() {
        let direction: i16 = trip.direction_id.map(|d| d as i16).unwrap_or(0);
        let trip_headsign = trip.trip_headsign.as_deref().unwrap_or("").to_owned();

        for st in &trip.stop_times {
            // Prefer per-stop headsign, fall back to trip headsign
            let headsign = st
                .stop_headsign
                .as_deref()
                .filter(|s| !s.is_empty())
                .unwrap_or(trip_headsign.as_str())
                .to_owned();

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

    let mut result: Vec<RouteStop> = accum
        .into_iter()
        .map(|((route_id, stop_id, direction), acc)| {
            let headsign = acc
                .headsign_counts
                .into_iter()
                .max_by_key(|(_, count)| *count)
                .map(|(h, _)| h)
                .unwrap_or_default();

            RouteStop {
                route_id,
                stop_id,
                stop_sequence: acc.min_sequence,
                data: RouteStopData::NjtBus {
                    headsign,
                    direction,
                },
            }
        })
        .collect();
    // TODO: remove this after testing route stop directions are working
    // debug: check for any duplicates once we uppercase keys
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

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Convert ALL-CAPS strings (common in NJT GTFS) to Title Case.
/// Capitalises the first letter and any letter following a non-word character.
fn title_case(s: &str) -> String {
    let re = Regex::new(r"\W").unwrap();
    let mut name = s.to_lowercase();

    // Collect indices of the character that follows each non-word character
    let capitalize_at: Vec<usize> = re.find_iter(&name).map(|m| m.end()).collect();

    for idx in capitalize_at {
        if let Some(c) = name[idx..].chars().next() {
            let upper = c.to_uppercase().to_string();
            let end = idx + c.len_utf8();
            name.replace_range(idx..end, &upper);
        }
    }

    // Capitalise the very first character
    let mut chars = name.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
    }
}
