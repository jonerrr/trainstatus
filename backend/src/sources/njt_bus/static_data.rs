use crate::network::dataset::NetworkDataset;
use crate::utils::normalization::{normalize_title, normalize_whitespace};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    io::Cursor,
    time::Duration,
};

use crate::network::expansion::expand_gtfs;
use crate::network::index::TripPatternRevision;
use crate::network::types::route::Route;
use crate::network::types::route::RouteData;
use crate::network::types::stop::RouteStop;
use crate::network::types::stop::RouteStopData;
use crate::network::types::stop::Stop;
use crate::network::types::stop::StopData;
use crate::network::types::stop::{NjtBusStopData, NjtBusStopDirection};
use crate::source::Source;
use crate::sources::StaticAdapter;
use anyhow::Context;
use async_trait::async_trait;
use geo::Point;
use tracing::warn;

use super::patterns::{self, PatternDataset, PatternFeature};

const NJT_DEFAULT_COLOR: &str = "1A2B57";
const NJT_GTFS_URL: &str = "https://pcsdata.njtransit.com/api/GTFSG2/getGTFS";
pub struct NjtBusStatic;

#[async_trait]
impl StaticAdapter for NjtBusStatic {
    fn source(&self) -> Source {
        Source::NjtBus
    }

    fn refresh_interval(&self) -> Duration {
        Duration::from_secs(60 * 60 * 24) // 24 hours
    }

    async fn collect(&self) -> anyhow::Result<NetworkDataset> {
        let token = super::get_token()
            .await
            .context("NJT authentication failed")?;

        let gtfs_bytes = download_gtfs(&token)
            .await
            .context("NJT GTFS download failed")?;

        // Parse GTFS in a blocking task
        let gtfs = tokio::task::spawn_blocking(move || {
            // GIS supplies geometry; retain trip shape IDs without parsing the
            // large, unused GTFS shapes.txt coordinate table.
            gtfs_structures::GtfsReader::default()
                .read_shapes(false)
                .raw()
                .read_from_reader(Cursor::new(gtfs_bytes))
                .and_then(gtfs_structures::Gtfs::try_from)
        })
        .await
        .context("GTFS parse task panicked")?
        .context("Failed to parse NJT GTFS")?;

        let mut scheduled_trips = expand_gtfs(Source::NjtBus, &gtfs);

        // Collapse parent/child gate stops that share a public `stop_code` into a
        // single canonical stop, and build the `child_stop_id -> canonical_id`
        // remap used to rewrite route_stops, cached trips, and realtime feeds.
        let (stops, stop_remap) = collapse_stops(&gtfs);

        // Rewrite cached trip stop ids so any consumer joining to `static.stop`
        // resolves to the canonical (collapsed) stop.
        for trip in scheduled_trips.iter_mut() {
            for st in trip.stop_times.iter_mut() {
                if let Some(canonical) = stop_remap.get(&st.stop_id) {
                    st.stop_id = canonical.clone();
                }
            }
        }

        let features = patterns::fetch_patterns()
            .await
            .context("Failed to fetch NJT operating patterns")?;
        let (gtfs, patterns) = tokio::task::spawn_blocking(move || {
            let patterns = patterns::build_patterns(&gtfs, features);
            (gtfs, patterns)
        })
        .await
        .context("NJT pattern matching task panicked")?;
        anyhow::ensure!(
            !patterns.shapes.is_empty(),
            "No valid NJT operating patterns; retaining previous import"
        );
        let dataset = build_static_dataset(&gtfs, patterns, scheduled_trips, stops, &stop_remap);

        Ok(dataset)
    }
}

// ── GTFS download ─────────────────────────────────────────────────────────────

async fn download_gtfs(token: &str) -> anyhow::Result<Vec<u8>> {
    let form = reqwest::multipart::Form::new().text("token", token.to_owned());

    let bytes = crate::integrations::http::static_data()
        .post(NJT_GTFS_URL)
        .multipart(form)
        .send()
        .await?
        .error_for_status()?
        .bytes()
        .await?;

    Ok(bytes.to_vec())
}

