use crate::support::{fixture_root, test_stores};
use backend::{
    models::source::Source,
    sources::njt_bus::{patterns::PatternFeature, static_data::build_static_dataset_from_patterns},
    static_data::expansion::expand_gtfs,
};
use std::time::Instant;

fn memory_kib(field: &str) -> u64 {
    std::fs::read_to_string("/proc/self/status")
        .unwrap()
        .lines()
        .find_map(|line| {
            line.strip_prefix(field)
                .map(|s| s.split_whitespace().next().unwrap().parse().unwrap())
        })
        .unwrap()
}

fn rss_kib() -> u64 {
    memory_kib("VmRSS:")
}

#[sqlx::test]
#[ignore = "full captured NJT import/restart measurement; run on an idle database"]
async fn njt_static_import_and_restart_memory(pool: sqlx::PgPool) {
    let root = fixture_root().join("njt_bus/static/basic/raw");
    let start = Instant::now();
    let gtfs = gtfs_structures::GtfsReader::default()
        .read_shapes(false)
        .raw()
        .read_from_reader(std::fs::File::open(root.join("gtfs.zip")).unwrap())
        .and_then(gtfs_structures::Gtfs::try_from)
        .unwrap();
    let features: Vec<PatternFeature> =
        serde_json::from_slice(&std::fs::read(root.join("operating_patterns.json")).unwrap())
            .unwrap();
    let mut dataset = build_static_dataset_from_patterns(&gtfs, features).dataset;
    dataset.scheduled_trips = expand_gtfs(Source::NjtBus, &gtfs);
    for trip in &mut dataset.scheduled_trips {
        for stop in &mut trip.stop_times {
            if let Some(id) = dataset.stop_remap.get(&stop.stop_id) {
                stop.stop_id.clone_from(id);
            }
        }
    }
    let expanded = dataset.scheduled_trips.len();
    assert!(
        expanded > 0,
        "capture service calendar must cover today's measurement; refresh fixture if it has expired"
    );
    let build_ms = start.elapsed().as_millis();
    let build_rss = rss_kib();
    drop(gtfs);
    // First import models yesterday/today, then roll into today/tomorrow while
    // the old revision is pinned. This includes retained overnight schedules.
    shift_service_dates(&mut dataset, -1);
    let mut retained_keys: std::collections::HashSet<_> = dataset
        .scheduled_trips
        .iter()
        .map(|trip| (trip.trip_id.clone(), trip.start_date.clone()))
        .collect();
    let stores = test_stores(pool.clone());
    let start = Instant::now();
    stores.static_data_store.persist(&dataset).await.unwrap();
    let import_ms = start.elapsed().as_millis();
    let old = stores
        .static_data_store
        .static_index()
        .get(Source::NjtBus)
        .unwrap();
    shift_service_dates(&mut dataset, 1);
    retained_keys.extend(
        dataset
            .scheduled_trips
            .iter()
            .map(|trip| (trip.trip_id.clone(), trip.start_date.clone())),
    );
    let restored_expected = retained_keys.len();
    drop(retained_keys);
    // Re-import while a worker pins the previous revision to measure overlap.
    stores.static_data_store.persist(&dataset).await.unwrap();
    let overlap_rss = rss_kib();
    drop(dataset);
    drop(old);
    let steady_rss = rss_kib();
    let restart = test_stores(pool);
    let start = Instant::now();
    let restored = restart
        .static_data_store
        .load_revision(Source::NjtBus)
        .await
        .unwrap()
        .unwrap();
    let restart_ms = start.elapsed().as_millis();
    let restored_count: usize = restored
        .scheduled_trips
        .values()
        .map(|dates| dates.len())
        .sum();
    assert_eq!(restored_count, restored_expected);
    println!(
        "NJT expanded={expanded} retained_total={restored_count} build_ms={build_ms} import_ms={import_ms} restart_ms={restart_ms} build_rss_kib={build_rss} overlap_rss_kib={overlap_rss} steady_rss_kib={steady_rss} restart_rss_kib={} peak_rss_kib={}",
        rss_kib(),
        memory_kib("VmHWM:")
    );
}

fn shift_service_dates(dataset: &mut backend::static_data::dataset::StaticDataset, days: i64) {
    let offset = chrono::Duration::days(days);
    for trip in &mut dataset.scheduled_trips {
        let date = chrono::NaiveDate::parse_from_str(&trip.start_date, "%Y%m%d").unwrap();
        trip.start_date = (date + offset).format("%Y%m%d").to_string();
        trip.start_time += offset;
        for stop in &mut trip.stop_times {
            stop.arrival += offset;
            stop.departure += offset;
        }
    }
}
