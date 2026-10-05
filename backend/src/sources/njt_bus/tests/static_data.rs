use super::*;
use crate::models::stop::StopData;
use gtfs_structures::{Gtfs, LocationType, Stop as GtfsStop};
use std::sync::Arc;

fn make_stop(id: &str, code: &str, location_type: LocationType, parent: Option<&str>) -> GtfsStop {
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
    assert!(!remap.contains_key("16339"));
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
fn njt_static_geometry_is_not_discarded() {
    let gtfs = Gtfs::from_path(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/njt_bus/static/basic/raw/patterns_gtfs.zip"
    ))
    .unwrap();
    let (stops, remap) = collapse_stops(&gtfs);
    let features: Vec<PatternFeature> = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/njt_bus/static/basic/raw/operating_patterns.json"
    )))
    .unwrap();
    let dataset = build_static_dataset(
        &gtfs,
        patterns::build_patterns(&gtfs, features),
        vec![],
        stops,
        &remap,
    );
    assert!(
        !dataset.shapes.is_empty(),
        "NJT route geometry must reach static.shape"
    );
}

#[test]
fn stop_id_sort_key_orders_numerically() {
    assert!(stop_id_sort_key("2") < stop_id_sort_key("10"));
    // Non-numeric ids sort after numeric ones.
    assert!(stop_id_sort_key("999999") < stop_id_sort_key("A1"));
}

/// End-to-end against the captured NJT GTFS feed. Ignored by default because
/// it parses a ~55MB zip; run the named library test explicitly with `--ignored`.
#[test]
#[ignore = "large captured GTFS import; run the named library test explicitly"]
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
