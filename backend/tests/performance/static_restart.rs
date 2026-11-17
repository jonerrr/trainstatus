use crate::support::{TestContext, fixture_root, test_owners};
use backend::network::expansion::expand_gtfs;
use backend::source::Source;
use backend::sources::njt_bus::patterns::PatternFeature;
use backend::sources::njt_bus::static_data::build_static_dataset_from_patterns;
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

#[tokio::test]
#[ignore = "full captured NJT import/restart measurement; run on an idle machine"]
async fn njt_static_import_and_restart_memory() {
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
    let captured_at = chrono::DateTime::from_timestamp(1_791_340_688, 0).unwrap();
    dataset.scheduled_trips = expand_gtfs(&gtfs, captured_at, |template| {
        backend::sources::njt_bus::static_data::prepare_schedule_template(
            template,
            &dataset.stop_remap,
        );
    });
    let expanded = dataset.scheduled_trips.len();
    assert!(
        expanded > 0,
        "capture service calendar must cover the explicit capture date"
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
        .map(|trip| (trip.template.trip_id.clone(), trip.start_date))
        .collect();
    let ctx = TestContext::new();
    let storage = ctx.storage();
    let stores = test_owners(storage.clone());
    let retained_dataset = dataset.clone();
    let start = Instant::now();
    stores.network.persist(dataset).await.unwrap();
    let import_ms = start.elapsed().as_millis();
    let mut dataset = retained_dataset;
    let old = stores.network.network_index().get(Source::NjtBus).unwrap();
    shift_service_dates(&mut dataset, 1);
    retained_keys.extend(
        dataset
            .scheduled_trips
            .iter()
            .map(|trip| (trip.template.trip_id.clone(), trip.start_date)),
    );
    let restored_expected = retained_keys.len();
    drop(retained_keys);
    // Re-import while a worker pins the previous revision to measure overlap.
    stores.network.persist(dataset).await.unwrap();
    let overlap_rss = rss_kib();
    drop(old);
    let steady_rss = rss_kib();
    let restart = test_owners(storage);
    let start = Instant::now();
    let restored = restart
        .network
        .restore_revision(Source::NjtBus)
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

fn shift_service_dates(dataset: &mut backend::network::dataset::NetworkDataset, days: i64) {
    let offset = chrono::Duration::days(days);
    for trip in &mut dataset.scheduled_trips {
        trip.start_date += offset;
    }
}
