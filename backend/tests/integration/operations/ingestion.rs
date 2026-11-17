use chrono::{Duration, SubsecRound, Utc};

use crate::support::{self, TestContext, eastern_service_date, mta_bus_dataset};
use backend::{
    geometry::Geom,
    network::index::NetworkRevision,
    operations::{
        identity::RunId,
        ingestor::OperationsIngestor,
        live::LiveSnapshots,
        source::CollectedSnapshot,
        types::{
            position::{
                MtaBusPositionData, MtaSubwayPositionData, NjtBusPositionData, OccupancyStatus,
                PositionData, VehicleObservation,
            },
            trip::{
                MtaBusData, MtaSubwayStopTimeData, MtaSubwayTripData, StopTimeData, StopVisit,
                TripData, TripRun,
            },
        },
    },
    source::Source,
};

async fn ingestor() -> (TestContext, OperationsIngestor, LiveSnapshots) {
    let ctx = TestContext::new();
    let stores = support::test_owners(ctx.storage());
    stores
        .network
        .persist(support::mta_subway_dataset())
        .await
        .unwrap();
    (ctx, stores.ingestor, stores.live_snapshots)
}

fn snapshot_now() -> chrono::DateTime<Utc> {
    static NOW: std::sync::LazyLock<chrono::DateTime<Utc>> =
        std::sync::LazyLock::new(|| Utc::now().trunc_subsecs(0));
    *NOW
}

fn snapshot() -> CollectedSnapshot {
    let now = snapshot_now();
    let id = RunId::mta_subway("0A 0808 101/102", eastern_service_date(now)).unwrap();
    let trip = TripRun {
        id,
        original_id: "0A 0808 101/102".into(),
        vehicle_id: "vehicle".into(),
        route_id: "A".into(),
        shape_ids: vec![],
        direction: 1,
        created_at: now,
        updated_at: now,
        partition_date: eastern_service_date(now),
        data: TripData::MtaSubway(MtaSubwayTripData {
            consist: None,
            consist_cars: vec![],
        }),
    };
    let stops = ["101", "102"]
        .into_iter()
        .enumerate()
        .map(|(i, stop_id)| StopVisit {
            trip_id: id,
            stop_sequence: i as i32,
            stop_id: stop_id.into(),
            arrival: now + Duration::minutes(5),
            departure: now + Duration::minutes(6),
            data: StopTimeData::MtaSubway(MtaSubwayStopTimeData {
                scheduled_track: None,
                actual_track: None,
                platform_edges: vec![],
            }),
        })
        .collect();
    let position = VehicleObservation {
        vehicle_id: "vehicle".into(),
        trip_id: Some((&trip).into()),
        stop_id: Some("101".into()),
        updated_at: now,
        data: PositionData::MtaSubway(MtaSubwayPositionData {
            assigned: true,
            status: None,
        }),
        geom: None,
    };
    CollectedSnapshot {
        source: Source::MtaSubway,
        network_revision: None,
        renormalize: None,
        trips: vec![(trip, stops)],
        positions: vec![position],
    }
}

fn set_occurrence(input: &mut CollectedSnapshot, source: Source, original_id: &str) {
    input.source = source;
    let (trip, visits) = &mut input.trips[0];
    let previous = backend::operations::identity::RunRef::from(&*trip);
    trip.original_id = original_id.into();
    trip.id = match source {
        Source::MtaSubway => RunId::mta_subway(original_id, trip.partition_date).unwrap(),
        Source::MtaBus | Source::NjtBus => {
            RunId::scheduled_bus(source, original_id, trip.partition_date)
        }
    };
    for visit in visits {
        visit.trip_id = trip.id;
    }
    for position in &mut input.positions {
        if position.trip_id == Some(previous) {
            position.trip_id = Some((&*trip).into());
        }
    }
}

fn selected_shape(
    snapshot: &backend::operations::ingestor::SourceSnapshot,
) -> std::sync::Arc<Geom> {
    let ids = &snapshot.trips[0].shape_ids;
    assert_eq!(ids.len(), 1, "exactly one accepted shape must be published");
    snapshot
        .network_revision
        .shape(&ids[0])
        .unwrap()
        .expect("accepted geometry")
}

