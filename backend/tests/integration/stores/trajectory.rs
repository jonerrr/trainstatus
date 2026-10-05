use crate::support;

use backend::models::source::Source;
use support::{mta_bus_dataset, test_stores};
use uuid::Uuid;

/// Real B94 stops (from the checked-in Helium fixture) all served by shape
/// `B940017`. Used to build a synthetic realtime trip whose stop sequence
/// unambiguously belongs to that shape.
const B94_STOP_IDS: [&str; 5] = ["504409", "901701", "904218", "904219", "904979"];
fn collected_trip(route_id: &str, stop_ids: &[&str]) -> backend::realtime::CollectedSnapshot {
    use backend::models::trip::{MtaBusData, StopTime, StopTimeData, Trip, TripData};
    let id = Uuid::now_v7();
    let now = crate::support::fixtures::fixed_time();
    let trip = Trip {
        id,
        original_id: format!("test-trip-{id}"),
        vehicle_id: format!("test-vehicle-{id}"),
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
            StopTime {
                trip_id: id,
                stop_id: (*stop).into(),
                arrival,
                departure: arrival,
                data: StopTimeData::MtaBus,
            }
        })
        .collect();
    backend::realtime::CollectedSnapshot {
        source: Source::MtaBus,
        trips: vec![(trip, stops)],
        positions: vec![],
    }
}

/// A candidate shape can be geometrically closer to a trip's stops than the
/// real shape (e.g. a coincidence, or here — deliberately — a decoy built
/// exactly through the stop points) while having zero confirmed stop-membership
/// hits. Ingestion must prefer the hit-rich real shape over the
/// distance-closer decoy.
#[sqlx::test]
async fn historical_trajectory_uses_final_departure_and_past_history_points(pool: sqlx::PgPool) {
    use backend::{
        models::{
            geom::Geom,
            position::{MtaBusPositionData, PositionData, VehiclePosition},
        },
        stores::trajectory::TrajectoryStore,
        trajectory::TrajectoryCache,
    };
    use std::sync::Arc;
    let _redis = crate::support::TestRedis::start().await.unwrap();
    let redis = _redis.pool();
    let stores = test_stores(pool.clone(), redis);
    mta_bus_dataset()
        .persist(
            &stores.route_store,
            &stores.stop_store,
            &stores.static_cache_store,
        )
        .await
        .unwrap();
    let at = crate::support::fixtures::fixed_time() - chrono::Duration::hours(1);
    let mut input = collected_trip("B94", &B94_STOP_IDS);
    input.trips[0].0.created_at = at - chrono::Duration::minutes(5);
    input.trips[0].0.updated_at = at + chrono::Duration::hours(2);
    for stop in &mut input.trips[0].1 {
        stop.arrival = at - chrono::Duration::minutes(2);
        stop.departure = at + chrono::Duration::minutes(1);
    }
    let old_point = geo::Point::new(-73.948151, 40.744723);
    input.positions.push(VehiclePosition {
        vehicle_id: input.trips[0].0.vehicle_id.clone(),
        trip_id: Some(input.trips[0].0.id),
        stop_id: None,
        geom: Some(Geom::from(old_point)),
        updated_at: at - chrono::Duration::seconds(20),
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
    let historical = TrajectoryStore::new(pool.clone(), Arc::new(TrajectoryCache::new()))
        .load_historical_inputs(Source::MtaBus, at)
        .await
        .unwrap();
    assert_eq!(
        historical.len(),
        1,
        "final departure keeps retained trip relevant even with later updated_at"
    );
    assert_eq!(historical[0].trip_id, committed.trips[0].id);
    assert_eq!(historical[0].positions.len(), 1);
    assert_eq!(
        historical[0].positions[0].updated_at,
        committed.positions[0].updated_at
    );
    assert_eq!(
        historical[0].positions[0].geom.as_ref().unwrap().0,
        geo::Geometry::Point(old_point)
    );
    assert!(!historical[0].stops.is_empty());
}
