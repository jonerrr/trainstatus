use backend::{
    fixtures::{self, FixtureKind},
    models::source::Source,
    realtime::CollectedSnapshot,
    sources::njt_bus::realtime::NjtBusRealtime,
    static_data::index::StaticTransitIndex,
    static_data::index::StaticTransitRevision,
    static_data::schedule::ScheduledTrip,
};

use crate::support::{fixture_root, njt_bus_dataset};

#[tokio::test]
async fn realtime_fixture_maps_trips() {
    let root = fixture_root();
    let manifest =
        fixtures::load_manifest_for(&root, Source::NjtBus, FixtureKind::Realtime, "basic")
            .expect("NJT bus realtime manifest should load");
    let mut fixture = fixtures::read_gtfs_realtime_payload(&root, &manifest, "trip_updates")
        .expect("NJT bus realtime fixture should decode");
    let vehicles = fixtures::read_gtfs_realtime_payload(&root, &manifest, "vehicle_positions")
        .expect("NJT positions fixture should decode");
    // The capture is fixed; give missing dates a fixed service day rather than today.
    for entity in &mut fixture.entity {
        if let Some(update) = &mut entity.trip_update {
            update
                .trip
                .start_date
                .get_or_insert_with(|| "20260528".into());
        }
    }
    let scheduled_trips = fixture
        .entity
        .iter()
        .filter_map(|entity| {
            let update = entity.trip_update.as_ref()?;
            let trip_id = update.trip.trip_id.clone()?;
            Some(ScheduledTrip {
                trip_id,
                route_id: update
                    .trip
                    .route_id
                    .clone()
                    .unwrap_or_else(|| "fixture_route".to_string()),
                direction_id: update.trip.direction_id.map(|d| d as i16).unwrap_or(0),
                headsign: "Fixture Headsign".to_string(),
                start_date: update.trip.start_date.clone().unwrap(),
                start_time: crate::support::fixtures::fixed_time(),
                stop_times: vec![],
            })
        })
        .collect::<Vec<_>>();

    let mut dataset = njt_bus_dataset();
    dataset.scheduled_trips = scheduled_trips;
    let index = StaticTransitIndex::new();
    index.publish(StaticTransitRevision::from_dataset(&dataset));
    let adapter = NjtBusRealtime::new(index);

    let snapshot: CollectedSnapshot = adapter
        .build_snapshot(vec![fixture, vehicles])
        .await
        .expect("NJT snapshot should normalize");
    assert_eq!(snapshot.source, Source::NjtBus);
    let (trip, times) = snapshot
        .trips
        .iter()
        .find(|(trip, times)| {
            !times.is_empty()
                && snapshot.positions.iter().any(|position| {
                    position.trip_id == Some(trip.id) && position.vehicle_id == trip.vehicle_id
                })
        })
        .expect("fixture should contain a trip with predictions and a position");
    assert!(!trip.vehicle_id.is_empty());
    let prediction = &times[0];
    assert!(!prediction.stop_id.is_empty());
    assert!(prediction.arrival <= prediction.departure);
    let position = snapshot
        .positions
        .iter()
        .find(|position| position.trip_id == Some(trip.id))
        .expect("captured bus position");
    assert_eq!(position.trip_id, Some(trip.id));
    let geo::Geometry::Point(point) = position.geom.as_ref().unwrap().0 else {
        panic!("bus point")
    };
    assert!(point.x().is_finite());
    assert!(point.y().is_finite());
    assert!(
        !snapshot.trips.is_empty(),
        "fixture should include processable NJT trips"
    );
    assert!(
        !snapshot.positions.is_empty(),
        "fixture should include NJT positions"
    );
    for position in &snapshot.positions {
        if let Some(trip_id) = position.trip_id {
            assert!(
                snapshot
                    .trips
                    .iter()
                    .any(|(trip, _)| trip.id == trip_id && trip.vehicle_id == position.vehicle_id)
            );
        }
    }
    for (trip, stop_times) in &snapshot.trips {
        assert!(!trip.original_id.is_empty());
        for stop_time in stop_times {
            assert_eq!(stop_time.trip_id, trip.id);
        }
    }
}