#[tokio::test]
async fn commits_all_entities_with_persistent_linkage_and_duplicate_occurrences() {
    let (_ctx, ingestor, _) = ingestor().await;
    let first = ingestor.ingest(snapshot()).await.unwrap();
    let id = first.trips[0].id;
    let mut second = snapshot();
    let mut duplicate = second.trips[0].clone();
    duplicate.0.original_id = "$A 0808 101/103".into();
    duplicate.0.id =
        RunId::mta_subway(&duplicate.0.original_id, duplicate.0.partition_date).unwrap();
    duplicate.0.updated_at += Duration::seconds(1);
    for visit in &mut duplicate.1 {
        visit.trip_id = duplicate.0.id;
    }
    second.trips.push(duplicate);
    let saved = ingestor.ingest(second).await.unwrap();
    assert_eq!(saved.trips.len(), 1);
    assert_eq!(saved.trips[0].id, id);
    assert_eq!(saved.trips[0].original_id, "$A 0808 101/103");
    assert_eq!(
        saved.trips[0].updated_at,
        snapshot_now() + Duration::seconds(1)
    );
    assert_eq!(saved.stop_times.len(), 2);
    assert!(saved.stop_times.iter().all(|stop| stop.trip_id == id));
    assert_eq!(saved.positions[0].trip_id, Some(id));
    assert_eq!(saved.network_revision.source, Source::MtaSubway);
}

#[tokio::test]
async fn unchanged_stops_replay_is_idempotent_and_one_arrival_changes_one_visit() {
    let (_ctx, ingestor, _) = ingestor().await;
    let first = ingestor.ingest(snapshot()).await.unwrap();
    let stop_one_before = first.stop_times[1].arrival;
    let replay = ingestor.ingest(snapshot()).await.unwrap();
    assert_eq!(replay.stop_times, first.stop_times);
    assert_eq!(replay.trips, first.trips);
    assert_eq!(replay.positions, first.positions);
    let mut changed = snapshot();
    let stop_zero_before = changed.trips[0].1[0].arrival;
    changed.trips[0].1[0].arrival += Duration::seconds(30);
    let saved = ingestor.ingest(changed).await.unwrap();
    assert_eq!(
        saved.stop_times[0].arrival,
        stop_zero_before + Duration::seconds(30)
    );
    assert_eq!(saved.stop_times[1].arrival, stop_one_before);
}

#[tokio::test]
async fn empty_shapes_preserve_and_nonempty_shapes_replace_stored_shape() {
    let (_ctx, ingestor, _) = ingestor().await;
    let initial = Geom::from(geo::LineString::from(vec![(0., 0.), (1., 1.)]));
    let replacement = Geom::from(geo::LineString::from(vec![(2., 2.), (3., 3.)]));
    let mut revision = NetworkRevision::from_dataset(&support::mta_subway_dataset());
    revision.shapes = backend::network::shape_index::ShapeIndex::from_prepared(
        std::collections::HashMap::from([
            ("initial".into(), initial.clone()),
            ("replacement".into(), replacement.clone()),
        ]),
    );
    let revision = std::sync::Arc::new(revision);
    let mut first = snapshot();
    first.network_revision = Some(revision.clone());
    first.trips[0].0.shape_ids = vec!["initial".into()];
    let accepted = ingestor.ingest(first).await.unwrap();
    assert_eq!(selected_shape(&accepted).as_ref(), &initial);
    let mut next = snapshot();
    next.network_revision = Some(revision.clone());
    let preserved = ingestor.ingest(next).await.unwrap();
    assert_eq!(selected_shape(&preserved).as_ref(), &initial);
    let mut next = snapshot();
    next.network_revision = Some(revision);
    next.trips[0].0.shape_ids = vec!["replacement".into()];
    let replaced = ingestor.ingest(next).await.unwrap();
    assert_eq!(selected_shape(&replaced).as_ref(), &replacement);
}

#[tokio::test]
async fn repeated_stop_visits_persist_and_publish_by_sequence() {
    let (_ctx, ingestor, live) = ingestor().await;
    let mut input = snapshot();
    let mut revisit = input.trips[0].1[0].clone();
    revisit.stop_sequence = 2;
    revisit.arrival += Duration::minutes(20);
    revisit.departure += Duration::minutes(20);
    input.trips[0].1.push(revisit);
    let saved = ingestor.ingest(input).await.unwrap();
    let base = snapshot_now() + Duration::minutes(5);
    let mut stored: Vec<(i32, String, chrono::DateTime<Utc>)> = saved
        .stop_times
        .iter()
        .map(|stop| (stop.stop_sequence, stop.stop_id.clone(), stop.arrival))
        .collect();
    stored.sort_by_key(|(seq, _, _)| *seq);
    assert_eq!(
        stored,
        [
            (0, "101".to_owned(), base),
            (1, "102".to_owned(), base),
            (2, "101".to_owned(), base + Duration::minutes(20)),
        ]
    );
    let published = live.get(Source::MtaSubway).unwrap();
    let mut sequences: Vec<(i32, &str)> = published
        .stop_times
        .iter()
        .map(|stop| (stop.stop_sequence, stop.stop_id.as_ref()))
        .collect();
    sequences.sort();
    assert_eq!(sequences, [(0, "101"), (1, "102"), (2, "101")]);
}

