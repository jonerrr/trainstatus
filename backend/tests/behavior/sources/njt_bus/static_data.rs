use crate::support::{contracts, njt_bus_dataset};

#[test]
fn static_fixture_preserves_exact_patterns_and_canonical_stops() {
    let dataset = njt_bus_dataset();
    contracts::assert_static_dataset_contract(&dataset);
    assert_eq!(dataset.shapes.len(), 3);
    assert_eq!(dataset.trip_patterns["1"].shape_id.as_deref(), Some("87-3"));
    assert_eq!(dataset.stop_remap["16758"], "6509");
    assert!(dataset.stops.iter().any(|stop| stop.id == "6509"));
    assert!(!dataset.stops.iter().any(|stop| stop.id == "16758"));
}
