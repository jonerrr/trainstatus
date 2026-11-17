use super::*;
use crate::{
    geometry::Geom,
    network::{
        dataset::NetworkDataset,
        types::{
            route::{Route, RouteData},
            stop::{NjtBusStopData, Stop, StopData},
        },
    },
    operations::{
        active::{ArchivedRun, DeltaPayload, RetainedDictionary},
        archive::ArchiveBatch,
        history::GpsSample,
        identity::RunRef,
        types::{
            position::{NjtBusPositionData, PositionData, VehicleObservation},
            trip::{NjtBusData, StopTimeData, TripData},
        },
    },
};

fn poll(at: DateTime<Utc>) -> CollectedSnapshot {
    let source = Source::NjtBus;
    let id = RunId::scheduled_bus(source, "reopen", at.date_naive());
    let mut dataset = NetworkDataset::new(source);
    dataset.routes.push(Route {
        id: "1".into(),
        long_name: "One".into(),
        short_name: "1".into(),
        color: "FFFFFF".into(),
        text_color: "000000".into(),
        data: RouteData::NjtBus,
    });
    for stop in ["STOP1", "STOP2"] {
        dataset.stops.push(Stop {
            id: stop.into(),
            name: stop.into(),
            geom: Geom::from(geo::Point::new(-74.0, 40.7)),
            nearby_stops: vec![],
            routes: vec![],
            data: StopData::NjtBus(NjtBusStopData {
                stop_code: stop.into(),
            }),
        });
    }
    let trip = TripRun {
        id,
        original_id: "reopen".into(),
        vehicle_id: "bus".into(),
        route_id: "1".into(),
        shape_ids: vec![],
        direction: 0,
        created_at: at,
        updated_at: at,
        partition_date: at.date_naive(),
        data: TripData::NjtBus(NjtBusData {
            deviation: None,
            headsign: "Downtown".into(),
        }),
    };
    let visits = ["STOP1", "STOP2"]
        .into_iter()
        .enumerate()
        .map(|(index, stop)| StopVisit {
            trip_id: id,
            stop_id: stop.into(),
            stop_sequence: index as i32 + 1,
            arrival: at,
            departure: at,
            data: StopTimeData::NjtBus,
        })
        .collect();
    CollectedSnapshot {
        source,
        network_revision: Some(Arc::new(NetworkRevision::from_dataset(&dataset))),
        renormalize: None,
        trips: vec![(trip, visits)],
        positions: vec![],
    }
}