#[tokio::test]
async fn invalid_stop_rolls_back_entire_snapshot() {
    let (_ctx, ingestor, live) = ingestor().await;
    let mut broken = snapshot();
    broken.trips[0].1[1].stop_id = "missing-stop".into();
    assert!(ingestor.ingest(broken).await.is_err());
    assert!(live.get(Source::MtaSubway).is_none());
    let history = ingestor
        .history()
        .load(Source::MtaSubway, snapshot_now(), None)
        .await
        .unwrap();
    assert!(history.trips.is_empty() && history.stop_times.is_empty());
}

#[tokio::test]
async fn older_position_returns_and_publishes_stored_state() {
    let (_ctx, ingestor, live) = ingestor().await;
    let first = ingestor.ingest(snapshot()).await.unwrap();
    let mut stale = snapshot();
    stale.positions[0].updated_at -= Duration::minutes(1);
    stale.positions[0].stop_id = Some("102".into());
    let saved = ingestor.ingest(stale).await.unwrap();
    assert_eq!(saved.positions[0].updated_at, first.positions[0].updated_at);
    assert_eq!(saved.positions[0].stop_id.as_deref(), Some("101"));
    let current = live.get(Source::MtaSubway).unwrap();
    assert!(std::sync::Arc::ptr_eq(&saved, &current));
    let committed = &current.positions;
    assert_eq!(committed[0].stop_id.as_deref(), Some("101"));
    assert_eq!(committed[0].updated_at, first.positions[0].updated_at);
}

#[tokio::test]
async fn position_failure_rolls_back_existing_trip_and_stop_changes() {
    let (_ctx, ingestor, live) = ingestor().await;
    let first = ingestor.ingest(snapshot()).await.unwrap();
    let arrival_before = first.stop_times[0].arrival;
    let mut broken = snapshot();
    broken.trips[0].0.shape_ids = vec!["must-rollback".into()];
    broken.trips[0].1[0].arrival += Duration::minutes(10);
    broken.positions[0].stop_id = Some("missing-position-stop".into());
    broken.positions[0].updated_at += Duration::seconds(1);
    assert!(ingestor.ingest(broken).await.is_err());
    let current = live.get(Source::MtaSubway).unwrap();
    assert!(
        std::sync::Arc::ptr_eq(&first, &current),
        "rollback cannot replace live generation"
    );
    let committed = &current.trips;
    assert!(committed[0].shape_ids.is_empty());
    assert_eq!(current.stop_times[0].arrival, arrival_before);
    let retained = ingestor
        .history()
        .load(Source::MtaSubway, snapshot_now(), None)
        .await
        .unwrap();
    assert!(retained.trips[0].shape_ids.is_empty());
    assert_eq!(retained.stop_times[0].arrival, arrival_before);
}

