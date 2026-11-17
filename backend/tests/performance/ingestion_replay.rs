//! Opt-in ingestion replay benchmark on local storage (no PostgreSQL WAL).

use crate::support::{TestContext, mta_bus_dataset, test_owners};
use backend::{
    fixtures::{self, FixtureKind},
    operations::{identity::RunId, source::CollectedSnapshot},
    source::Source,
    sources::mta_bus::realtime::MtaBusRealtime,
};

#[tokio::test]
#[ignore = "large ingestion replay benchmark; run explicitly"]
async fn replay_large_snapshot_reports_ingestion_timing() {
    let ctx = TestContext::new();
    let storage = ctx.storage();
    let stores = test_owners(storage);
    let dataset = mta_bus_dataset();
    stores.network.persist(dataset).await.unwrap();
    let root = crate::support::fixture_root();
    let manifest =
        fixtures::load_manifest_for(&root, Source::MtaBus, FixtureKind::Realtime, "basic").unwrap();
    let feed = fixtures::read_gtfs_realtime_payload(&root, &manifest, "trip_updates").unwrap();
    let fixture = MtaBusRealtime.build_snapshot(vec![feed], vec![]);
    let template = fixture
        .trips
        .into_iter()
        .find(|(_trip, stops)| stops.len() >= 17)
        .expect("fixture trip with enough stops");
    let trips: Vec<_> = (0..200)
        .map(|n| {
            let mut trip = template.0.clone();
            trip.id = RunId::arbitrary(&format!("bench-{n}"));
            trip.original_id = format!("bench-trip-{n}");
            trip.partition_date =
                backend::operations::identity::eastern_service_date(trip.created_at);
            let stops = template
                .1
                .iter()
                .take(17)
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
        network_revision: stores.network.network_index().get(Source::MtaBus),
        renormalize: None,
        trips,
        positions: vec![],
    };
    let ingestor = stores.ingestor;
    let initial_started = std::time::Instant::now();
    let first = ingestor.ingest(input.clone()).await.unwrap();
    println!(
        "initial_ms={} trips={} stop_times={} changes={:?}",
        initial_started.elapsed().as_millis(),
        first.trips.len(),
        first.stop_times.len(),
        first.changes
    );
    let replay_started = std::time::Instant::now();
    let replay = ingestor.ingest(input).await.unwrap();
    println!(
        "replay_ms={} changes={:?}",
        replay_started.elapsed().as_millis(),
        replay.changes
    );
    assert_eq!(replay.changes.stop_times, 0);
}
