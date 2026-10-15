use backend::fixtures;
use backend::fixtures::FixtureKind;
use backend::network::index::NetworkRevision;
use backend::network::schedule::ScheduledTrip;
use backend::operations::CollectedSnapshot;
use backend::source::Source;
use backend::sources::njt_bus::realtime::NjtBusRealtime;
use chrono::TimeZone;

use crate::support::fixture_root;

#[tokio::test]
async fn realtime_infers_missing_service_date_from_schedule() {
    let root = fixture_root();
    let manifest = fixtures::load_manifest_for(
        &root,
        Source::NjtBus,
        FixtureKind::Realtime,
        "missing_service_date",
    )
    .expect("NJT bus realtime manifest should load");
    let fixture = fixtures::read_gtfs_realtime_payload(&root, &manifest, "trip_updates")
        .expect("NJT bus realtime fixture should decode");
    assert!(
        fixture
            .entity
            .iter()
            .filter_map(|entity| entity.trip_update.as_ref())
            .all(|update| update.trip.start_date.is_none())
    );
    let vehicles = fixtures::read_gtfs_realtime_payload(&root, &manifest, "vehicle_positions")
        .expect("NJT positions fixture should decode");
    // Keep the provider payload unchanged: NJT omits start_date. Pin real
    // scheduled times to the capture's explicit New York service day.
    let gtfs = gtfs_structures::GtfsReader::default()
        .read_shapes(false)
        .raw()
        .read_from_reader(
            std::fs::File::open(manifest.fixture_dir(&root).join("raw/schedule_gtfs.zip")).unwrap(),
        )
        .and_then(gtfs_structures::Gtfs::try_from)
        .unwrap();
    let service_date = chrono::NaiveDate::from_ymd_opt(2026, 10, 13).unwrap();
    let midnight = chrono_tz::America::New_York
        .from_local_datetime(&service_date.and_hms_opt(0, 0, 0).unwrap())
        .single()
        .unwrap()
        .with_timezone(&chrono::Utc);
    let scheduled_trips = fixture
        .entity
        .iter()
        .filter_map(|entity| {
            let update = entity.trip_update.as_ref()?;
            let trip_id = update.trip.trip_id.as_ref()?;
            let trip = gtfs.trips.get(trip_id)?;
            let start_seconds = trip.stop_times.first()?.arrival_time?;
            Some(ScheduledTrip {
                trip_id: trip_id.clone(),
                route_id: trip.route_id.clone(),
                direction_id: trip.direction_id.map(|d| d as i16).unwrap_or(0),
                headsign: trip.trip_headsign.clone().unwrap_or_default(),
                start_date: "20261013".into(),
                start_time: midnight + chrono::Duration::seconds(i64::from(start_seconds)),
                stop_times: trip
                    .stop_times
                    .iter()
                    .filter_map(|stop| {
                        Some(backend::network::schedule::ScheduledStopTime {
                            stop_id: stop.stop.id.clone(),
                            stop_sequence: stop.stop_sequence as u16,
                            arrival: midnight
                                + chrono::Duration::seconds(i64::from(stop.arrival_time?)),
                            departure: midnight
                                + chrono::Duration::seconds(i64::from(stop.departure_time?)),
                        })
                    })
                    .collect(),
            })
        })
        .collect::<Vec<_>>();

    let mut dataset = backend::network::dataset::NetworkDataset::new(Source::NjtBus);
    dataset.scheduled_trips = scheduled_trips;
    drop(gtfs);
    let snapshot: CollectedSnapshot = NjtBusRealtime::build_snapshot(
        vec![fixture, vehicles],
        std::sync::Arc::new(NetworkRevision::from_dataset(&dataset)),
    )
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
    assert_eq!(trip.original_id, "27119247");
    assert_eq!(trip.vehicle_id, "17113");
    assert_eq!(trip.route_id, "553");
    assert_eq!(trip.direction, 1);
    let scheduled = dataset
        .scheduled_trips
        .iter()
        .find(|scheduled| scheduled.trip_id == trip.original_id)
        .unwrap();
    assert_eq!(trip.created_at, scheduled.start_time);
    assert_eq!(trip.route_id, scheduled.route_id);
    assert_eq!(trip.direction, scheduled.direction_id);
    let prediction = &times[0];
    assert_eq!(prediction.stop_id, "31717");
    assert_eq!(prediction.arrival.timestamp(), 1_791_896_115);
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