#[tokio::test]
async fn resolves_shapes_by_membership_then_geometry_and_pins_revision() {
    let ctx = TestContext::new();
    let storage = ctx.storage();
    let stores = support::test_owners(storage);
    let dataset = support::mta_subway_dataset();
    let mut revision = NetworkRevision::from_dataset(&dataset);
    stores.network.persist(dataset).await.unwrap();
    std::sync::Arc::make_mut(&mut revision.routes)
        .get_mut("A")
        .unwrap()
        .shape_ids = vec!["far".into(), "near".into()];
    revision.shapes = backend::network::shape_index::ShapeIndex::from_prepared(
        std::collections::HashMap::from([
            (
                "far".into(),
                Geom::from(geo::LineString::from(vec![(10., 10.), (11., 11.)])),
            ),
            (
                "near".into(),
                Geom::from(geo::LineString::from(vec![(0., 0.), (1., 1.)])),
            ),
        ]),
    );
    for stop in ["101", "102"] {
        std::sync::Arc::make_mut(&mut revision.stops)
            .get_mut(stop)
            .unwrap()
            .geom = Geom::from(geo::Point::new(0., 0.));
    }
    std::sync::Arc::make_mut(&mut revision.route_stop_shapes)
        .insert(("A".into(), "101".into()), vec!["far".into()]);
    let index = stores.network.network_index();
    index.publish_bundle(vec![revision.clone()]);
    let ingestor = stores.ingestor;
    let first = ingestor.ingest(snapshot()).await.unwrap();
    let far = Geom::from(geo::LineString::from(vec![(10., 10.), (11., 11.)]));
    assert_eq!(
        selected_shape(&first).as_ref(),
        &far,
        "exact stop membership must beat closer geometry"
    );
    std::sync::Arc::make_mut(&mut revision.route_stop_shapes).clear();
    index.publish_bundle(vec![revision]);
    assert!(
        first
            .network_revision
            .route_stop_shapes
            .contains_key(&("A".into(), "101".into()))
    );
    let preserved = ingestor.ingest(snapshot()).await.unwrap();
    assert_eq!(selected_shape(&preserved).as_ref(), &far);
    let mut fallback = snapshot();
    set_occurrence(&mut fallback, Source::MtaSubway, "0A 0809 101/102");
    let saved = ingestor.ingest(fallback).await.unwrap();
    let near = Geom::from(geo::LineString::from(vec![(0., 0.), (1., 1.)]));
    assert_eq!(selected_shape(&saved).as_ref(), &near);
}

#[tokio::test]
async fn omitted_rows_remain_retained_but_are_absent_from_current_snapshot() {
    let (_ctx, ingestor, live) = ingestor().await;
    let first = ingestor.ingest(snapshot()).await.unwrap();
    let at = first.trips[0].created_at;
    let empty = ingestor
        .ingest(CollectedSnapshot {
            source: Source::MtaSubway,
            network_revision: None,
            renormalize: None,
            trips: vec![],
            positions: vec![],
        })
        .await
        .unwrap();
    assert!(empty.trips.is_empty() && empty.stop_times.is_empty() && empty.positions.is_empty());
    let current = live.get(Source::MtaSubway).unwrap();
    assert!(current.trips.is_empty());
    let retained = ingestor
        .history()
        .load(Source::MtaSubway, at + Duration::seconds(1), None)
        .await
        .unwrap();
    assert_eq!(retained.trips.len(), 1);
    assert_eq!(retained.stop_times.len(), 2);
}

#[tokio::test]
async fn correlation_mapping_survives_subsecond_precision() {
    let (_ctx, ingestor, _) = ingestor().await;
    let mut input = snapshot();
    input.trips[0].0.created_at += Duration::nanoseconds(123);
    let saved = ingestor.ingest(input).await.unwrap();
    assert_eq!(saved.positions[0].trip_id, Some(saved.trips[0].id));
    assert!(
        saved
            .stop_times
            .iter()
            .all(|stop| stop.trip_id == saved.trips[0].id)
    );
}

#[tokio::test]
async fn stop_time_shared_snapshot_matches_committed_microseconds_on_replay() {
    let (_ctx, ingestor, live) = ingestor().await;
    let mut input = snapshot();
    input.trips[0].1[0].arrival += Duration::nanoseconds(123_456_789);
    input.trips[0].1[0].departure += Duration::nanoseconds(987_654_321);
    let saved = ingestor.ingest(input.clone()).await.unwrap();
    let expected_arrival = snapshot_now() + Duration::minutes(5) + Duration::microseconds(123_456);
    let expected_departure =
        snapshot_now() + Duration::minutes(6) + Duration::microseconds(987_654);
    assert_eq!(saved.stop_times[0].arrival, expected_arrival);
    assert_eq!(saved.stop_times[0].departure, expected_departure);
    let arrival_before = saved.stop_times[0].arrival;
    let replay = ingestor.ingest(input).await.unwrap();
    assert_eq!(replay.stop_times[0].arrival, arrival_before);
    assert_eq!(replay.stop_times[0].departure, expected_departure);
    let current = live.get(Source::MtaSubway).unwrap();
    assert!(std::sync::Arc::ptr_eq(&replay, &current));
    let committed = &current.stop_times;
    assert_eq!(committed[0].arrival, expected_arrival);
    assert_eq!(committed[0].departure, expected_departure);
}

