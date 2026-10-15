use crate::support;
use backend::geometry::Geom;
use backend::operations::CollectedObservation as VehicleObservation;
use backend::operations::CollectedObservation;
use backend::operations::CollectedTrip as TripRun;
use backend::operations::CollectedVisit as StopVisit;
use backend::operations::history::History;
use backend::operations::types::position::MtaBusPositionData;
use backend::operations::types::position::PositionData;
use backend::operations::types::trip::MtaBusData;
use backend::operations::types::trip::StopTimeData;
use backend::operations::types::trip::TripData;

use backend::source::Source;
use support::{mta_bus_dataset, test_owners};
use uuid::Uuid;

/// Real B94 stops (from the checked-in Helium fixture) all served by shape
/// `B940017`. Used to build a synthetic realtime trip whose stop sequence
/// unambiguously belongs to that shape.
const B94_STOP_IDS: [&str; 5] = ["504409", "901701", "904218", "904219", "904979"];

#[sqlx::test]
async fn concurrent_historical_requests_share_snapshot(pool: sqlx::PgPool) {
    let service = backend::trajectory::TrajectoryService::new(pool);
    let at = crate::support::fixtures::fixed_time();
    let results =
        futures::future::join_all((0..8).map(|_| service.snapshot(Source::MtaBus, Some(at)))).await;
    let snapshots: Vec<_> = results.into_iter().map(Result::unwrap).collect();
    for snapshot in &snapshots[1..] {
        assert!(
            std::sync::Arc::ptr_eq(&snapshots[0], snapshot),
            "concurrent misses must return the same computed snapshot"
        );
    }
    let cached = service.snapshot(Source::MtaBus, Some(at)).await.unwrap();
    assert!(std::sync::Arc::ptr_eq(&snapshots[0], &cached));
    let next_second = service
        .snapshot(Source::MtaBus, Some(at + chrono::Duration::seconds(1)))
        .await
        .unwrap();
    assert!(!std::sync::Arc::ptr_eq(&snapshots[0], &next_second));
}

#[sqlx::test]
async fn nearby_historical_times_keep_their_own_samples_in_both_request_orders(pool: sqlx::PgPool) {
    let stores = test_owners(pool.clone());
    stores.network.persist(&mta_bus_dataset()).await.unwrap();
    let at = crate::support::fixtures::fixed_time();
    stores
        .ingestor
        .ingest(collected_trip("B94", &B94_STOP_IDS))
        .await
        .unwrap();
    let later = at + chrono::Duration::seconds(1);
    assert_eq!(
        at.timestamp().div_euclid(300),
        later.timestamp().div_euclid(300)
    );
    let first = backend::trajectory::TrajectoryService::new(pool.clone())
        .snapshot(Source::MtaBus, Some(at))
        .await
        .unwrap();
    let second = backend::trajectory::TrajectoryService::new(pool.clone())
        .snapshot(Source::MtaBus, Some(later))
        .await
        .unwrap();
    assert!(!first.render_units.is_empty());
    assert!(!second.render_units.is_empty());
    let expected_first = serde_json::to_value(&first.render_units).unwrap();
    let expected_second = serde_json::to_value(&second.render_units).unwrap();
    assert_ne!(
        expected_first, expected_second,
        "sampling windows depend on the requested second"
    );
    for order in [[at, later], [later, at]] {
        let service = backend::trajectory::TrajectoryService::new(pool.clone());
        for time in order {
            let snapshot = service.snapshot(Source::MtaBus, Some(time)).await.unwrap();
            let expected = if time == at {
                &expected_first
            } else {
                &expected_second
            };
            assert_eq!(
                &serde_json::to_value(&snapshot.render_units).unwrap(),
                expected
            );
        }
    }
}

#[sqlx::test]
async fn failed_historical_load_can_be_retried(pool: sqlx::PgPool) {
    let service = backend::trajectory::TrajectoryService::new(pool.clone());
    let at = crate::support::fixtures::fixed_time();
    sqlx::query("ALTER TABLE realtime.trip RENAME TO unavailable_trip")
        .execute(&pool)
        .await
        .unwrap();
    assert!(service.snapshot(Source::MtaBus, Some(at)).await.is_err());
    sqlx::query("ALTER TABLE realtime.unavailable_trip RENAME TO trip")
        .execute(&pool)
        .await
        .unwrap();
    service.snapshot(Source::MtaBus, Some(at)).await.unwrap();
}

fn collected_trip(route_id: &str, stop_ids: &[&str]) -> backend::operations::CollectedSnapshot {
    let id: backend::operations::RunKey = Uuid::now_v7().into();
    let now = crate::support::fixtures::fixed_time();
    let trip = TripRun {
        id,
        original_id: format!("test-trip-{}", Uuid::now_v7()),
        vehicle_id: format!("test-vehicle-{}", Uuid::now_v7()),
        route_id: route_id.into(),
        shape_ids: vec![],
        direction: 0,
        created_at: now,
        updated_at: now,
        data: TripData::MtaBus(MtaBusData { deviation: None }),
    };
    let stops = stop_ids
        .iter()
        .enumerate()
        .map(|(i, stop)| {
            let arrival = now + chrono::Duration::minutes(i as i64 * 2);
            StopVisit {
                trip_id: id,
                stop_sequence: i as i32,
                stop_id: (*stop).into(),
                arrival,
                departure: arrival,
                data: StopTimeData::MtaBus,
            }
        })
        .collect();
    backend::operations::CollectedSnapshot {
        source: Source::MtaBus,
        network_revision: None,
        renormalize: None,
        trips: vec![(trip, stops)],
        positions: vec![],
    }
}

