use backend::{
    feed::vehicle_position::OccupancyStatus,
    integrations::gtfs_realtime::remap_realtime_stop_ids,
    models::{
        position::{NjtBusPositionData, PositionData, VehiclePosition},
        source::Source,
        trip::{NjtBusData, StopTime, StopTimeData, Trip, TripData},
    },
    static_index::{StaticTransitIndex, StaticTransitRevision},
};
use chrono::Utc;
use uuid::Uuid;

use crate::common::{mta_bus_dataset, njt_bus_dataset};

fn trip() -> Trip {
    let now = Utc::now();
    Trip {
        id: Uuid::now_v7(),
        original_id: "fixture_trip".to_string(),
        vehicle_id: "fixture_vehicle".to_string(),
        route_id: "1".to_string(),
        shape_ids: vec![],
        direction: 0,
        created_at: now,
        updated_at: now,
        data: TripData::NjtBus(NjtBusData {
            deviation: None,
            headsign: "Fixture".to_string(),
        }),
    }
}

fn stop_time(stop_id: &str) -> StopTime {
    let now = Utc::now();
    StopTime {
        trip_id: Uuid::now_v7(),
        stop_id: stop_id.to_string(),
        arrival: now,
        departure: now,
        data: StopTimeData::NjtBus,
    }
}

fn vehicle(stop_id: Option<&str>) -> VehiclePosition {
    VehiclePosition {
        vehicle_id: "fixture_vehicle".to_string(),
        trip_id: None,
        stop_id: stop_id.map(str::to_string),
        updated_at: Utc::now(),
        data: PositionData::NjtBus(NjtBusPositionData {
            occupancy_status: OccupancyStatus::default(),
        }),
        geom: None,
    }
}

#[test]
fn published_revision_contains_trip_patterns_and_stop_remap() {
    let dataset = njt_bus_dataset();
    let index = StaticTransitIndex::new();
    index.publish(StaticTransitRevision::from_dataset(&dataset));

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
    let revision = StaticTransitRevision::from_dataset(&njt_bus_dataset());

    // 16758 is a gate child collapsed into representative 6509; 500 is a plain
    // 1:1 stop with no remap entry.
    let mut data = vec![(trip(), vec![stop_time("16758"), stop_time("500")])];
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
}

/// A revision without a remap leaves every stop ID unchanged.
#[test]
fn source_without_remap_is_identity() {
    let revision = StaticTransitRevision::from_dataset(&mta_bus_dataset());

    let mut data = vec![(trip(), vec![stop_time("16957")])];
    let mut positions = vec![vehicle(Some("16957"))];

    remap_realtime_stop_ids(&revision, &mut data, &mut positions);

    assert_eq!(
        data[0].1[0].stop_id, "16957",
        "no remap for source -> identity"
    );
    assert_eq!(positions[0].stop_id.as_deref(), Some("16957"));
}