#[tokio::test]
async fn gps_samples_are_atomic_deduplicated_stationary_and_time_scoped() {
    let (_ctx, ingestor, live) = ingestor().await;
    let history = ingestor.history();
    let mut input = snapshot();
    let at = input.positions[0].updated_at;
    input.positions[0].geom = Some(Geom::from(geo::Point::new(-73.99, 40.75)));
    let first = ingestor.ingest(input.clone()).await.unwrap();
    let id = first.trips[0].id;
    ingestor.ingest(input.clone()).await.unwrap();
    let mut tied = input.clone();
    tied.positions[0].geom = Some(Geom::from(geo::Point::new(-74.0, 40.76)));
    let tied = ingestor.ingest(tied).await.unwrap();
    assert_eq!(
        tied.positions[0].geom.as_ref().map(|geom| &geom.0),
        first.positions[0].geom.as_ref().map(|geom| &geom.0)
    );
    input.positions[0].updated_at += Duration::seconds(60);
    ingestor.ingest(input.clone()).await.unwrap();
    let accepted = live.get(Source::MtaSubway).unwrap();
    let past = history
        .load_trajectories(Source::MtaSubway, at + Duration::seconds(30))
        .await
        .unwrap();
    assert_eq!(past.anchors[&id].observed_at, at);
    let later = history
        .load_trajectories(Source::MtaSubway, at + Duration::seconds(90))
        .await
        .unwrap();
    assert_eq!(later.anchors[&id].observed_at, at + Duration::seconds(60));
    let stale = history
        .load_trajectories(Source::MtaSubway, at + Duration::seconds(361))
        .await
        .unwrap();
    assert!(!stale.anchors.contains_key(&id));
    input.positions[0].updated_at = at + Duration::seconds(120);
    input.positions[0].stop_id = Some("missing-position-stop".into());
    assert!(ingestor.ingest(input).await.is_err());
    assert!(std::sync::Arc::ptr_eq(
        &accepted,
        &live.get(Source::MtaSubway).unwrap()
    ));
    let after_failure = history
        .load_trajectories(Source::MtaSubway, at + Duration::seconds(150))
        .await
        .unwrap();
    assert_eq!(
        after_failure.anchors[&id].observed_at,
        at + Duration::seconds(60)
    );
}

#[tokio::test]
async fn equal_time_oba_recovery_updates_status_without_replacing_gps() {
    let ctx = TestContext::new();
    let storage = ctx.storage();
    let owners = support::test_owners(storage);
    let dataset = mta_bus_dataset();
    let route_id = dataset.routes[0].id.clone();
    let stop_id = dataset.stops[0].id.clone();
    owners.network.persist(dataset).await.unwrap();
    let mut input = snapshot();
    set_occurrence(&mut input, Source::MtaBus, "0A 0808 101/102");
    input.trips[0].0.route_id = route_id;
    input.trips[0].0.data = TripData::MtaBus(MtaBusData { deviation: None });
    input.trips[0].1.truncate(1);
    input.trips[0].1[0].stop_id = stop_id;
    input.trips[0].1[0].data = StopTimeData::MtaBus;
    input.positions[0].stop_id = None;
    input.positions[0].geom = Some(Geom::from(geo::Point::new(-73.99, 40.75)));
    input.positions[0].data = PositionData::MtaBus(MtaBusPositionData {
        bearing: 0.0,
        passengers: None,
        capacity: None,
        status: None,
        phase: None,
    });
    let first = owners.ingestor.ingest(input.clone()).await.unwrap();
    let at = first.positions[0].updated_at;
    input.positions[0].data = PositionData::MtaBus(MtaBusPositionData {
        bearing: 0.0,
        passengers: Some(12),
        capacity: Some(60),
        status: Some("approaching".into()),
        phase: Some("in_progress".into()),
    });
    input.positions[0].geom = Some(Geom::from(geo::Point::new(-74.0, 40.76)));
    let recovered = owners.ingestor.ingest(input.clone()).await.unwrap();
    let published = owners.live_snapshots.get(Source::MtaBus).unwrap();
    assert!(std::sync::Arc::ptr_eq(&recovered, &published));
    let PositionData::MtaBus(data) = &published.positions[0].data else {
        panic!("expected bus status");
    };
    assert_eq!(data.passengers, Some(12));
    assert_eq!(data.capacity, Some(60));
    assert_eq!(data.status.as_deref(), Some("approaching"));
    assert_eq!(data.phase.as_deref(), Some("in_progress"));
    assert_eq!(published.positions[0].updated_at, at);
    assert_eq!(
        published.positions[0].geom.as_ref().map(|geom| &geom.0),
        first.positions[0].geom.as_ref().map(|geom| &geom.0)
    );
    let anchored = owners
        .ingestor
        .history()
        .load_trajectories(Source::MtaBus, at + Duration::seconds(1))
        .await
        .unwrap();
    let sample = anchored.anchors.get(&first.trips[0].id).unwrap();
    assert_eq!(sample.observed_at, at);
    assert_eq!(sample.point, geo::Point::new(-73.99, 40.75));
    let replay = owners.ingestor.ingest(input).await.unwrap();
    assert_eq!(replay.positions, recovered.positions);
}