#[sqlx::test]
async fn historical_inputs_use_final_departure_and_past_history_points(pool: sqlx::PgPool) {
    let stores = test_owners(pool.clone());
    let dataset = mta_bus_dataset();
    stores.network.persist(&dataset).await.unwrap();
    let at = crate::support::fixtures::fixed_time() - chrono::Duration::hours(1);
    let mut input = collected_trip("B94", &B94_STOP_IDS);
    input.trips[0].0.created_at = at - chrono::Duration::minutes(5);
    input.trips[0].0.updated_at = at + chrono::Duration::hours(2);
    for stop in &mut input.trips[0].1 {
        stop.arrival = at - chrono::Duration::minutes(2);
        stop.departure = at + chrono::Duration::minutes(1);
    }
    let old_point = geo::Point::new(-73.948151, 40.744723);
    let observed_at = at - chrono::Duration::seconds(20);
    input.positions.push(VehicleObservation {
        vehicle_id: input.trips[0].0.vehicle_id.clone(),
        trip_id: Some(input.trips[0].0.id),
        stop_id: None,
        geom: Some(Geom::from(old_point)),
        updated_at: observed_at,
        data: PositionData::MtaBus(MtaBusPositionData {
            bearing: 0.0,
            passengers: None,
            capacity: None,
            status: None,
            phase: None,
        }),
    });
    let committed = stores.ingestor.ingest(input.clone()).await.unwrap();
    input.positions[0].updated_at = at + chrono::Duration::seconds(20);
    input.positions[0].geom = Some(Geom::from(geo::Point::new(-73.94, 40.75)));
    stores.ingestor.ingest(input).await.unwrap();
    let historical = History::new(pool.clone())
        .load_trajectories(Source::MtaBus, at)
        .await
        .unwrap();
    assert_eq!(
        historical.service.trips.len(),
        1,
        "final departure keeps retained trip relevant"
    );
    assert_eq!(historical.service.trips[0].id, committed.trips[0].id);
    let anchor = &historical.anchors[&committed.trips[0].id];
    assert_eq!(anchor.observed_at, committed.positions[0].updated_at);
    assert_eq!(anchor.point, old_point);
    assert!(!historical.service.stop_times.is_empty());
}

#[sqlx::test]
async fn historical_rendering_uses_past_gps_coordinates_at_the_observation_time(
    pool: sqlx::PgPool,
) {
    let stores = test_owners(pool.clone());
    let dataset = mta_bus_dataset();
    stores.network.persist(&dataset).await.unwrap();
    let at = crate::support::fixtures::fixed_time();
    let mut input = collected_trip("B94", &B94_STOP_IDS);
    let shape = &dataset
        .shapes
        .iter()
        .find(|shape| shape.id == "B940017")
        .unwrap()
        .geom
        .0;
    let geo::Geometry::LineString(shape) = shape else {
        panic!("fixture must contain a line");
    };
    let old_point = geo::Point::from(shape.0[0]);
    let observed_at = at - chrono::Duration::seconds(20);
    input.positions.push(CollectedObservation {
        vehicle_id: input.trips[0].0.vehicle_id.clone(),
        trip_id: Some(input.trips[0].0.id),
        stop_id: None,
        geom: Some(Geom::from(old_point)),
        updated_at: observed_at,
        data: PositionData::MtaBus(MtaBusPositionData {
            bearing: 0.0,
            passengers: None,
            capacity: None,
            status: None,
            phase: None,
        }),
    });
    let committed = stores.ingestor.ingest(input.clone()).await.unwrap();
    input.positions[0].updated_at = at + chrono::Duration::seconds(20);
    input.positions[0].geom = Some(Geom::from(geo::Point::from(*shape.0.last().unwrap())));
    stores.ingestor.ingest(input).await.unwrap();
    let rendered = backend::trajectory::TrajectoryService::new(pool)
        .snapshot(Source::MtaBus, Some(at))
        .await
        .unwrap();
    let unit = rendered
        .render_units
        .values()
        .find(|unit| unit.trip_id == committed.trips[0].id.to_string())
        .expect("historical bus renders");
    let sample = unit
        .timestamps
        .iter()
        .position(|t| (*t - observed_at.timestamp() as f64).abs() < 0.001)
        .expect("rendered path samples the historical GPS observation time");
    let [lon, lat] = unit.positions[sample];
    assert!(
        (lon - old_point.x()).abs() < 0.00001 && (lat - old_point.y()).abs() < 0.00001,
        "rendered position must use the past GPS fix, not schedule-only interpolation or the future fix: {lon}, {lat}"
    );
}