#[cfg(feature = "fixture-capture")]
pub async fn capture_fixtures() -> anyhow::Result<std::collections::BTreeMap<String, Vec<u8>>> {
    let token = super::get_token().await?;
    let gtfs_zip = download_gtfs(&token).await?;
    let patterns = patterns::fetch_patterns().await?;
    Ok(std::collections::BTreeMap::from([
        ("gtfs.zip".to_string(), gtfs_zip),
        (
            "operating_patterns.json".to_string(),
            serde_json::to_vec(&patterns)?,
        ),
    ]))
}

pub struct PatternStaticBuild {
    pub dataset: NetworkDataset,
    pub revision: TripPatternRevision,
}

/// Pure fixture seam shared with the production import.
pub fn build_static_dataset_from_patterns(
    gtfs: &gtfs_structures::Gtfs,
    features: Vec<PatternFeature>,
) -> PatternStaticBuild {
    let patterns = patterns::build_patterns(gtfs, features);
    let (stops, remap) = collapse_stops(gtfs);
    let dataset = build_static_dataset(gtfs, patterns, vec![], stops, &remap);
    PatternStaticBuild {
        // TODO: why is this clone necessary?
        revision: TripPatternRevision {
            patterns: dataset.trip_patterns.clone(),
            stop_remap: dataset.stop_remap.clone(),
        },
        dataset,
    }
}

// ── Build routes ──────────────────────────────────────────────────────────────

fn build_routes(gtfs: &gtfs_structures::Gtfs) -> Vec<Route> {
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
    patterns: PatternDataset,
    mut scheduled_trips: Vec<crate::network::schedule::ScheduledTrip>,
    stops: Vec<Stop>,
    stop_remap: &HashMap<String, String>,
) -> NetworkDataset {
    for trip in &mut scheduled_trips {
        trip.headsign = normalize_headsign(&trip.route_id, &trip.headsign);
    }

    let PatternDataset { shapes, trips } = patterns;

    NetworkDataset {
        source: Source::NjtBus,
        routes: build_routes(gtfs),
        stops,
        route_stops: build_route_stops(gtfs, stop_remap),
        shapes,
        scheduled_trips,
        trip_patterns: trips,
        stop_remap: stop_remap.clone(),
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
            nearby_stops: vec![],
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

struct Accumulator {
    min_sequence: i16,
    headsigns: BTreeMap<Option<i16>, HashMap<String, usize>>,
}

fn build_route_stops(
    gtfs: &gtfs_structures::Gtfs,
    stop_remap: &HashMap<String, String>,
) -> Vec<RouteStop> {
    let mut memberships: HashMap<(String, String), Accumulator> = HashMap::new();
    for trip in gtfs.trips.values() {
        let direction = trip.direction_id.map(|d| d as i16);
        for st in &trip.stop_times {
            let stop_id = stop_remap.get(&st.stop.id).unwrap_or(&st.stop.id).clone();
            let raw_headsign = st
                .stop_headsign
                .as_deref()
                .filter(|s| !s.trim().is_empty())
                .or(trip.trip_headsign.as_deref())
                .unwrap_or("");
            let headsign = normalize_headsign(&trip.route_id, raw_headsign);
            let entry = memberships
                .entry((trip.route_id.clone(), stop_id))
                .or_insert_with(|| Accumulator {
                    min_sequence: st.stop_sequence as i16,
                    headsigns: BTreeMap::new(),
                });
            entry.min_sequence = entry.min_sequence.min(st.stop_sequence as i16);
            let counts = entry.headsigns.entry(direction).or_default();
            if !headsign.is_empty() {
                *counts.entry(headsign).or_default() += 1;
            }
        }
    }
    memberships
        .into_iter()
        .map(|((route_id, stop_id), entry)| RouteStop {
            route_id,
            stop_id,
            stop_sequence: entry.min_sequence,
            data: RouteStopData::NjtBus {
                directions: entry
                    .headsigns
                    .into_iter()
                    .map(|(direction_id, counts)| {
                        // Resolve equal-frequency labels lexically so import order cannot change them.
                        let headsign = counts
                            .into_iter()
                            .max_by(|(a, ac), (b, bc)| ac.cmp(bc).then_with(|| b.cmp(a)))
                            .map(|(headsign, _)| headsign)
                            .unwrap_or_default();
                        NjtBusStopDirection {
                            direction_id,
                            headsign,
                        }
                    })
                    .collect(),
            },
        })
        .collect()
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
#[path = "tests/static_data.rs"]
mod tests;
