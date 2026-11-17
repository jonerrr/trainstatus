use crate::support::{eastern_service_date, fixed_time, njt_bus_dataset};
use backend::{
    integrations::gtfs_realtime::remap_realtime_stop_ids,
    network::index::{NetworkIndex, NetworkRevision},
    operations::identity::RunId,
    operations::types::{
        position::{NjtBusPositionData, OccupancyStatus, PositionData, VehicleObservation},
        trip::{NjtBusData, StopTimeData, StopVisit, TripData, TripRun},
    },
    source::Source,
};

fn trip() -> TripRun {
    let now = fixed_time();
    let id = RunId::arbitrary("fixture-trip");
    TripRun {
        id,
        original_id: "fixture_trip".into(),
        vehicle_id: "fixture_vehicle".into(),
        route_id: "1".into(),
        shape_ids: vec![],
        direction: 0,
        created_at: now,
        updated_at: now,
        partition_date: eastern_service_date(now),
        data: TripData::NjtBus(NjtBusData {
            deviation: None,
            headsign: "Fixture".to_string(),
        }),
    }
}

fn stop_time(stop_id: &str, trip_id: RunId) -> StopVisit {
    let now = fixed_time();
    StopVisit {
        trip_id,
        stop_sequence: 1,
        stop_id: stop_id.into(),
        arrival: now,
        departure: now,
        data: StopTimeData::NjtBus,
    }
}

fn vehicle(stop_id: Option<&str>) -> VehicleObservation {
    VehicleObservation {
        vehicle_id: "fixture_vehicle".into(),
        trip_id: None,
        stop_id: stop_id.map(Into::into),
        updated_at: fixed_time(),
        data: PositionData::NjtBus(NjtBusPositionData {
            occupancy_status: OccupancyStatus::default(),
        }),
        geom: None,
    }
}

#[test]
fn published_revision_contains_trip_patterns_and_stop_remap() {
    let dataset = njt_bus_dataset();
    let index = NetworkIndex::new();
    index.publish_bundle(vec![NetworkRevision::from_dataset(&dataset)]);

    let revision = index.get(Source::NjtBus).expect("published revision");
    assert_eq!(
        revision.trip_patterns["1"].shape_id.as_deref(),
        Some("87-3")
    );
    assert_eq!(revision.stop_remap["16758"], "6509");
}

/// A published remap rewrites collapsed child gate ids to their canonical stop,
/// leaves unmapped ids untouched, and covers both stop_times and positions.
#[test]
fn remaps_child_stop_ids_to_canonical() {
    let revision = NetworkRevision::from_dataset(&njt_bus_dataset());

    // 16758 is a gate child collapsed into representative 6509; 500 is a plain
    // 1:1 stop with no remap entry.
    let trip = trip();
    let mut data = vec![(
        trip.clone(),
        vec![stop_time("16758", trip.id), stop_time("500", trip.id)],
    )];
    let mut positions = vec![vehicle(Some("16758")), vehicle(Some("500")), vehicle(None)];

    remap_realtime_stop_ids(&revision, &mut data, &mut positions);

    let stop_times = &data[0].1;
    assert_eq!(
        stop_times[0].stop_id, "6509",
        "gate child -> representative"
    );
    assert_eq!(stop_times[1].stop_id, "500", "unmapped id passes through");

    assert_eq!(positions[0].stop_id.as_deref(), Some("6509"));
    assert_eq!(positions[1].stop_id.as_deref(), Some("500"));
    assert_eq!(positions[2].stop_id, None, "None stop_id stays None");
    let identity = NetworkRevision::from_dataset(&backend::network::dataset::NetworkDataset::new(
        Source::MtaBus,
    ));
    let before = data[0].1[0].stop_id.clone();
    remap_realtime_stop_ids(&identity, &mut data, &mut positions);
    assert_eq!(data[0].1[0].stop_id, before);
    assert_eq!(positions[0].stop_id.as_deref(), Some("6509"));
}