#[tokio::test]
async fn equal_time_report_can_link_a_run_without_replacing_coordinates() {
    let (_ctx, ingestor, live) = ingestor().await;
    let mut input = snapshot();
    let run = input.positions[0].trip_id;
    input.positions[0].trip_id = None;
    input.positions[0].geom = Some(Geom::from(geo::Point::new(-73.99, 40.75)));
    let first = ingestor.ingest(input.clone()).await.unwrap();
    assert!(first.positions[0].trip_id.is_none());
    input.positions[0].trip_id = run;
    input.positions[0].geom = Some(Geom::from(geo::Point::new(-74.0, 40.76)));
    ingestor.ingest(input).await.unwrap();
    let published = live.get(Source::MtaSubway).unwrap();
    assert_eq!(published.positions[0].trip_id, Some(first.trips[0].id));
    assert_eq!(
        published.positions[0].geom.as_ref().map(|geom| &geom.0),
        first.positions[0].geom.as_ref().map(|geom| &geom.0)
    );
}

#[tokio::test]
async fn vehicle_reassignment_keeps_run_identity_and_samples_belong_to_runs() {
    let (_ctx, ingestor, _) = ingestor().await;
    let history = ingestor.history();
    let mut input = snapshot();
    let at = input.positions[0].updated_at;
    input.positions[0].geom = Some(Geom::from(geo::Point::new(-73.99, 40.75)));
    let first = ingestor.ingest(input.clone()).await.unwrap();
    input.trips[0].0.vehicle_id = "replacement".into();
    input.positions[0].vehicle_id = "replacement".into();
    input.positions[0].updated_at += Duration::seconds(1);
    let reassigned = ingestor.ingest(input.clone()).await.unwrap();
    assert_eq!(first.trips[0].id, reassigned.trips[0].id);
    assert_eq!(reassigned.trips[0].vehicle_id, "replacement");
    set_occurrence(&mut input, Source::MtaSubway, "0A 0809 101/102");
    input.positions[0].updated_at += Duration::seconds(1);
    let next = ingestor.ingest(input).await.unwrap();
    assert_ne!(first.trips[0].id, next.trips[0].id);
    assert_eq!(
        next.trips[0].id,
        RunId::mta_subway("0A 0809 101/102", eastern_service_date(at)).unwrap()
    );
    assert_eq!(next.positions[0].trip_id, Some(next.trips[0].id));
    let early = history
        .load_trajectories(Source::MtaSubway, at + Duration::seconds(2))
        .await
        .unwrap();
    assert_eq!(
        early.anchors[&first.trips[0].id].observed_at,
        at + Duration::seconds(1)
    );
    let reassigned_time = history
        .load_trajectories(Source::MtaSubway, at + Duration::seconds(3))
        .await
        .unwrap();
    assert!(reassigned_time.anchors.contains_key(&first.trips[0].id));
    let next_time = history
        .load_trajectories(Source::MtaSubway, at + Duration::seconds(5))
        .await
        .unwrap();
    assert_eq!(
        next_time
            .anchors
            .get(&next.trips[0].id)
            .map(|s| s.observed_at),
        Some(at + Duration::seconds(2))
    );
}

#[tokio::test]
async fn conflicting_visit_sequence_rejects_collection_without_publication() {
    let (_ctx, ingestor, live) = ingestor().await;
    let first = ingestor.ingest(snapshot()).await.unwrap();
    let mut input = snapshot();
    input.trips[0].1[1].stop_sequence = input.trips[0].1[0].stop_sequence;
    assert!(ingestor.ingest(input).await.is_err());
    assert!(std::sync::Arc::ptr_eq(
        &first,
        &live.get(Source::MtaSubway).unwrap()
    ));
}

