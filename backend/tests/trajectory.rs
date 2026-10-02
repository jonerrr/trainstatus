mod common;

use backend::models::source::Source;
use chrono::Utc;
use common::{mta_bus_dataset, setup_redis, test_stores};
use uuid::Uuid;

/// Real B94 stops (from the checked-in Helium fixture) all served by shape
/// `B940017`. Used to build a synthetic realtime trip whose stop sequence
/// unambiguously belongs to that shape.
const B94_STOP_IDS: [&str; 5] = ["504409", "901701", "904218", "904219", "904979"];
const B94_STOP_COORDS: [(f64, f64); 5] = [
    (-73.948151, 40.744723),
    (-73.945602, 40.745906),
    (-73.950969, 40.724258),
    (-73.944334, 40.74747),
    (-73.954116, 40.730018),
];
const B94_REAL_SHAPE: &str = "B940017";
const DECOY_SHAPE: &str = "TEST_DECOY_SHAPE";

fn collected_trip(route_id: &str, stop_ids: &[&str]) -> backend::realtime::CollectedSnapshot {
    use backend::models::trip::{MtaBusData, StopTime, StopTimeData, Trip, TripData};
    let id = Uuid::now_v7();
    let now = Utc::now();
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
async fn resolves_shape_by_stop_hits_over_raw_distance(pool: sqlx::PgPool) {
    let redis_pool = setup_redis().await;
    let stores = test_stores(pool.clone(), redis_pool);

    let dataset = mta_bus_dataset();
    dataset
        .persist(
            &stores.route_store,
            &stores.stop_store,
            &stores.static_cache_store,
        )
        .await
        .expect("fixture should persist");

    // A decoy shape threaded exactly through the trip's real stops: ST_Distance
    // to every one of them is ~0m, versus ~5.2m for the real B940017 shape.
    let decoy_points: Vec<String> = B94_STOP_COORDS
        .iter()
        .map(|(lon, lat)| format!("ST_MakePoint({lon}, {lat})"))
        .collect();
    sqlx::query(&format!(
        "INSERT INTO static.shape (id, source, geom, data)
         VALUES ('{DECOY_SHAPE}', 'mta_bus', ST_SetSRID(ST_MakeLine(ARRAY[{}]), 4326), '{{}}'::jsonb)",
        decoy_points.join(", ")
    ))
    .execute(&pool)
    .await
    .expect("insert decoy shape");

    sqlx::query(
        "UPDATE static.route
         SET data = jsonb_set(data, '{shape_ids}', (data->'shape_ids') || to_jsonb($1::text))
         WHERE id = 'B94' AND source = 'mta_bus'",
    )
    .bind(DECOY_SHAPE)
    .execute(&pool)
    .await
    .expect("append decoy shape to route candidates");

    // Sanity check the setup actually creates the adversarial condition this
    // test is meant to catch a regression of.
    let (decoy_avg, real_avg): (Option<f64>, Option<f64>) = sqlx::query_as(
        "SELECT
            (SELECT AVG(ST_Distance(sh.geom::geography, s.geom::geography))
             FROM static.shape sh, static.stop s
             WHERE sh.id = $2 AND sh.source = 'mta_bus'
               AND s.id = ANY($1) AND s.source = 'mta_bus'),
            (SELECT AVG(ST_Distance(sh.geom::geography, s.geom::geography))
             FROM static.shape sh, static.stop s
             WHERE sh.id = $3 AND sh.source = 'mta_bus'
               AND s.id = ANY($1) AND s.source = 'mta_bus')",
    )
    .bind(&B94_STOP_IDS[..])
    .bind(DECOY_SHAPE)
    .bind(B94_REAL_SHAPE)
    .fetch_one(&pool)
    .await
    .expect("distance sanity check query");
    assert!(
        decoy_avg.unwrap() < real_avg.unwrap(),
        "test setup should make the decoy shape the closer one by raw distance"
    );

    let mut revision = backend::static_index::StaticTransitRevision::from_dataset(&dataset);
    revision.shapes.insert(
        DECOY_SHAPE.into(),
        backend::models::geom::Geom::from(geo::LineString::from(B94_STOP_COORDS.to_vec())),
    );
    revision
        .routes
        .get_mut("B94")
        .unwrap()
        .shape_ids
        .push(DECOY_SHAPE.into());
    stores.static_cache_store.static_index().publish(revision);
    let committed = stores
        .ingestor
        .ingest(collected_trip("B94", &B94_STOP_IDS))
        .await
        .unwrap();
    assert_eq!(
        committed.trips[0].shape_ids,
        [B94_REAL_SHAPE],
        "membership must beat the geometrically closer decoy"
    );
    assert_eq!(committed.changes.resolved_shapes, 1);
}

/// Before a fresh static import populates `route_stop.data->'shape_ids'`, every
/// candidate has zero hits. Resolution must still fall back to the old average-
/// distance ranking rather than erroring or dropping the trip.
#[sqlx::test]
async fn falls_back_to_distance_when_no_hit_data_exists(pool: sqlx::PgPool) {
    let redis_pool = setup_redis().await;
    let stores = test_stores(pool.clone(), redis_pool);

    let dataset = mta_bus_dataset();
    dataset
        .persist(
            &stores.route_store,
            &stores.stop_store,
            &stores.static_cache_store,
        )
        .await
        .expect("fixture should persist");

    // Simulate pre-migration route_stop rows: no shape_ids key at all.
    sqlx::query("UPDATE static.route_stop SET data = data - 'shape_ids' WHERE source = 'mta_bus'")
        .execute(&pool)
        .await
        .expect("strip shape_ids from route_stop");

    let mut revision = backend::static_index::StaticTransitRevision::from_dataset(&dataset);
    revision.route_stop_shapes.clear();
    stores.static_cache_store.static_index().publish(revision);
    let committed = stores
        .ingestor
        .ingest(collected_trip("B94", &B94_STOP_IDS))
        .await
        .unwrap();
    assert!(
        !committed.trips[0].shape_ids.is_empty(),
        "zero membership hits still resolve a shape through distance"
    );
    assert_eq!(committed.changes.resolved_shapes, 1);
}

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
    let redis = setup_redis().await;
    let stores = test_stores(pool.clone(), redis);
    mta_bus_dataset()
        .persist(
            &stores.route_store,
            &stores.stop_store,
            &stores.static_cache_store,
        )
        .await
        .unwrap();
    let at = Utc::now() - chrono::Duration::hours(1);
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
