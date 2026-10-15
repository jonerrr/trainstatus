use crate::support;
use backend::fixtures;
use backend::fixtures::FixtureKind;
use backend::operations::CollectedSnapshot;
use backend::operations::OperationsIngestor;
use backend::source::Source;
use backend::sources::mta_bus::realtime::MtaBusRealtime;
use uuid::Uuid;

/// Fixture-derived workload matching the original approximately 95k-row cycle.
/// Identical replay must avoid both physical stop writes and tuple-lock WAL.
#[sqlx::test]
#[ignore = "large snapshot WAL benchmark; run explicitly against local PostgreSQL"]
async fn replay_large_snapshot_reports_write_counts(pool: sqlx::PgPool) {
    let stores = support::test_owners(pool.clone());
    let dataset = support::mta_bus_dataset();
    let import_started = std::time::Instant::now();
    stores.network.persist(&dataset).await.unwrap();
    println!(
        "captured_network_import_ms={} routes={} stops={} shapes={}",
        import_started.elapsed().as_millis(),
        dataset.routes.len(),
        dataset.stops.len(),
        dataset.shapes.len()
    );
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
            trip.id = Uuid::now_v7().into();
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
        network_revision: None,
        renormalize: None,
        trips,
        positions: vec![],
    };
    let ingestor = OperationsIngestor::new(
        pool.clone(),
        stores.live_snapshots.clone(),
        stores.network.network_index(),
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
    let replay = ingestor.ingest(input.clone()).await.unwrap();
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
    assert!(
        wal_bytes.0 < 7_160_000,
        "replay WAL must be at least 90% below the recorded 71.6 MB baseline"
    );
    report_space(&pool, "identical_replay").await;
    let mut changed = input.clone();
    changed.trips[0].1[0].arrival += chrono::Duration::seconds(30);
    let updated = ingestor.ingest(changed).await.unwrap();
    assert_eq!(updated.changes.stop_times, 1);
    report_space(&pool, "one_changed_call").await;
    // Report compact sample growth separately from the latest-record workload.
    let mut gps = input;
    gps.trips.truncate(1);
    let run = &gps.trips[0].0;
    gps.positions
        .push(backend::operations::CollectedObservation {
            vehicle_id: run.vehicle_id.clone(),
            trip_id: Some(run.id),
            stop_id: None,
            updated_at: run.updated_at,
            geom: Some(geo::Point::new(-73.99, 40.75).into()),
            data: backend::operations::types::position::PositionData::MtaBus(
                backend::operations::types::position::MtaBusPositionData {
                    bearing: 0.0,
                    passengers: None,
                    capacity: None,
                    status: None,
                    phase: None,
                },
            ),
        });
    ingestor.ingest(gps.clone()).await.unwrap();
    let identical = ingestor.ingest(gps.clone()).await.unwrap();
    assert_eq!(identical.changes.positions, 0);
    gps.positions[0].updated_at += chrono::Duration::seconds(30);
    ingestor.ingest(gps).await.unwrap();
    let counts: (i64,i64,i64,i64) = sqlx::query_as("SELECT (SELECT count(*) FROM realtime.trip), (SELECT count(*) FROM realtime.stop_time), (SELECT count(*) FROM realtime.vehicle_position), (SELECT count(*) FROM realtime.gps_sample)").fetch_one(&pool).await.unwrap();
    assert_eq!(counts, (5800, 95000, 1, 2));
    println!(
        "durable_counts runs={} visits={} observations={} gps_samples={}",
        counts.0, counts.1, counts.2, counts.3
    );
    report_space(&pool, "gps_growth").await;
    sqlx::query("ANALYZE realtime.trip, realtime.stop_time, realtime.gps_sample")
        .execute(&pool)
        .await
        .unwrap();
    let at = first
        .stop_times
        .iter()
        .map(|visit| visit.arrival)
        .min()
        .unwrap();
    let plans: Vec<(String,)> = sqlx::query_as("EXPLAIN (ANALYZE, BUFFERS) SELECT t.id FROM realtime.trip t WHERE t.source = $1 AND EXISTS (SELECT 1 FROM realtime.stop_time v WHERE v.trip_id = t.id AND v.source = $1 AND (v.arrival BETWEEN $2 AND $2 + INTERVAL '4 hours' OR v.departure BETWEEN $2 AND $2 + INTERVAL '4 hours'))")
        .bind(Source::MtaBus).bind(at).fetch_all(&pool).await.unwrap();
    for (line,) in plans {
        println!("history_plan {line}");
    }
    sqlx::query("VACUUM (ANALYZE) realtime.stop_time")
        .execute(&pool)
        .await
        .unwrap();
    report_space(&pool, "after_vacuum").await;
}

async fn report_space(pool: &sqlx::PgPool, stage: &str) {
    let tables: Vec<(String, i64, i64, i64)> = sqlx::query_as("SELECT n.nspname || '.' || c.relname, pg_table_size(c.oid), pg_indexes_size(c.oid), CASE WHEN c.reltoastrelid = 0 THEN 0::bigint ELSE pg_total_relation_size(c.reltoastrelid) END FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace WHERE c.relkind = 'r' AND ((n.nspname = 'realtime' AND c.relname IN ('trip','stop_time','vehicle_position','gps_sample','run_shape')) OR (n.nspname = 'static' AND c.relname = 'shape_asset')) ORDER BY 1").fetch_all(pool).await.unwrap();
    for (table, bytes, indexes, toast) in tables {
        println!(
            "space stage={stage} table={table} table_bytes={bytes} index_bytes={indexes} toast_bytes={toast}"
        );
    }
}
