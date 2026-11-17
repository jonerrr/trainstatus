use super::*;
use crate::geometry::Geom;
use crate::operations::{
    active::ShapeBinding,
    tests::support::series,
    types::trip::{NjtBusData, StopTimeData, StopVisit, TripData, TripRun},
};
use chrono::TimeZone;

fn run(number: usize, at: DateTime<Utc>) -> ArchivedRun {
    let id = RunId::scheduled_bus(
        Source::NjtBus,
        &format!("bounded-{number}"),
        at.date_naive(),
    );
    let mut gps = GpsSeries::default();
    for sample in 0..80 {
        gps.insert(GpsSample {
            observed_at: at + chrono::Duration::seconds(sample),
            point: geo::Point::new(-74.0 + number as f64 / 100.0, 40.0 + sample as f64 / 1000.0),
        });
    }
    ArchivedRun {
        trip: TripRun {
            id,
            original_id: format!("bounded-{number}"),
            vehicle_id: format!("bus-{number}"),
            route_id: "1".into(),
            shape_ids: vec!["shape".into()],
            direction: 0,
            created_at: at,
            updated_at: at,
            partition_date: at.date_naive(),
            data: TripData::NjtBus(NjtBusData {
                deviation: None,
                headsign: "Bounded".into(),
            }),
        },
        visits: vec![StopVisit {
            trip_id: id,
            stop_id: "stop".into(),
            stop_sequence: 1,
            arrival: at,
            departure: at + chrono::Duration::minutes(1),
            data: StopTimeData::NjtBus,
        }],
        gps,
        shapes: vec![ShapeBinding {
            provider_shape_id: "shape".into(),
            asset_id: ContentId::hash(b"asset"),
        }],
        min_call: at,
        max_call: at + chrono::Duration::minutes(1),
    }
}

fn batch(runs: Vec<ArchivedRun>) -> ArchiveBatch {
    ArchiveBatch {
        source: Source::NjtBus,
        runs,
        dictionary: RetainedDictionary {
            routes: HashMap::from([("1".into(), "123456".into())]),
            stops: HashMap::from([(
                "stop".into(),
                (
                    Geom::from(geo::Point::new(-74.0, 40.0)),
                    crate::network::types::stop::StopData::NjtBus(
                        crate::network::types::stop::NjtBusStopData {
                            stop_code: "10001".into(),
                        },
                    ),
                ),
            )]),
        },
    }
}

fn open(dir: &std::path::Path) -> ArchiveStore {
    ArchiveStore::open(
        dir.to_path_buf(),
        Source::NjtBus,
        GpsStore::open(dir.join("blocks")).unwrap(),
    )
    .unwrap()
}

