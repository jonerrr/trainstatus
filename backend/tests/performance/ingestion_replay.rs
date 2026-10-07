use crate::support;
use backend::{
    models::source::Source,
    realtime::{CollectedSnapshot, RealtimeIngestor},
};
use uuid::Uuid;

/// Fixture-derived workload matching the original approximately 95k-row cycle.
/// Identical replay must avoid both physical stop writes and tuple-lock WAL.
#[sqlx::test]
#[ignore = "large snapshot WAL benchmark; run explicitly against local PostgreSQL"]
async fn replay_large_snapshot_reports_write_counts(pool: sqlx::PgPool) {
    use backend::{
        fixtures::{self, FixtureKind},
        sources::mta_bus::realtime::MtaBusRealtime,
    };

    let stores = support::test_stores(pool.clone());
    let dataset = support::mta_bus_dataset();
    stores.static_data_store.persist(&dataset).await.unwrap();
    let root = support::fixture_root();
    let manifest =
        fixtures::load_manifest_for(&root, Source::MtaBus, FixtureKind::Realtime, "basic").unwrap();
    let feed = fixtures::read_gtfs_realtime_payload(&root, &manifest, "trip_updates").unwrap();
    let fixture = MtaBusRealtime.build_snapshot(vec![feed], vec![]);
    let valid_stops: std::collections::HashSet<_> =
        dataset.stops.iter().map(|s| s.id.as_str()).collect();
    let template = fixture
        .trips
        .into_iter()
        .find(|(trip, stops)| {
            dataset
                .routes
                .iter()
                .any(|r| r.id.as_str() == trip.route_id)
                && stops
                    .iter()
                    .map(|s| s.stop_id.as_str())
                    .collect::<std::collections::HashSet<_>>()
                    .len()
                    >= 17
                && stops
                    .iter()
                    .all(|s| valid_stops.contains(s.stop_id.as_str()))
        })
        .expect("fixture has a route with at least 17 FK-valid unique stops");
    let mut unique_stops = std::collections::HashSet::new();
    let template_stops: Vec<_> = template
        .1
        .into_iter()
        .filter(|s| unique_stops.insert(s.stop_id.clone()))
        .collect();
    let trips = (0..5800)
        .map(|n| {
            let mut trip = template.0.clone();
            trip.id = Uuid::now_v7();
            trip.original_id = format!("wal-benchmark-trip-{n}");
            trip.vehicle_id = format!("wal-benchmark-vehicle-{n}");
            let stops = template_stops
                .iter()
                .take(if n < 2200 { 17 } else { 16 })
                .cloned()
                .map(|mut s| {
                    s.trip_id = trip.id;
                    s
                })
                .collect();
            (trip, stops)
        })
        .collect();
    let input = CollectedSnapshot {
        source: Source::MtaBus,
        trips,
        positions: vec![],
    };
    let ingestor = RealtimeIngestor::new(
        pool.clone(),
        stores.live_snapshots.clone(),
        stores.static_data_store.static_index(),
    );
    let initial_started = std::time::Instant::now();
    let first = ingestor.ingest(input.clone()).await.unwrap();
    let initial_ms = initial_started.elapsed().as_millis();
    assert_eq!(first.trips.len(), 5800);
    assert_eq!(first.stop_times.len(), 95000);
    assert_eq!(first.changes.stop_times, 95000);
    let before: Vec<(Uuid, String, String)> = sqlx::query_as(
        "SELECT trip_id, stop_id, xmin::text FROM realtime.stop_time ORDER BY trip_id, stop_id",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    let lsn_before: (String,) = sqlx::query_as("SELECT pg_current_wal_lsn()::text")
        .fetch_one(&pool)
        .await
        .unwrap();
    let replay_started = std::time::Instant::now();
    let replay = ingestor.ingest(input).await.unwrap();
    let replay_ms = replay_started.elapsed().as_millis();
    let lsn_after: (String,) = sqlx::query_as("SELECT pg_current_wal_lsn()::text")
        .fetch_one(&pool)
        .await
        .unwrap();
    let wal_bytes: (i64,) =
        sqlx::query_as("SELECT pg_wal_lsn_diff($1::pg_lsn, $2::pg_lsn)::bigint")
            .bind(&lsn_after.0)
            .bind(&lsn_before.0)
            .fetch_one(&pool)
            .await
            .unwrap();
    let after: Vec<(Uuid, String, String)> = sqlx::query_as(
        "SELECT trip_id, stop_id, xmin::text FROM realtime.stop_time ORDER BY trip_id, stop_id",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    let changed_versions = before.iter().zip(&after).filter(|(a, b)| a != b).count();
    println!(
        "trips=5800 stop_times=95000 initial_ms={initial_ms} replay_ms={replay_ms} changes={:?} changed_tuple_versions={changed_versions} wal_before={} wal_after={} wal_bytes={}",
        replay.changes, lsn_before.0, lsn_after.0, wal_bytes.0
    );
    assert_eq!(before.len(), after.len());
    assert_eq!(replay.changes.stop_times, 0);
    assert_eq!(changed_versions, 0);
    assert!(replay.changed_trip_ids.is_empty());
    assert!(
        wal_bytes.0 < 7_160_000,
        "replay WAL must be at least 90% below the recorded 71.6 MB baseline"
    );
}
