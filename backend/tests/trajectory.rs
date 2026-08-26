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

async fn insert_trip_and_stop_times(
    pool: &sqlx::PgPool,
    route_id: &str,
    stop_ids: &[&str],
) -> Uuid {
    let trip_id = Uuid::now_v7();
    let now = Utc::now();

    sqlx::query(
        r#"
        INSERT INTO realtime.trip
            (id, original_id, vehicle_id, route_id, shape_ids, source, direction, created_at, updated_at, data)
        VALUES ($1, $2, $3, $4, ARRAY[]::varchar[], 'mta_bus', 0, $5, $5, '{}'::jsonb)
        "#,
    )
    .bind(trip_id)
    .bind(format!("test-trip-{trip_id}"))
    .bind(format!("test-vehicle-{trip_id}"))
    .bind(route_id)
    .bind(now)
    .execute(pool)
    .await
    .expect("insert synthetic trip");

    for (i, stop_id) in stop_ids.iter().enumerate() {
        let arrival = now + chrono::Duration::minutes(i as i64 * 2);
        sqlx::query(
            r#"
            INSERT INTO realtime.stop_time (trip_id, stop_id, source, arrival, departure, data)
            VALUES ($1, $2, 'mta_bus', $3, $3, '{"source": "mta_bus"}'::jsonb)
            "#,
        )
        .bind(trip_id)
        .bind(*stop_id)
        .bind(arrival)
        .execute(pool)
        .await
        .expect("insert synthetic stop_time");
    }

    trip_id
}

/// A candidate shape can be geometrically closer to a trip's stops than the
/// real shape (e.g. a coincidence, or here — deliberately — a decoy built
/// exactly through the stop points) while having zero confirmed stop-membership
/// hits. `get_trajectory_inputs` must prefer the hit-rich real shape over the
/// distance-closer decoy.
#[sqlx::test]
async fn resolves_shape_by_stop_hits_over_raw_distance(pool: sqlx::PgPool) {
    let redis_pool = setup_redis().await;
    let stores = test_stores(pool.clone(), redis_pool);

    mta_bus_dataset()
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

    let trip_id = insert_trip_and_stop_times(&pool, "B94", &B94_STOP_IDS).await;

    let rows = stores
        .trip_store
        .get_trajectory_inputs(Source::MtaBus, Utc::now(), None)
        .await
        .expect("get_trajectory_inputs should succeed");

    let trip_rows: Vec<_> = rows.into_iter().filter(|r| r.trip_id == trip_id).collect();
    assert!(
        !trip_rows.is_empty(),
        "expected rows for the synthetic trip"
    );
    for row in &trip_rows {
        assert_eq!(
            row.shape_id, B94_REAL_SHAPE,
            "hit-count should win over the geometrically-closer decoy"
        );
    }
}

/// Before a fresh static import populates `route_stop.data->'shape_ids'`, every
/// candidate has zero hits. Resolution must still fall back to the old average-
/// distance ranking rather than erroring or dropping the trip.
#[sqlx::test]
async fn falls_back_to_distance_when_no_hit_data_exists(pool: sqlx::PgPool) {
    let redis_pool = setup_redis().await;
    let stores = test_stores(pool.clone(), redis_pool);

    mta_bus_dataset()
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

    let trip_id = insert_trip_and_stop_times(&pool, "B94", &B94_STOP_IDS).await;

    let rows = stores
        .trip_store
        .get_trajectory_inputs(Source::MtaBus, Utc::now(), None)
        .await
        .expect("get_trajectory_inputs should succeed even with no hit data");

    let trip_rows: Vec<_> = rows.into_iter().filter(|r| r.trip_id == trip_id).collect();
    assert!(
        !trip_rows.is_empty(),
        "should still resolve a shape via the distance fallback"
    );
}