#[test]
fn unchanged_records_reuse_sealed_pages_and_keep_pinned_baselines() {
    let dir = std::env::temp_dir().join(format!("archive-reuse-{}", uuid::Uuid::new_v4()));
    let at = Utc.with_ymd_and_hms(2026, 6, 1, 9, 0, 0).unwrap();
    let mut store = open(&dir);
    let mut original = run(0, at);
    original.gps = series(
        &store.gps_store,
        (0..300).map(|offset| GpsSample {
            observed_at: at + chrono::Duration::seconds(offset),
            point: geo::Point::new(-74.0, 40.0 + offset as f64 / 1000.0),
        }),
    );
    let id = original.trip.id;
    let date = partition_key(at.date_naive());
    store.append_batch(batch(vec![original.clone()])).unwrap();
    let first = store
        .load_head_page(&date)
        .unwrap()
        .unwrap()
        .runs
        .remove(&id)
        .unwrap();
    let locator = HashMap::from([(date.clone(), vec![id])]);
    let (baseline, _) = store
        .shadow_runs_with_refs(&locator)
        .unwrap()
        .remove(&id)
        .unwrap();
    store.append_batch(batch(vec![original.clone()])).unwrap();
    let second = store
        .load_head_page(&date)
        .unwrap()
        .unwrap()
        .runs
        .remove(&id)
        .unwrap();
    assert_eq!(first.passenger.record, second.passenger.record);
    let gps_store = store.gps_store.clone();
    let gps_blocks = |gps: &GpsSeries| {
        let mut blocks = HashSet::new();
        gps_store.live_blocks(gps, &mut blocks).unwrap();
        blocks
    };
    assert_eq!(gps_blocks(&first.gps), gps_blocks(&second.gps));
    original.gps.insert(GpsSample {
        observed_at: at + chrono::Duration::seconds(300),
        point: geo::Point::new(-73.0, 41.0),
    });
    store.append_batch(batch(vec![original.clone()])).unwrap();
    let third = store
        .load_head_page(&date)
        .unwrap()
        .unwrap()
        .runs
        .remove(&id)
        .unwrap();
    assert_eq!(second.passenger.record, third.passenger.record);
    let before = gps_blocks(&second.gps);
    let after = gps_blocks(&third.gps);
    assert!(!before.is_disjoint(&after));
    assert_ne!(before, after);
    let selected = store
        .gps_anchors(
            at + chrono::Duration::seconds(300),
            &[original.trip.clone()],
        )
        .unwrap();
    assert_eq!(selected[&id].point, geo::Point::new(-73.0, 41.0));
    let pinned = store
        .read_gps_window(
            &baseline,
            at + chrono::Duration::seconds(299),
            at + chrono::Duration::seconds(300),
        )
        .unwrap();
    assert_eq!(
        pinned
            .iter()
            .map(|sample| sample.observed_at)
            .collect::<Vec<_>>(),
        [at + chrono::Duration::seconds(299)]
    );
    drop(store);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn records_preserve_pinned_history_and_failed_publication() {
    let dir = std::env::temp_dir().join(format!("archive-bounds-{}", uuid::Uuid::new_v4()));
    let at = Utc.with_ymd_and_hms(2026, 6, 1, 9, 0, 0).unwrap();
    let mut store = open(&dir);
    store
        .append_batch_bounded(batch(vec![run(0, at)]), 32_768)
        .unwrap();
    let pinned = store.clone();
    let first_id = run(0, at).trip.id;
    let date = partition_key(at.date_naive());
    let original_locators = HashMap::from([(date.clone(), vec![first_id])]);
    let (reference, _) = store
        .shadow_runs_with_refs(&original_locators)
        .unwrap()
        .remove(&first_id)
        .unwrap();
    let mut replacement: Vec<_> = (0..4).map(|number| run(number, at)).collect();
    replacement[0].gps.insert(GpsSample {
        observed_at: at + chrono::Duration::seconds(90),
        point: geo::Point::new(-72.0, 42.0),
    });
    replacement[0].trip.updated_at = at + chrono::Duration::minutes(3);
    let expected = replacement.clone();
    store
        .append_batch_bounded(batch(replacement), 32_768)
        .unwrap();
    let old = pinned
        .gps_anchors(at + chrono::Duration::seconds(90), &[run(0, at).trip])
        .unwrap();
    assert_eq!(
        old[&first_id].observed_at,
        at + chrono::Duration::seconds(79)
    );
    let locators = HashMap::from([(
        date.clone(),
        expected.iter().map(|run| run.trip.id).collect(),
    )]);
    for (id, actual) in store.shadow_runs(&locators).unwrap() {
        let expected = expected.iter().find(|run| run.trip.id == id).unwrap();
        assert_eq!(actual.trip.updated_at, expected.trip.updated_at);
        assert_eq!(actual.visits[0].arrival, expected.visits[0].arrival);
        assert_eq!(actual.shapes[0].asset_id, ContentId::hash(b"asset"));
        let samples = |run: &ArchivedRun| {
            store
                .gps_store
                .all(&run.gps)
                .unwrap()
                .into_iter()
                .map(|sample| (sample.observed_at, sample.point))
                .collect::<Vec<_>>()
        };
        assert_eq!(samples(&actual), samples(expected));
    }
    let committed = std::fs::read(&store.manifest_path).unwrap();
    let sequence = store.manifest_seq();
    let mut failed = expected.clone();
    failed[0].trip.original_id = "uncommitted".into();
    assert!(store.append_batch_bounded(batch(failed), 1).is_err());
    assert_eq!(std::fs::read(&store.manifest_path).unwrap(), committed);
    assert_eq!(store.manifest_seq(), sequence);
    assert_ne!(
        store.shadow_runs(&locators).unwrap()[&first_id]
            .trip
            .original_id,
        "uncommitted"
    );
    drop(store);
    let reopened = open(&dir);
    assert_eq!(
        reopened
            .gps_anchors(
                at + chrono::Duration::seconds(90),
                &[expected[0].trip.clone()]
            )
            .unwrap()[&first_id]
            .point,
        geo::Point::new(-72.0, 42.0)
    );
    let prior = reopened.read_run_ref(&reference).unwrap();
    assert_eq!(
        reopened
            .gps_store
            .all(&prior.gps)
            .unwrap()
            .last()
            .unwrap()
            .observed_at,
        at + chrono::Duration::seconds(79)
    );
    drop(reopened);
    drop(pinned);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn post_rename_root_error_reconciles_publication_before_retry() {
    let dir = std::env::temp_dir().join(format!("archive-root-error-{}", uuid::Uuid::new_v4()));
    let at = Utc.with_ymd_and_hms(2026, 6, 1, 9, 0, 0).unwrap();
    let mut store = open(&dir);
    store
        .append_batch_bounded(batch(vec![run(0, at)]), 32_768)
        .unwrap();
    let mut published = (*store.manifest).clone();
    published.sequence += 1;
    let sequence = published.sequence;
    assert!(
        store
            .commit_manifest(published, |path, manifest| {
                write_atomic(path, manifest)?;
                anyhow::bail!("injected post-rename directory synchronization failure")
            })
            .is_err()
    );
    assert!(store.publication_uncertain);
    assert!(store.live_blocks(&dir.join("checkpoint")).is_err());
    store.reconcile_publication().unwrap();
    assert_eq!(store.manifest_seq(), sequence);
    assert!(!store.publication_uncertain);
    store
        .append_batch_bounded(batch(vec![run(1, at)]), 32_768)
        .unwrap();
    assert_eq!(store.manifest_seq(), sequence + 1);
    let locators = HashMap::from([(
        partition_key(at.date_naive()),
        vec![run(0, at).trip.id, run(1, at).trip.id],
    )]);
    let selected = store.shadow_runs(&locators).unwrap();
    assert_eq!(selected[&run(0, at).trip.id].trip.original_id, "bounded-0");
    assert_eq!(selected[&run(1, at).trip.id].trip.original_id, "bounded-1");
    drop(store);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn live_blocks_include_pinned_roots_until_reader_drops() {
    let dir = std::env::temp_dir().join(format!("archive-pins-{}", uuid::Uuid::new_v4()));
    let at = Utc.with_ymd_and_hms(2026, 6, 1, 9, 0, 0).unwrap();
    let mut store = open(&dir);
    let mut original = run(0, at);
    let id = original.trip.id;
    let date = partition_key(at.date_naive());
    store.append_batch(batch(vec![original.clone()])).unwrap();
    let old_block = store.load_head_page(&date).unwrap().unwrap().runs[&id]
        .passenger
        .record
        .block;
    let pinned = store.clone();
    original.trip.original_id = "changed".into();
    store.append_batch(batch(vec![original])).unwrap();
    let live = store.live_blocks(&dir.join("checkpoint")).unwrap();
    assert!(live.contains(&old_block));
    drop(pinned);
    let live = store.live_blocks(&dir.join("checkpoint")).unwrap();
    assert!(!live.contains(&old_block));
    drop(store);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn locator_reads_only_its_bounded_head_page() {
    let dir = std::env::temp_dir().join(format!("archive-pages-{}", uuid::Uuid::new_v4()));
    let at = Utc.with_ymd_and_hms(2026, 6, 1, 9, 0, 0).unwrap();
    let mut store = open(&dir);
    let runs: Vec<_> = (0..130).map(|n| run(n, at)).collect();
    store.append_batch(batch(runs)).unwrap();
    let date = partition_key(at.date_naive());
    let root = &store.manifest.partitions[&date].root;
    let HeadNode::Branch(children) = store.blocks.read::<HeadNode>(&root.record).unwrap() else {
        panic!("expected multiple head leaves")
    };
    assert!(children.len() >= 2);
    let selected = children[0].first;
    std::fs::remove_file(
        store
            .blocks
            .root()
            .join(format!("{}.trsb", children[1].record.block)),
    )
    .unwrap();
    drop(store);
    let store = open(&dir);
    let found = store
        .shadow_runs(&HashMap::from([(date, vec![selected])]))
        .unwrap();
    assert!(found.contains_key(&selected));
    drop(store);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn updating_one_head_reuses_unrelated_tree_branch() {
    let dir = std::env::temp_dir().join(format!("archive-cow-{}", uuid::Uuid::new_v4()));
    let at = Utc.with_ymd_and_hms(2026, 6, 1, 9, 0, 0).unwrap();
    let mut store = open(&dir);
    let runs: Vec<_> = (0..130).map(|n| run(n, at)).collect();
    store.append_batch(batch(runs.clone())).unwrap();
    let date = partition_key(at.date_naive());
    let HeadNode::Branch(before) = store
        .blocks
        .read::<HeadNode>(&store.manifest.partitions[&date].root.record)
        .unwrap()
    else {
        panic!("expected branch")
    };
    let mut changed = runs
        .into_iter()
        .find(|run| run.trip.id == before[0].first)
        .unwrap();
    changed.trip.original_id = "updated".into();
    store.append_batch(batch(vec![changed])).unwrap();
    let HeadNode::Branch(after) = store
        .blocks
        .read::<HeadNode>(&store.manifest.partitions[&date].root.record)
        .unwrap()
    else {
        panic!("expected branch")
    };
    assert_eq!(before.last().unwrap().record, after.last().unwrap().record);
    drop(store);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn passenger_projection_skips_dictionary_and_route_filter_skips_passenger() {
    let dir = std::env::temp_dir().join(format!("archive-projection-{}", uuid::Uuid::new_v4()));
    let at = Utc.with_ymd_and_hms(2026, 6, 1, 9, 0, 0).unwrap();
    let mut store = open(&dir);
    let original = run(0, at);
    let id = original.trip.id;
    store.append_batch(batch(vec![original])).unwrap();
    let entry = store
        .load_head_page(&partition_key(at.date_naive()))
        .unwrap()
        .unwrap()
        .runs
        .remove(&id)
        .unwrap();
    std::fs::remove_file(
        store
            .blocks
            .root()
            .join(format!("{}.trsb", entry.dictionary.block)),
    )
    .unwrap();
    let (trips, _, _) = store
        .query_service(at, None, &HashSet::new(), ArchiveProjection::Passenger)
        .unwrap();
    assert_eq!(trips.len(), 1);
    std::fs::remove_file(
        store
            .blocks
            .root()
            .join(format!("{}.trsb", entry.passenger.record.block)),
    )
    .unwrap();
    drop(store);
    let store = open(&dir);
    let (trips, _, _) = store
        .query_service(
            at,
            Some(&["other".into()]),
            &HashSet::new(),
            ArchiveProjection::Passenger,
        )
        .unwrap();
    assert!(trips.is_empty());
    drop(store);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn dictionaries_are_canonical_source_typed_and_reconstruct_for_trajectory() {
    use crate::network::types::stop::{NjtBusStopData, StopData};
    let dir = std::env::temp_dir().join(format!("archive-dictionary-{}", uuid::Uuid::new_v4()));
    let at = Utc.with_ymd_and_hms(2026, 6, 1, 9, 0, 0).unwrap();
    let mut store = open(&dir);
    let mut original = run(0, at);
    let mut second = original.visits[0].clone();
    second.stop_id = "another".into();
    second.stop_sequence = 2;
    original.visits.push(second);
    let mut input = batch(vec![original.clone()]);
    input.dictionary.stops.insert(
        "another".into(),
        (
            Geom::from(geo::Point::new(-73.0, 41.0)),
            StopData::NjtBus(NjtBusStopData {
                stop_code: "20002".into(),
            }),
        ),
    );
    store.append_batch(input.clone()).unwrap();
    let date = partition_key(at.date_naive());
    let first = store.load_head_page(&date).unwrap().unwrap().runs[&original.trip.id]
        .dictionary
        .clone();
    let native: DictionaryRecord<String, String, Geom, NjtBusStopData> =
        store.blocks.read(&first).unwrap();
    assert_eq!(native.stops["stop"].1.stop_code, "10001");
    assert_eq!(native.stops["another"].1.stop_code, "20002");
    let (_, _, retained) = store
        .query_service(at, None, &HashSet::new(), ArchiveProjection::Trajectory)
        .unwrap();
    assert_eq!(retained.routes["1"], "123456");
    assert!(
        matches!(&retained.stops["another"].1, StopData::NjtBus(data) if data.stop_code == "20002")
    );
    let mut reordered = input.clone();
    reordered.dictionary.stops = input.dictionary.stops.into_iter().collect();
    store.append_batch(reordered).unwrap();
    let second = &store.load_head_page(&date).unwrap().unwrap().runs[&original.trip.id].dictionary;
    assert_eq!(&first, second);
    drop(store);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn actual_calls_not_interval_overlap_control_inclusive_selection() {
    let dir = std::env::temp_dir().join(format!("archive-calls-{}", uuid::Uuid::new_v4()));
    let at = Utc.with_ymd_and_hms(2026, 6, 1, 9, 0, 0).unwrap();
    let mut store = open(&dir);
    let mut original = run(0, at);
    original.visits[0].arrival = at - chrono::Duration::seconds(1);
    original.visits[0].departure = at + chrono::Duration::hours(4) + chrono::Duration::seconds(1);
    original.min_call = original.visits[0].arrival;
    original.max_call = original.visits[0].departure;
    store.append_batch(batch(vec![original.clone()])).unwrap();
    assert!(
        store
            .query_service(at, None, &HashSet::new(), ArchiveProjection::Passenger)
            .unwrap()
            .0
            .is_empty()
    );
    original.visits[0].arrival = at;
    original.visits[0].departure = at + chrono::Duration::hours(4);
    original.min_call = original.visits[0].arrival;
    original.max_call = original.visits[0].departure;
    store.append_batch(batch(vec![original])).unwrap();
    let (trips, visits, _) = store
        .query_service(at, None, &HashSet::new(), ArchiveProjection::Passenger)
        .unwrap();
    assert_eq!(trips.len(), 1);
    assert_eq!(visits[0].arrival, at);
    assert_eq!(visits[0].departure, at + chrono::Duration::hours(4));
    drop(store);
    std::fs::remove_dir_all(dir).unwrap();
}