#[test]
fn archived_baseline_is_journalled_once_and_omitted_visits_remain_historical() {
    let root = std::env::temp_dir().join(format!("ops-reopen-once-{}", uuid::Uuid::new_v4()));
    let source = Source::NjtBus;
    let at = Utc::now().trunc_subsecs(6);
    let mut input = poll(at);
    let id = input.trips[0].0.id;
    let owner = OperationsOwner::open(Storage::open(&root).unwrap(), NetworkIndex::new()).unwrap();
    owner
        .commit_blocking(CollectedSnapshot {
            trips: vec![],
            ..input.clone()
        })
        .unwrap();
    {
        let mut writer = owner.sources[source.index()].lock().unwrap();
        let gps = crate::operations::tests::support::series(
            &writer.active.gps_store,
            [GpsSample {
                observed_at: at,
                point: geo::Point::new(-74.0, 40.7),
            }],
        );
        writer
            .archive
            .append_batch(ArchiveBatch {
                source,
                runs: vec![ArchivedRun {
                    trip: input.trips[0].0.clone(),
                    visits: input.trips[0].1.clone(),
                    gps,
                    shapes: vec![],
                    min_call: at,
                    max_call: at,
                }],
                dictionary: RetainedDictionary {
                    routes: HashMap::new(),
                    stops: HashMap::new(),
                },
            })
            .unwrap();
    }
    input.trips[0].1.pop();
    let first = owner.commit_blocking(input.clone()).unwrap().0;
    let second = owner.commit_blocking(input).unwrap().0;
    assert_eq!(
        first
            .stop_times
            .iter()
            .map(|visit| visit.stop_sequence)
            .collect::<Vec<_>>(),
        [1]
    );
    assert_eq!(
        second
            .stop_times
            .iter()
            .map(|visit| visit.stop_sequence)
            .collect::<Vec<_>>(),
        [1]
    );
    assert!(second.positions.is_empty());
    let history = owner.load_service_blocking(source, at, None).unwrap();
    assert_eq!(
        history
            .stop_times
            .iter()
            .map(|visit| visit.stop_sequence)
            .collect::<Vec<_>>(),
        [1, 2]
    );
    let mut frames = Vec::new();
    Journal::<AcceptedPoll>::replay_records(
        &root
            .join("operations")
            .join(source.as_str())
            .join("journal"),
        |_, reader| {
            frames.push(ciborium::de::from_reader::<AcceptedPoll, _>(reader)?);
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(frames[0].baselines.len(), 1);
    assert!(frames[1].baselines.is_empty());
    let DeltaPayload::NjtBus(delta) = &frames[1].payload else {
        panic!("wrong source")
    };
    assert!(
        delta.runs.is_empty()
            && delta.positions.is_empty()
            && delta.gps.is_empty()
            && delta.seals.is_empty()
    );
    drop(owner);
    let recovered =
        OperationsOwner::open(Storage::open(&root).unwrap(), NetworkIndex::new()).unwrap();
    assert_eq!(
        recovered
            .load_service_blocking(source, at, None)
            .unwrap()
            .stop_times
            .iter()
            .map(|visit| visit.stop_sequence)
            .collect::<Vec<_>>(),
        [1, 2]
    );
    assert_eq!(
        recovered
            .load_trajectories_blocking(source, at)
            .unwrap()
            .anchors[&id]
            .point,
        geo::Point::new(-74.0, 40.7)
    );
    drop(recovered);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn gps_only_link_reopens_archived_run_by_partition_date() {
    let root = std::env::temp_dir().join(format!("ops-gps-reopen-{}", uuid::Uuid::new_v4()));
    let source = Source::NjtBus;
    let at = DateTime::from_timestamp(1_700_000_000, 0).unwrap();
    let observed_at = at + chrono::Duration::seconds(1);
    let mut input = poll(at);
    for visit in &mut input.trips[0].1 {
        visit.departure = observed_at;
    }
    let id = input.trips[0].0.id;
    let owner = OperationsOwner::open(Storage::open(&root).unwrap(), NetworkIndex::new()).unwrap();
    owner
        .commit_blocking(CollectedSnapshot {
            trips: vec![],
            ..input.clone()
        })
        .unwrap();
    {
        let mut writer = owner.sources[source.index()].lock().unwrap();
        let gps = crate::operations::tests::support::series(
            &writer.active.gps_store,
            [GpsSample {
                observed_at: at,
                point: geo::Point::new(-74.0, 40.7),
            }],
        );
        writer
            .archive
            .append_batch(ArchiveBatch {
                source,
                runs: vec![ArchivedRun {
                    trip: input.trips[0].0.clone(),
                    visits: input.trips[0].1.clone(),
                    gps,
                    shapes: vec![],
                    min_call: at,
                    max_call: observed_at,
                }],
                dictionary: RetainedDictionary {
                    routes: HashMap::new(),
                    stops: HashMap::new(),
                },
            })
            .unwrap();
    }
    let point = geo::Point::new(-73.999, 40.7);
    let snapshot = owner
        .commit_blocking(CollectedSnapshot {
            trips: vec![],
            positions: vec![VehicleObservation {
                vehicle_id: "bus".into(),
                trip_id: Some(RunRef {
                    id,
                    partition_date: at.date_naive(),
                }),
                stop_id: None,
                updated_at: observed_at,
                data: PositionData::NjtBus(NjtBusPositionData {
                    occupancy_status: Default::default(),
                }),
                geom: Some(Geom::from(point)),
            }],
            ..input
        })
        .unwrap()
        .0;
    assert!(snapshot.trips.is_empty());
    assert_eq!(snapshot.positions[0].trip_id, Some(id));
    assert_eq!(
        owner
            .load_service_blocking(source, at, None)
            .unwrap()
            .stop_times
            .len(),
        2
    );
    assert_eq!(
        owner
            .load_trajectories_blocking(source, observed_at)
            .unwrap()
            .anchors[&id]
            .point,
        point
    );
    drop(owner);
    let recovered =
        OperationsOwner::open(Storage::open(&root).unwrap(), NetworkIndex::new()).unwrap();
    assert_eq!(
        recovered
            .load_service_blocking(source, at, None)
            .unwrap()
            .stop_times
            .len(),
        2
    );
    assert_eq!(
        recovered
            .load_trajectories_blocking(source, observed_at)
            .unwrap()
            .anchors[&id]
            .point,
        point
    );
    drop(recovered);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn recovery_replays_reopens_when_hot_evictions_were_not_checkpointed() {
    let root = std::env::temp_dir().join(format!("ops-reopen-recovery-{}", uuid::Uuid::new_v4()));
    let source = Source::NjtBus;
    let at = DateTime::from_timestamp(1_600_000_000, 0).unwrap();
    let mut input = poll(at);
    let id = input.trips[0].0.id;
    let owner = OperationsOwner::open(Storage::open(&root).unwrap(), NetworkIndex::new()).unwrap();
    owner
        .commit_blocking(CollectedSnapshot {
            trips: vec![],
            ..input.clone()
        })
        .unwrap();
    for step in 0..3 {
        let time = at + chrono::Duration::seconds(step * 15);
        input.trips[0].0.updated_at = time;
        input.trips[0].1[0].arrival = time;
        input.trips[0].1[0].departure = time;
        if step == 1 {
            input.trips[0].1.pop();
        }
        input.positions = vec![VehicleObservation {
            vehicle_id: "bus".into(),
            trip_id: Some(RunRef::from(&input.trips[0].0)),
            stop_id: None,
            updated_at: time,
            data: PositionData::NjtBus(NjtBusPositionData {
                occupancy_status: Default::default(),
            }),
            geom: Some(Geom::from(geo::Point::new(
                -74.0 + step as f64 / 1000.0,
                40.7,
            ))),
        }];
        owner.commit_blocking(input.clone()).unwrap();
    }
    // Every old provider run retires after its poll; the journal still contains all three acceptances.
    assert!(
        owner.sources[source.index()]
            .lock()
            .unwrap()
            .active
            .get_trip(&id)
            .is_none()
    );
    drop(owner);
    let recovered =
        OperationsOwner::open(Storage::open(&root).unwrap(), NetworkIndex::new()).unwrap();
    let service = recovered.load_service_blocking(source, at, None).unwrap();
    assert_eq!(
        service
            .stop_times
            .iter()
            .map(|visit| (visit.stop_sequence, visit.arrival))
            .collect::<Vec<_>>(),
        [(1, at + chrono::Duration::seconds(30)), (2, at)]
    );
    let final_state = recovered
        .load_trajectories_blocking(source, at + chrono::Duration::seconds(30))
        .unwrap();
    assert_eq!(
        final_state.anchors[&id].observed_at,
        at + chrono::Duration::seconds(30)
    );
    assert_eq!(
        final_state.anchors[&id].point,
        geo::Point::new(-73.998, 40.7)
    );
    let original = recovered.load_trajectories_blocking(source, at).unwrap();
    assert_eq!(original.anchors[&id].point, geo::Point::new(-74.0, 40.7));
    drop(recovered);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn missing_checkpoint_after_wal_reset_cannot_publish_an_empty_recovery() {
    let context = crate::operations::tests::support::TempStore::new();
    let source = Source::NjtBus;
    let owner =
        OperationsOwner::open(Storage::open(&context.root).unwrap(), NetworkIndex::new()).unwrap();
    let input = poll(Utc::now().trunc_subsecs(6));
    owner
        .commit_blocking(CollectedSnapshot {
            trips: vec![],
            ..input
        })
        .unwrap();
    drop(owner);
    let source_dir = context.root.join("operations").join(source.as_str());
    let manifest = source_dir
        .join("checkpoint")
        .join(checkpoint::MANIFEST_NAME);
    let journal = source_dir.join("journal");
    let before = std::fs::read(&journal).unwrap();
    std::fs::remove_file(&manifest).unwrap();
    assert!(
        OperationsOwner::open(Storage::open(&context.root).unwrap(), NetworkIndex::new()).is_err()
    );
    assert!(!manifest.exists());
    assert_eq!(std::fs::read(journal).unwrap(), before);
}

#[test]
fn checkpoint_archive_watermark_rejects_a_missing_cold_root_without_resetting_wal() {
    let context = crate::operations::tests::support::TempStore::new();
    let source = Source::NjtBus;
    let owner =
        OperationsOwner::open(Storage::open(&context.root).unwrap(), NetworkIndex::new()).unwrap();
    owner
        .commit_blocking(poll(DateTime::from_timestamp(1_700_000_000, 0).unwrap()))
        .unwrap();
    drop(owner);
    let operations = context.root.join("operations");
    let source_dir = operations.join(source.as_str());
    let checkpoint = source_dir
        .join("checkpoint")
        .join(checkpoint::MANIFEST_NAME);
    let journal = source_dir.join("journal");
    let checkpoint_before = std::fs::read(&checkpoint).unwrap();
    let journal_before = std::fs::read(&journal).unwrap();
    std::fs::remove_file(archived_path(&operations, source).join("manifest.trst")).unwrap();
    assert!(
        OperationsOwner::open(Storage::open(&context.root).unwrap(), NetworkIndex::new()).is_err()
    );
    assert_eq!(std::fs::read(checkpoint).unwrap(), checkpoint_before);
    assert_eq!(std::fs::read(journal).unwrap(), journal_before);
}
