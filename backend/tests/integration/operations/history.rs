use crate::support::{TestContext, fixtures::fixed_time, mta_bus_dataset, test_owners};
use backend::{
    geometry::Geom,
    operations::{
        identity::RunId,
        source::CollectedSnapshot,
        types::{
            position::{MtaBusPositionData, PositionData, VehicleObservation},
            trip::{MtaBusData, StopTimeData, StopVisit, TripData, TripRun},
        },
    },
    source::Source,
    trajectory::service::TrajectoryService,
};

const B94_STOP_IDS: [&str; 5] = ["904218", "904979", "504409", "901701", "904219"];

fn collected_trip(route_id: &str, stop_ids: &[&str]) -> CollectedSnapshot {
    let id = RunId::arbitrary("history-b94");
    let now = fixed_time();
    let trip = TripRun {
        id,
        original_id: "history-b94".into(),
        vehicle_id: "history-bus".into(),
        route_id: route_id.into(),
        shape_ids: vec![],
        direction: 0,
        created_at: now,
        updated_at: now,
        partition_date: crate::support::fixtures::eastern_service_date(now),
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
    CollectedSnapshot {
        source: Source::MtaBus,
        network_revision: None,
        renormalize: None,
        trips: vec![(trip, stops)],
        positions: vec![],
    }
}

#[tokio::test]
async fn concurrent_historical_requests_share_snapshot() {
    let ctx = TestContext::new();
    let storage = ctx.storage();
    let stores = test_owners(storage);
    stores.network.persist(mta_bus_dataset()).await.unwrap();
    stores
        .ingestor
        .ingest(collected_trip("B94", &B94_STOP_IDS))
        .await
        .unwrap();
    let service = TrajectoryService::new(stores.ingestor.history());
    let at = fixed_time();
    let results =
        futures::future::join_all((0..8).map(|_| service.snapshot(Source::MtaBus, Some(at)))).await;
    let snapshots: Vec<_> = results.into_iter().map(Result::unwrap).collect();
    for snapshot in &snapshots[1..] {
        assert!(
            std::sync::Arc::ptr_eq(&snapshots[0], snapshot),
            "concurrent misses must return the same computed snapshot"
        );
    }
}

#[tokio::test]
async fn nearby_historical_times_keep_their_own_samples_in_both_request_orders() {
    let ctx = TestContext::new();
    let storage = ctx.storage();
    let stores = test_owners(storage);
    stores.network.persist(mta_bus_dataset()).await.unwrap();
    let at = fixed_time();
    stores
        .ingestor
        .ingest(collected_trip("B94", &B94_STOP_IDS))
        .await
        .unwrap();
    let later = at + chrono::Duration::seconds(1);
    let service = TrajectoryService::new(stores.ingestor.history());
    let first = service.snapshot(Source::MtaBus, Some(at)).await.unwrap();
    let second = service.snapshot(Source::MtaBus, Some(later)).await.unwrap();
    assert!(!first.render_units.is_empty());
    assert!(!second.render_units.is_empty());
    assert_ne!(first.render_units, second.render_units);
}

#[tokio::test]
async fn historical_inputs_use_final_departure_and_past_history_points() {
    let ctx = TestContext::new();
    let storage = ctx.storage();
    let stores = test_owners(storage);
    stores.network.persist(mta_bus_dataset()).await.unwrap();
    let at = fixed_time() - chrono::Duration::hours(1);
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
        trip_id: Some((&input.trips[0].0).into()),
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
    let historical = stores
        .ingestor
        .history()
        .load_trajectories(Source::MtaBus, at)
        .await
        .unwrap();
    assert_eq!(historical.service.trips.len(), 1);
    assert_eq!(historical.service.trips[0].id, committed.trips[0].id);
    let anchor = historical.anchors.get(&committed.trips[0].id).unwrap();
    assert_eq!(anchor.observed_at, committed.positions[0].updated_at);
    assert_eq!(anchor.point, old_point);
    assert!(!historical.service.stop_times.is_empty());
}
