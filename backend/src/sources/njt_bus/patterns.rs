//! Exact GTFS shape_id -> NJT GIS operating-pattern geometry.
use std::collections::{BTreeMap, HashMap, HashSet};

use anyhow::{Context, ensure};
use geo::{Coord, LineString, Point};
use serde::{Deserialize, Serialize};

use crate::{
    models::{shape::Shape, source::Source},
    sources::normalize_id,
    stores::static_cache::TripPattern,
    trajectory::geometry::{
        build_shape_geometry, project_point_onto_line, project_wgs84_point_to_epsg,
    },
};

pub const PATTERN_URL: &str = "https://services6.arcgis.com/M0t0HPE53pFK525U/arcgis/rest/services/Bus_Operating_Patterns_of_NJ_Transit/FeatureServer/7/query";
const PAGE_SIZE: usize = 1000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternFeature {
    pub attributes: PatternAttributes,
    pub geometry: Option<PatternGeometry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub struct PatternAttributes {
    pub objectid: u64,
    pub bus_pattern: Option<String>,
    pub route_dir: Option<String>,
    pub line: Option<f64>,
    pub id: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternGeometry {
    pub paths: Vec<Vec<[f64; 2]>>,
}

#[derive(Deserialize)]
struct PatternPage {
    #[serde(default)]
    features: Vec<PatternFeature>,
    #[serde(default, rename = "exceededTransferLimit")]
    exceeded_transfer_limit: bool,
    error: Option<serde_json::Value>,
}

fn append_page(all: &mut Vec<PatternFeature>, page: PatternPage) -> anyhow::Result<bool> {
    ensure!(page.error.is_none(), "ArcGIS error: {:?}", page.error);
    ensure!(
        !page.exceeded_transfer_limit || !page.features.is_empty(),
        "ArcGIS pagination made no progress"
    );
    let mut previous = all.last().map(|f| f.attributes.objectid);
    for feature in &page.features {
        ensure!(
            previous.is_none_or(|id| feature.attributes.objectid > id),
            "ArcGIS returned duplicate or unordered object IDs"
        );
        previous = Some(feature.attributes.objectid);
    }
    all.extend(page.features);
    Ok(page.exceeded_transfer_limit)
}

pub async fn fetch_patterns() -> anyhow::Result<Vec<PatternFeature>> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()?;
    let mut features = Vec::new();
    loop {
        let page = client
            .get(PATTERN_URL)
            .query(&[
                ("where", "1=1".to_owned()),
                (
                    "outFields",
                    "OBJECTID,BUS_PATTERN,ROUTE_DIR,LINE,ID".to_owned(),
                ),
                ("outSR", "4326".to_owned()),
                ("f", "json".to_owned()),
                ("orderByFields", "OBJECTID ASC".to_owned()),
                ("resultRecordCount", PAGE_SIZE.to_string()),
                ("resultOffset", features.len().to_string()),
            ])
            .send()
            .await?
            .error_for_status()?
            .json::<PatternPage>()
            .await?;
        if !append_page(&mut features, page)? {
            break;
        }
    }
    ensure!(
        !features.is_empty(),
        "ArcGIS returned no NJT operating patterns"
    );
    Ok(features)
}

/// Multipart lines may be reordered/reversed only at identical endpoints.
/// A branch or disconnected component is not an animation path.
fn connected_line(geometry: &PatternGeometry) -> anyhow::Result<LineString> {
    let mut parts: Vec<Vec<Coord>> = Vec::new();
    for path in &geometry.paths {
        ensure!(
            path.iter().all(|[x, y]| x.is_finite()
                && y.is_finite()
                && (-180.0..=180.0).contains(x)
                && (-90.0..=90.0).contains(y)),
            "invalid WGS84 coordinates"
        );
        let mut points: Vec<_> = path.iter().map(|[x, y]| Coord { x: *x, y: *y }).collect();
        points.dedup();
        ensure!(points.len() >= 2, "degenerate path");
        parts.push(points);
    }
    ensure!(!parts.is_empty(), "empty geometry");
    let mut line = parts.remove(0);
    while !parts.is_empty() {
        let mut candidates = Vec::new();
        for (index, part) in parts.iter().enumerate() {
            for prepend in [false, true] {
                let end = if prepend { line.first() } else { line.last() };
                if end == part.first() {
                    candidates.push((index, prepend, prepend));
                }
                if end == part.last() {
                    candidates.push((index, prepend, !prepend));
                }
            }
        }
        // Joining either end is fine, but multiple choices at the same endpoint imply a branch.
        ensure!(!candidates.is_empty(), "disconnected multipart geometry");
        for prepend in [false, true] {
            ensure!(
                candidates.iter().filter(|c| c.1 == prepend).count() <= 1,
                "ambiguous multipart branch"
            );
        }
        let (index, prepend, reverse) = candidates[0];
        let mut part = parts.remove(index);
        if reverse {
            part.reverse();
        }
        if prepend {
            part.pop();
            part.extend(line);
            line = part;
        } else {
            line.extend(part.into_iter().skip(1));
        }
    }
    let line = LineString::new(line);
    ensure!(
        build_shape_geometry(&line, 6538).is_some(),
        "zero-length geometry"
    );
    Ok(line)
}

/// Determine travel orientation from ordered raw GTFS stops (before gate collapse).
fn orient_line(
    mut line: LineString,
    trips: &[&gtfs_structures::Trip],
) -> anyhow::Result<LineString> {
    let geometry = build_shape_geometry(&line, 6538).context("invalid projected geometry")?;
    let mut orientations = HashSet::new();
    let mut seen_sequences = HashSet::new();
    for trip in trips {
        let mut stops: Vec<_> = trip.stop_times.iter().collect();
        stops.sort_by_key(|st| st.stop_sequence);
        let ids: Vec<_> = stops.iter().map(|st| st.stop.id.as_str()).collect();
        if !seen_sequences.insert(ids) {
            continue;
        }
        let distances: Vec<_> = stops
            .iter()
            .filter_map(|st| {
                let point = Point::new(st.stop.longitude?, st.stop.latitude?);
                let p = project_wgs84_point_to_epsg(&point, 6538)?;
                project_point_onto_line(&p, &geometry.projected_line, &geometry.cum_dist)
            })
            .collect();
        ensure!(distances.len() >= 2, "insufficient stops for orientation");
        // Small nearest-point jitter is harmless; large reversals indicate an ambiguous loop/branch.
        let forward = distances.windows(2).all(|w| w[1] + 100.0 >= w[0]);
        let backward = distances.windows(2).all(|w| w[0] + 100.0 >= w[1]);
        let net = distances.last().unwrap() - distances[0];
        ensure!(net.abs() > 1.0, "ambiguous stop ordering");
        let reverse = net < 0.0;
        ensure!(
            if reverse { backward } else { forward },
            "backtracking stop ordering"
        );
        orientations.insert(reverse);
    }
    ensure!(orientations.len() == 1, "inconsistent trip orientations");
    if orientations.contains(&true) {
        line.0.reverse();
    }
    Ok(line)
}

pub struct PatternDataset {
    pub shapes: Vec<Shape>,
    pub trips: HashMap<String, TripPattern>,
}

pub fn build_patterns(
    gtfs: &gtfs_structures::Gtfs,
    features: Vec<PatternFeature>,
) -> PatternDataset {
    let mut by_pattern: BTreeMap<String, Vec<PatternFeature>> = BTreeMap::new();
    for feature in features {
        if let Some(id) = feature
            .attributes
            .bus_pattern
            .as_deref()
            .map(normalize_id)
            .filter(|id| !id.is_empty())
        {
            by_pattern.entry(id).or_default().push(feature);
        }
    }
    let mut gtfs_patterns: BTreeMap<String, Vec<&gtfs_structures::Trip>> = BTreeMap::new();
    for trip in gtfs.trips.values() {
        if let Some(id) = &trip.shape_id {
            gtfs_patterns
                .entry(normalize_id(id))
                .or_default()
                .push(trip);
        }
    }
    let mut shapes = Vec::new();
    // Retain metadata even for unmatched geometry so realtime trips can still
    // appear in lists when the feed omits route/direction and the dated cache misses.
    let mut trips: HashMap<_, _> = gtfs
        .trips
        .values()
        .map(|trip| {
            (
                trip.id.clone(),
                TripPattern {
                    route_id: normalize_id(&trip.route_id),
                    direction: trip.direction_id.map(|d| d as i16).unwrap_or(0),
                    shape_id: None,
                },
            )
        })
        .collect();
    let mut rejected = 0;
    let mut rejection_reasons: BTreeMap<String, usize> = BTreeMap::new();
    for (id, pattern_trips) in gtfs_patterns {
        let result = (|| -> anyhow::Result<Shape> {
            let features = by_pattern.get(&id).context("pattern absent from ArcGIS")?;
            ensure!(features.len() == 1, "duplicate pattern ID");
            let f = &features[0];
            let direction = match f
                .attributes
                .route_dir
                .as_deref()
                .and_then(|s| s.chars().last())
            {
                Some('O') => 0,
                Some('I') => 1,
                _ => anyhow::bail!("unknown operating direction"),
            };
            ensure!(
                pattern_trips
                    .iter()
                    .all(|t| t.direction_id.is_some_and(|d| d as i16 == direction)),
                "GTFS/ArcGIS direction mismatch"
            );
            let line = connected_line(f.geometry.as_ref().context("missing geometry")?)?;
            let line = orient_line(line, &pattern_trips)?;
            Ok(Shape {
                id: id.clone(),
                source: Source::NjtBus,
                geom: geo::Geometry::LineString(line).into(),
                // TODO: why are we this extra shape data? I don't think it is used anywhere else in the codebase.
                data: serde_json::json!({"provenance": "njt_arcgis_operating_patterns", "bus_pattern": id,
                    "line": f.attributes.line, "route_dir": f.attributes.route_dir, "upstream_id": f.attributes.id}),
            })
        })();
        match result {
            Ok(shape) => {
                for trip in pattern_trips {
                    trips.insert(
                        trip.id.clone(),
                        TripPattern {
                            route_id: normalize_id(&trip.route_id),
                            direction: trip.direction_id.unwrap() as i16,
                            shape_id: Some(id.clone()),
                        },
                    );
                }
                shapes.push(shape);
            }
            Err(error) => {
                rejected += 1;
                *rejection_reasons.entry(error.to_string()).or_default() += 1;
                tracing::debug!(pattern = %id, reason = %error, "Rejected NJT operating pattern");
            }
        }
    }
    tracing::info!(
        valid_patterns = shapes.len(),
        rejected_patterns = rejected,
        rejection_reasons = ?rejection_reasons,
        matched_trips = trips.values().filter(|trip| trip.shape_id.is_some()).count(),
        unmatched_trips = trips.values().filter(|trip| trip.shape_id.is_none()).count(),
        "NJT pattern matching complete"
    );
    PatternDataset { shapes, trips }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (gtfs_structures::Gtfs, Vec<PatternFeature>) {
        // TODO: why are these imported in 2 different ways
        let gtfs = gtfs_structures::Gtfs::from_path(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/njt_bus/static/basic/raw/patterns_gtfs.zip"
        ))
        .unwrap();
        let features = serde_json::from_str(include_str!(
            "../../../tests/fixtures/njt_bus/static/basic/raw/operating_patterns.json"
        ))
        .unwrap();
        (gtfs, features)
    }
    #[test]
    fn exact_patterns_survive_import_and_match_all_fixture_trips() {
        let (gtfs, features) = fixture();
        let data = build_patterns(&gtfs, features);
        assert_eq!(data.shapes.len(), 3);
        assert_eq!(data.trips.len(), gtfs.trips.len());
        for (id, pattern) in data.trips {
            assert_eq!(pattern.shape_id, gtfs.trips[&id].shape_id);
            assert_eq!(pattern.route_id, gtfs.trips[&id].route_id);
        }
    }
    #[test]
    fn missing_duplicate_and_wrong_direction_patterns_are_rejected() {
        let (gtfs, features) = fixture();
        let missing = build_patterns(&gtfs, vec![]);
        assert!(missing.shapes.is_empty());
        assert_eq!(missing.trips.len(), gtfs.trips.len());
        assert!(missing.trips.values().all(|trip| trip.shape_id.is_none()));
        let mut duplicate = features.clone();
        duplicate.push(features[0].clone());
        assert_eq!(build_patterns(&gtfs, duplicate).shapes.len(), 2);
        let mut mismatch = features;
        mismatch[0].attributes.route_dir = Some("invalid".into());
        assert_eq!(build_patterns(&gtfs, mismatch).shapes.len(), 2);
    }
    #[test]
    fn reversed_geometry_is_restored_to_gtfs_travel_order() {
        let (gtfs, mut features) = fixture();
        let expected = build_patterns(&gtfs, features.clone());
        for f in &mut features {
            for path in &mut f.geometry.as_mut().unwrap().paths {
                path.reverse();
            }
        }
        let actual = build_patterns(&gtfs, features);
        assert_eq!(actual.shapes.len(), 3);
        for (a, b) in actual.shapes.iter().zip(&expected.shapes) {
            assert_eq!(a.geom.0, b.geom.0);
        }
    }
    #[test]
    fn multipart_requires_connected_unbranched_endpoints() {
        let a = [-74.0, 40.0];
        let b = [-74.01, 40.01];
        let c = [-74.02, 40.02];
        let line = connected_line(&PatternGeometry {
            paths: vec![vec![a, a, b], vec![c, b]],
        })
        .unwrap();
        assert_eq!(line.0, vec![Coord::from(a), Coord::from(b), Coord::from(c)]);
        assert!(
            connected_line(&PatternGeometry {
                paths: vec![vec![a, b], vec![c, [-75.0, 41.0]]]
            })
            .is_err()
        );
        assert!(
            connected_line(&PatternGeometry {
                paths: vec![vec![a, b], vec![b, c], vec![b, [-75.0, 41.0]]]
            })
            .is_err()
        );
        assert!(
            connected_line(&PatternGeometry {
                paths: vec![vec![a, a]]
            })
            .is_err()
        );
        assert!(
            connected_line(&PatternGeometry {
                paths: vec![vec![a, [f64::NAN, 40.0]]]
            })
            .is_err()
        );
    }
    #[test]
    fn pagination_rejects_errors_duplicates_and_nonprogress() {
        let (_, features) = fixture();
        let mut all = Vec::new();
        assert!(
            append_page(
                &mut all,
                PatternPage {
                    features: features[..1].to_vec(),
                    exceeded_transfer_limit: true,
                    error: None
                }
            )
            .unwrap()
        );
        assert!(
            !append_page(
                &mut all,
                PatternPage {
                    features: features[1..].to_vec(),
                    exceeded_transfer_limit: false,
                    error: None
                }
            )
            .unwrap()
        );
        assert!(
            append_page(
                &mut all,
                PatternPage {
                    features: vec![],
                    exceeded_transfer_limit: true,
                    error: None
                }
            )
            .is_err()
        );
        assert!(
            append_page(
                &mut all,
                PatternPage {
                    features: vec![],
                    exceeded_transfer_limit: false,
                    error: Some(serde_json::json!({"code":500}))
                }
            )
            .is_err()
        );
        assert!(
            append_page(
                &mut all,
                PatternPage {
                    features,
                    exceeded_transfer_limit: false,
                    error: None
                }
            )
            .is_err()
        );
    }
}