#[tokio::test]
async fn removed_static_membership_preserves_runs_and_original_shape_assets() {
    let ctx = TestContext::new();
    let storage = ctx.storage();
    let stores = support::test_owners(storage);
    let mut dataset = support::mta_subway_dataset();
    stores.network.persist(dataset.clone()).await.unwrap();
    let mut input = snapshot();
    let shape = dataset.shapes[0].id.clone();
    input.trips[0].0.shape_ids = vec![shape.clone()];
    let first = stores.ingestor.ingest(input.clone()).await.unwrap();
    let asset_id = first.trips[0].shape_ids[0].clone();
    let before = stores
        .ingestor
        .history()
        .load_trajectories(Source::MtaSubway, first.trips[0].created_at)
        .await
        .unwrap();
    let asset_before = before
        .network
        .unwrap()
        .shapes
        .get(&asset_id)
        .unwrap()
        .unwrap();
    dataset.shapes[0].geom = Geom::from(geo::LineString::from(vec![(-73.8, 40.8), (-73.7, 40.9)]));
    stores.network.persist(dataset.clone()).await.unwrap();
    stores.ingestor.ingest(input).await.unwrap();
    dataset.routes.retain(|route| route.id != "A");
    dataset
        .stops
        .retain(|stop| !["101", "102"].contains(&stop.id.as_str()));
    dataset
        .route_stops
        .retain(|visit| visit.route_id != "A" && !["101", "102"].contains(&visit.stop_id.as_str()));
    dataset.shapes.retain(|candidate| candidate.id != shape);
    stores.network.persist(dataset).await.unwrap();
    assert!(
        !stores
            .network
            .network_index()
            .get(Source::MtaSubway)
            .unwrap()
            .routes
            .contains_key("A")
    );
    let retained = stores
        .ingestor
        .history()
        .load_trajectories(Source::MtaSubway, first.trips[0].created_at)
        .await
        .unwrap();
    assert_eq!(retained.service.trips[0].id, first.trips[0].id);
    assert_eq!(retained.service.stop_times.len(), 2);
    let shapes = retained.network.unwrap().shapes;
    assert_eq!(
        retained.service.trips[0].shape_ids.as_slice(),
        std::slice::from_ref(&asset_id)
    );
    assert_eq!(*shapes.get(&asset_id).unwrap().unwrap(), *asset_before);
}

#[tokio::test]
async fn unknown_replacement_shape_drops_previous_retained_geometry() {
    let ctx = TestContext::new();
    let storage = ctx.storage();
    let stores = support::test_owners(storage);
    let dataset = support::mta_subway_dataset();
    let shape_id = dataset.shapes[0].id.clone();
    stores.network.persist(dataset).await.unwrap();
    let mut input = snapshot();
    input.trips[0].0.shape_ids = vec![shape_id];
    let first = stores.ingestor.ingest(input.clone()).await.unwrap();
    let before = stores
        .ingestor
        .history()
        .load_trajectories(Source::MtaSubway, first.trips[0].created_at)
        .await
        .unwrap();
    let asset_id = first.trips[0].shape_ids[0].clone();
    assert!(
        before
            .network
            .unwrap()
            .shapes
            .get(&asset_id)
            .unwrap()
            .is_some()
    );
    input.trips[0].0.shape_ids = vec!["unknown-replacement".into()];
    let replacement = stores.ingestor.ingest(input).await.unwrap();
    assert!(replacement.trips[0].shape_ids.is_empty());
    let history = stores
        .ingestor
        .history()
        .load_trajectories(Source::MtaSubway, first.trips[0].created_at)
        .await
        .unwrap();
    assert_eq!(history.service.trips[0].id, first.trips[0].id);
    assert!(history.service.trips[0].shape_ids.is_empty());
    assert!(
        history
            .network
            .unwrap()
            .shapes
            .get(&asset_id)
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn cloned_ingestors_serialize_commits_and_publication_for_one_source() {
    let (_ctx, ingestor, live) = ingestor().await;
    let first = tokio::spawn({
        let ingestor = ingestor.clone();
        async move { ingestor.ingest(snapshot()).await }
    });
    let mut second_input = snapshot();
    set_occurrence(&mut second_input, Source::MtaSubway, "0A 0809 101/102");
    second_input.positions.clear();
    let second = tokio::spawn(async move { ingestor.ingest(second_input).await });
    let first = first.await.unwrap().unwrap();
    let second = second.await.unwrap().unwrap();
    assert!(first.committed_at <= second.committed_at);
    let published = live.get(Source::MtaSubway).unwrap();
    assert!(std::sync::Arc::ptr_eq(&published, &second));
    assert_eq!(published.trips[0].original_id, "0A 0809 101/102");
}

#[tokio::test]
async fn same_provider_run_identity_is_independent_across_sources() {
    let ctx = TestContext::new();
    let stores = ctx.owners();
    stores
        .network
        .persist(support::mta_subway_dataset())
        .await
        .unwrap();
    let subway = stores.ingestor.ingest(snapshot()).await.unwrap();
    let bus_dataset = mta_bus_dataset();
    let route_id = bus_dataset.routes[0].id.clone();
    stores.network.persist(bus_dataset).await.unwrap();
    let mut bus = snapshot();
    set_occurrence(&mut bus, Source::MtaBus, "0A 0808 101/102");
    bus.trips[0].0.route_id = route_id;
    bus.trips[0].0.data = TripData::MtaBus(MtaBusData { deviation: None });
    bus.trips[0].1.clear();
    bus.positions.clear();
    let bus = stores.ingestor.ingest(bus).await.unwrap();
    assert_ne!(subway.trips[0].id, bus.trips[0].id);
    assert_eq!(
        bus.trips[0].id,
        RunId::scheduled_bus(
            Source::MtaBus,
            &bus.trips[0].original_id,
            bus.trips[0].partition_date
        )
    );
    assert_eq!(
        subway.trips[0].id,
        RunId::mta_subway(&subway.trips[0].original_id, subway.trips[0].partition_date).unwrap()
    );
    assert_eq!(subway.trips[0].original_id, bus.trips[0].original_id);
    assert_eq!(subway.trips[0].created_at, bus.trips[0].created_at);
}

#[tokio::test]
async fn mixed_source_payloads_reject_before_writes_and_keep_previous_publication() {
    let (_ctx, ingestor, live) = ingestor().await;
    let committed = ingestor.ingest(snapshot()).await.unwrap();
    let mut mixed = snapshot();
    mixed.positions[0].data = PositionData::NjtBus(NjtBusPositionData {
        occupancy_status: OccupancyStatus::Empty,
    });
    let error = ingestor.ingest(mixed).await.err().unwrap();
    assert!(error.to_string().contains("Observation has another source"));
    assert!(std::sync::Arc::ptr_eq(
        &committed,
        &live.get(Source::MtaSubway).unwrap()
    ));
    let current = live.get(Source::MtaSubway).unwrap();
    assert!(matches!(
        current.positions[0].data,
        PositionData::MtaSubway(_)
    ));
}

#[tokio::test]
async fn reused_provider_shape_names_do_not_reinterpret_accepted_live_or_historical_runs() {
    let ctx = TestContext::new();
    let stores = support::test_owners(ctx.storage());
    let source = Source::MtaSubway;
    let mut dataset = support::mta_subway_dataset();
    let provider = dataset.shapes[0].id.clone();
    let original_geometry = dataset.shapes[0].geom.clone();
    stores.network.persist(dataset.clone()).await.unwrap();
    let mut original = snapshot();
    original.positions.clear();
    original.trips[0].0.shape_ids = vec![provider];
    let before = stores.ingestor.ingest(original.clone()).await.unwrap();
    let old_id = before.trips[0].id;
    let replacement_geometry =
        Geom::from(geo::LineString::from(vec![(-73.5, 40.5), (-73.4, 40.6)]));
    dataset.shapes[0].geom = replacement_geometry.clone();
    stores.network.persist(dataset).await.unwrap();
    let mut new_run = original.clone();
    set_occurrence(&mut new_run, source, "0A 0810 101/102");
    original.trips.extend(new_run.trips);
    let current = stores.ingestor.ingest(original).await.unwrap();
    assert_eq!(current.trips.len(), 2);
    let history = stores.ingestor.history();
    let retained = history
        .load_trajectories(source, snapshot_now())
        .await
        .unwrap();
    let passenger = history.load(source, snapshot_now(), None).await.unwrap();
    let geometry = retained.network.unwrap();
    for trip in &current.trips {
        let expected = if trip.id == old_id {
            &original_geometry
        } else {
            &replacement_geometry
        };
        assert_eq!(
            current
                .network_revision
                .shape(&trip.shape_ids[0])
                .unwrap()
                .unwrap()
                .as_ref(),
            expected
        );
        let historical = retained
            .service
            .trips
            .iter()
            .find(|run| run.id == trip.id)
            .unwrap();
        assert_eq!(
            geometry
                .shapes
                .get(&historical.shape_ids[0])
                .unwrap()
                .unwrap()
                .as_ref(),
            expected
        );
        assert_eq!(
            passenger
                .trips
                .iter()
                .find(|run| run.id == trip.id)
                .unwrap()
                .shape_ids,
            trip.shape_ids
        );
    }
}
