use crate::support;

use backend::operations::CollectedObservation as VehicleObservation;
use backend::operations::CollectedSnapshot;
use backend::operations::CollectedTrip as TripRun;
use backend::operations::CollectedVisit as StopVisit;
use backend::operations::LiveSnapshots;
use backend::operations::OperationsIngestor;
use backend::operations::types::position::MtaSubwayPositionData;
use backend::operations::types::position::PositionData;
use backend::operations::types::trip::MtaSubwayStopTimeData;
use backend::operations::types::trip::MtaSubwayTripData;
use backend::operations::types::trip::StopTimeData;
use backend::operations::types::trip::TripData;
use backend::source::Source;
use chrono::{Duration, TimeZone, Utc};
use uuid::Uuid;

async fn ingestor(pool: &sqlx::PgPool) -> (OperationsIngestor, LiveSnapshots) {
    let stores = support::test_owners(pool.clone());
    stores
        .network
        .persist(&support::mta_subway_dataset())
        .await
        .unwrap();
    (stores.ingestor, stores.live_snapshots)
}

fn snapshot() -> CollectedSnapshot {
    let now = Utc.timestamp_opt(1_780_000_000, 0).unwrap();
    let id: backend::operations::RunKey = Uuid::now_v7().into();
    let trip = TripRun {
        id,
        original_id: "ingestion-trip".into(),
        vehicle_id: "vehicle".into(),
        route_id: "A".into(),
        shape_ids: vec![],
        direction: 1,
        created_at: now,
        updated_at: now,
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
        trip_id: Some(id),
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

async fn versions(pool: &sqlx::PgPool, trip_id: Uuid) -> Vec<(String, String)> {
    sqlx::query_as::<_, (String, String)>(
        "SELECT stop_id, xmin::text FROM realtime.stop_time WHERE trip_id = $1 ORDER BY stop_id",
    )
    .bind(trip_id)
    .fetch_all(pool)
    .await
    .unwrap()
}

#[sqlx::test]
async fn commits_all_entities_with_persistent_linkage_and_all_duplicate_aliases(
    pool: sqlx::PgPool,
) {
    let (ingestor, _) = ingestor(&pool).await;
    let first = ingestor.ingest(snapshot()).await.unwrap();
    let id = first.trips[0].id;
    let mut second = snapshot();
    let mut duplicate = second.trips[0].clone();
    duplicate.0.id = Uuid::now_v7().into();
    duplicate.0.updated_at += Duration::seconds(1);
    for visit in &mut duplicate.1 {
        visit.trip_id = duplicate.0.id;
    }
    // Position refers to the discarded older alias, not the deduplicated winner.
    second.trips.push(duplicate);
    let saved = ingestor.ingest(second).await.unwrap();
    assert_eq!(saved.trips.len(), 1);
    assert_eq!(saved.trips[0].id, id);
    assert_eq!(saved.stop_times.len(), 2);
    assert!(saved.stop_times.iter().all(|stop| stop.trip_id == id));
    assert_eq!(saved.positions[0].trip_id, Some(id));
    let linked: (Uuid,) =
        sqlx::query_as("SELECT trip_id FROM realtime.vehicle_position WHERE vehicle_id='vehicle'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(linked.0, id);
    assert_eq!(saved.network_revision.source, Source::MtaSubway);
}

#[sqlx::test]
async fn unchanged_stops_keep_tuple_versions_and_one_arrival_changes_one_row(pool: sqlx::PgPool) {
    let (ingestor, _) = ingestor(&pool).await;
    let first = ingestor.ingest(snapshot()).await.unwrap();
    let before = versions(&pool, first.trips[0].id).await;
    let replay = ingestor.ingest(snapshot()).await.unwrap();
    assert_eq!(versions(&pool, first.trips[0].id).await, before);
    assert_eq!(replay.changes.stop_times, 0);
    assert_eq!(replay.changes.trips, 0);
    assert_eq!(replay.changes.positions, 0);
    assert!(replay.changed_trip_ids.is_empty());
    let mut changed = snapshot();
    changed.trips[0].1[0].arrival += Duration::seconds(30);
    let saved = ingestor.ingest(changed).await.unwrap();
    let after = versions(&pool, first.trips[0].id).await;
    assert_ne!(after[0].1, before[0].1);
    assert_eq!(after[1].1, before[1].1);
    assert_eq!(saved.changes.stop_times, 1);
    assert!(saved.changed_trip_ids.contains(&first.trips[0].id));
}

#[sqlx::test]
async fn empty_shapes_preserve_and_nonempty_shapes_replace_stored_shape(pool: sqlx::PgPool) {
    let (ingestor, _) = ingestor(&pool).await;
    let mut first = snapshot();
    first.trips[0].0.shape_ids = vec!["initial".into()];
    ingestor.ingest(first).await.unwrap();
    let preserved = ingestor.ingest(snapshot()).await.unwrap();
    assert_eq!(
        preserved.trips[0]
            .shape_ids
            .iter()
            .map(AsRef::as_ref)
            .collect::<Vec<&str>>(),
        ["initial"]
    );
    let mut replacement = snapshot();
    replacement.trips[0].0.shape_ids = vec!["replacement".into()];
    let replaced = ingestor.ingest(replacement).await.unwrap();
    assert_eq!(
        replaced.trips[0]
            .shape_ids
            .iter()
            .map(AsRef::as_ref)
            .collect::<Vec<&str>>(),
        ["replacement"]
    );
    let stored: (Vec<String>,) = sqlx::query_as("SELECT shape_ids FROM realtime.trip")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(stored.0, ["replacement"]);
}

#[sqlx::test]
async fn repeated_stop_visits_persist_and_publish_by_sequence(pool: sqlx::PgPool) {
    let (ingestor, live) = ingestor(&pool).await;
    let mut input = snapshot();
    let mut revisit = input.trips[0].1[0].clone();
    revisit.stop_sequence = 2;
    revisit.arrival += Duration::minutes(20);
    revisit.departure += Duration::minutes(20);
    input.trips[0].1.push(revisit);
    let saved = ingestor.ingest(input).await.unwrap();
    let stored: Vec<(i32, String, chrono::DateTime<Utc>)> = sqlx::query_as(
        "SELECT stop_sequence, stop_id, arrival FROM realtime.stop_time WHERE trip_id = $1 ORDER BY stop_sequence",
    )
    .bind(saved.trips[0].id)
    .fetch_all(&pool)
    .await
    .unwrap();
    let base = Utc.timestamp_opt(1_780_000_300, 0).unwrap();
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

#[sqlx::test]
async fn invalid_stop_rolls_back_entire_snapshot(pool: sqlx::PgPool) {
    let (ingestor, _) = ingestor(&pool).await;
    let mut broken = snapshot();
    broken.trips[0].1[1].stop_id = "missing-stop".into();
    assert!(ingestor.ingest(broken).await.is_err());
    let counts: (i64, i64, i64) = sqlx::query_as("SELECT (SELECT count(*) FROM realtime.trip), (SELECT count(*) FROM realtime.stop_time), (SELECT count(*) FROM realtime.vehicle_position)").fetch_one(&pool).await.unwrap();
    assert_eq!(counts, (0, 0, 0));
}

#[sqlx::test]
async fn older_position_returns_and_publishes_stored_state(pool: sqlx::PgPool) {
    let (ingestor, live) = ingestor(&pool).await;
    let first = ingestor.ingest(snapshot()).await.unwrap();
    let mut stale = snapshot();
    stale.positions[0].updated_at -= Duration::minutes(1);
    stale.positions[0].stop_id = Some("102".into());
    let saved = ingestor.ingest(stale).await.unwrap();
    assert_eq!(saved.changes.positions, 0);
    assert_eq!(saved.positions[0].updated_at, first.positions[0].updated_at);
    assert_eq!(saved.positions[0].stop_id.as_deref(), Some("101"));
    let current = live.get(Source::MtaSubway).unwrap();
    assert!(std::sync::Arc::ptr_eq(&saved, &current));
    let committed = &current.positions;
    assert_eq!(committed[0].stop_id.as_deref(), Some("101"));
    assert_eq!(committed[0].updated_at, first.positions[0].updated_at);
}

#[sqlx::test]
async fn position_failure_rolls_back_existing_trip_and_stop_changes(pool: sqlx::PgPool) {
    let (ingestor, live) = ingestor(&pool).await;
    let first = ingestor.ingest(snapshot()).await.unwrap();
    let before = versions(&pool, first.trips[0].id).await;
    let mut broken = snapshot();
    broken.trips[0].0.shape_ids = vec!["must-rollback".into()];
    broken.trips[0].1[0].arrival += Duration::minutes(10);
    broken.positions[0].stop_id = Some("missing-position-stop".into());
    broken.positions[0].updated_at += Duration::seconds(1);
    assert!(ingestor.ingest(broken).await.is_err());
    assert_eq!(versions(&pool, first.trips[0].id).await, before);
    let shapes: (Vec<String>,) = sqlx::query_as("SELECT shape_ids FROM realtime.trip")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(shapes.0.is_empty());
    let current = live.get(Source::MtaSubway).unwrap();
    assert!(
        std::sync::Arc::ptr_eq(&first, &current),
        "rollback cannot replace live generation"
    );
    let committed = &current.trips;
    assert!(committed[0].shape_ids.is_empty());
}

#[sqlx::test]
async fn resolves_shapes_by_membership_then_geometry_and_pins_revision(pool: sqlx::PgPool) {
    use backend::geometry::Geom;
    use backend::network::index::NetworkRevision;

    let stores = support::test_owners(pool.clone());
    let dataset = support::mta_subway_dataset();
    stores.network.persist(&dataset).await.unwrap();
    let mut revision = NetworkRevision::from_dataset(&dataset);
    revision.routes.get_mut("A").unwrap().shape_ids = vec!["far".into(), "near".into()];
    revision.shapes.insert(
        "far".into(),
        Geom::from(geo::LineString::from(vec![(10., 10.), (11., 11.)])),
    );
    revision.shapes.insert(
        "near".into(),
        Geom::from(geo::LineString::from(vec![(0., 0.), (1., 1.)])),
    );
    for stop in ["101", "102"] {
        revision.stops.get_mut(stop).unwrap().geom = Geom::from(geo::Point::new(0., 0.));
    }
    revision
        .route_stop_shapes
        .insert(("A".into(), "101".into()), vec!["far".into()]);
    let index = stores.network.network_index();
    index.publish(revision.clone());
    let ingestor = OperationsIngestor::new(pool, stores.live_snapshots.clone(), index.clone());
    let first = ingestor.ingest(snapshot()).await.unwrap();
    assert_eq!(
        first.trips[0]
            .shape_ids
            .iter()
            .map(AsRef::as_ref)
            .collect::<Vec<&str>>(),
        ["far"],
        "exact stop membership must beat closer geometry"
    );
    assert_eq!(first.changes.resolved_shapes, 1);
    assert!(std::sync::Arc::ptr_eq(
        &first.network_revision,
        &index.get(Source::MtaSubway).unwrap()
    ));
    revision.route_stop_shapes.clear();
    index.publish(revision);
    assert!(!std::sync::Arc::ptr_eq(
        &first.network_revision,
        &index.get(Source::MtaSubway).unwrap()
    ));
    assert!(
        first
            .network_revision
            .route_stop_shapes
            .contains_key(&("A".into(), "101".into()))
    );
    let preserved = ingestor.ingest(snapshot()).await.unwrap();
    assert_eq!(
        preserved.trips[0]
            .shape_ids
            .iter()
            .map(AsRef::as_ref)
            .collect::<Vec<&str>>(),
        ["far"]
    );
    assert_eq!(preserved.changes.resolved_shapes, 0);
    let mut fallback = snapshot();
    fallback.trips[0].0.original_id = "geometric-fallback".into();
    let saved = ingestor.ingest(fallback).await.unwrap();
    assert_eq!(
        saved.trips[0]
            .shape_ids
            .iter()
            .map(AsRef::as_ref)
            .collect::<Vec<&str>>(),
        ["near"]
    );
}

#[sqlx::test]
async fn omitted_rows_remain_retained_but_are_absent_from_current_snapshot(pool: sqlx::PgPool) {
    let (ingestor, _) = ingestor(&pool).await;
    ingestor.ingest(snapshot()).await.unwrap();
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
    let counts: (i64, i64, i64) = sqlx::query_as("SELECT (SELECT count(*) FROM realtime.trip), (SELECT count(*) FROM realtime.stop_time), (SELECT count(*) FROM realtime.vehicle_position)").fetch_one(&pool).await.unwrap();
    assert_eq!(counts, (1, 2, 1));
}

#[sqlx::test]
async fn correlation_mapping_survives_postgres_timestamp_precision(pool: sqlx::PgPool) {
    let (ingestor, _) = ingestor(&pool).await;
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

#[sqlx::test]
async fn stop_time_shared_snapshot_matches_committed_microseconds_on_replay(pool: sqlx::PgPool) {
    let (ingestor, live) = ingestor(&pool).await;
    let mut input = snapshot();
    input.trips[0].1[0].arrival += Duration::nanoseconds(123_456_789);
    input.trips[0].1[0].departure += Duration::nanoseconds(987_654_321);
    let saved = ingestor.ingest(input.clone()).await.unwrap();
    let expected_arrival = Utc.timestamp_opt(1_780_000_300, 123_456_000).unwrap();
    let expected_departure = Utc.timestamp_opt(1_780_000_360, 987_654_000).unwrap();
    let stored: (chrono::DateTime<Utc>, chrono::DateTime<Utc>) = sqlx::query_as(
        "SELECT arrival, departure FROM realtime.stop_time WHERE trip_id = $1 AND stop_id = '101'",
    )
    .bind(saved.trips[0].id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(stored, (expected_arrival, expected_departure));
    assert_eq!(saved.stop_times[0].arrival, stored.0);
    assert_eq!(saved.stop_times[0].departure, stored.1);
    let before = versions(&pool, saved.trips[0].id).await;
    let replay = ingestor.ingest(input).await.unwrap();
    assert_eq!(replay.changes.stop_times, 0);
    assert_eq!(versions(&pool, saved.trips[0].id).await, before);
    assert_eq!(replay.stop_times[0].arrival, stored.0);
    assert_eq!(replay.stop_times[0].departure, stored.1);
    let current = live.get(Source::MtaSubway).unwrap();
    assert!(std::sync::Arc::ptr_eq(&replay, &current));
    let committed = &current.stop_times;
    assert_eq!(committed[0].arrival, stored.0);
    assert_eq!(committed[0].departure, stored.1);
}

#[sqlx::test]
async fn gps_samples_are_atomic_deduplicated_stationary_and_time_scoped(pool: sqlx::PgPool) {
    use backend::{geometry::Geom, operations::history::History};
    let (ingestor, live) = ingestor(&pool).await;
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
    input.positions[0].updated_at -= Duration::seconds(90);
    input.positions[0].geom = Some(Geom::from(geo::Point::new(-74.0, 40.76)));
    ingestor.ingest(input.clone()).await.unwrap();
    let samples: Vec<(chrono::DateTime<Utc>, f64)> = sqlx::query_as("SELECT observed_at, longitude FROM realtime.gps_sample WHERE run_id = $1 ORDER BY observed_at")
        .bind(id).fetch_all(&pool).await.unwrap();
    assert_eq!(
        samples,
        [(at, -73.99), (at + Duration::seconds(60), -73.99)]
    );
    let history = History::new(pool.clone());
    let past = history
        .load(Source::MtaSubway, at + Duration::seconds(30), None)
        .await
        .unwrap();
    assert_eq!(past.anchors[&id].observed_at, at);
    let stale = history
        .load(Source::MtaSubway, at + Duration::seconds(361), None)
        .await
        .unwrap();
    assert!(!stale.anchors.contains_key(&id));
    // A failure after the GPS write must roll back both state and the sample.
    sqlx::raw_sql("CREATE FUNCTION reject_run_shape() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'reject shape'; END $$; CREATE TRIGGER reject_run_shape BEFORE INSERT ON realtime.run_shape FOR EACH ROW EXECUTE FUNCTION reject_run_shape();").execute(&pool).await.unwrap();
    input.positions[0].updated_at = at + Duration::seconds(120);
    input.trips[0].0.shape_ids = vec![
        accepted
            .network_revision
            .shapes
            .keys()
            .next()
            .unwrap()
            .clone(),
    ];
    assert!(ingestor.ingest(input).await.is_err());
    let count: (i64,) = sqlx::query_as("SELECT count(*) FROM realtime.gps_sample")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count.0, 2);
    sqlx::query("DELETE FROM realtime.trip WHERE id = $1")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
    let count: (i64,) = sqlx::query_as("SELECT count(*) FROM realtime.gps_sample")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count.0, 0);
}

#[sqlx::test]
async fn vehicle_reassignment_keeps_run_identity_and_samples_belong_to_runs(pool: sqlx::PgPool) {
    let (ingestor, _) = ingestor(&pool).await;
    let mut input = snapshot();
    input.positions[0].geom = Some(geo::Point::new(-73.99, 40.75).into());
    let first = ingestor.ingest(input.clone()).await.unwrap();
    input.trips[0].0.vehicle_id = "replacement".into();
    input.positions[0].vehicle_id = "replacement".into();
    input.positions[0].updated_at += Duration::seconds(1);
    let reassigned = ingestor.ingest(input.clone()).await.unwrap();
    assert_eq!(first.trips[0].id, reassigned.trips[0].id);
    assert_eq!(reassigned.trips[0].vehicle_id, "replacement");
    input.trips[0].0.original_id = "next-run".into();
    input.positions[0].updated_at += Duration::seconds(1);
    let next = ingestor.ingest(input).await.unwrap();
    assert_ne!(first.trips[0].id, next.trips[0].id);
    let counts: Vec<(Uuid, i64)> = sqlx::query_as(
        "SELECT run_id, count(*) FROM realtime.gps_sample GROUP BY run_id ORDER BY count(*)",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(counts, [(next.trips[0].id, 1), (first.trips[0].id, 2)]);
}

#[sqlx::test]
async fn conflicting_visit_sequence_rejects_collection_without_publication(pool: sqlx::PgPool) {
    let (ingestor, live) = ingestor(&pool).await;
    let first = ingestor.ingest(snapshot()).await.unwrap();
    let mut input = snapshot();
    input.trips[0].1[1].stop_sequence = input.trips[0].1[0].stop_sequence;
    assert!(ingestor.ingest(input).await.is_err());
    assert!(std::sync::Arc::ptr_eq(
        &first,
        &live.get(Source::MtaSubway).unwrap()
    ));
}

#[sqlx::test]
async fn removed_static_membership_preserves_runs_and_original_shape_assets(pool: sqlx::PgPool) {
    use backend::operations::history::History;
    let stores = support::test_owners(pool.clone());
    let mut dataset = support::mta_subway_dataset();
    stores.network.persist(&dataset).await.unwrap();
    let mut input = snapshot();
    let shape = dataset.shapes[0].id.clone();
    input.trips[0].0.shape_ids = vec![shape.clone()];
    let first = stores.ingestor.ingest(input.clone()).await.unwrap();
    let asset: (String,) =
        sqlx::query_as("SELECT asset_id FROM realtime.run_shape WHERE run_id = $1")
            .bind(first.trips[0].id)
            .fetch_one(&pool)
            .await
            .unwrap();
    dataset.shapes[0].geom = geo::LineString::from(vec![(-73.8, 40.8), (-73.7, 40.9)]).into();
    stores.network.persist(&dataset).await.unwrap();
    stores.ingestor.ingest(input).await.unwrap();
    let retained_asset: (String,) =
        sqlx::query_as("SELECT asset_id FROM realtime.run_shape WHERE run_id = $1")
            .bind(first.trips[0].id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(retained_asset, asset);
    dataset.routes.retain(|route| route.id != "A");
    dataset
        .stops
        .retain(|stop| !["101", "102"].contains(&stop.id.as_str()));
    dataset
        .route_stops
        .retain(|visit| visit.route_id != "A" && !["101", "102"].contains(&visit.stop_id.as_str()));
    dataset.shapes.retain(|candidate| candidate.id != shape);
    stores.network.persist(&dataset).await.unwrap();
    assert!(
        !stores
            .network
            .network_index()
            .get(Source::MtaSubway)
            .unwrap()
            .routes
            .contains_key("A")
    );
    let retained = History::new(pool.clone())
        .load(Source::MtaSubway, first.trips[0].created_at, None)
        .await
        .unwrap();
    assert_eq!(retained.trips[0].id, first.trips[0].id);
    assert_eq!(retained.stop_times.len(), 2);
    assert!(
        retained
            .network
            .unwrap()
            .shapes
            .keys()
            .any(|id| id.ends_with(&asset.0))
    );
}

#[sqlx::test]
async fn unknown_replacement_shape_drops_previous_retained_geometry(pool: sqlx::PgPool) {
    let stores = support::test_owners(pool.clone());
    let dataset = support::mta_subway_dataset();
    stores.network.persist(&dataset).await.unwrap();
    let mut input = snapshot();
    input.trips[0].0.shape_ids = vec![dataset.shapes[0].id.clone()];
    let first = stores.ingestor.ingest(input.clone()).await.unwrap();
    let before: (i64,) =
        sqlx::query_as("SELECT count(*) FROM realtime.run_shape WHERE run_id = $1")
            .bind(first.trips[0].id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(before.0, 1);

    input.trips[0].0.shape_ids = vec!["unknown-replacement".into()];
    let replacement = stores.ingestor.ingest(input).await.unwrap();
    assert_eq!(replacement.trips[0].shape_ids, ["unknown-replacement"]);
    let history = backend::operations::history::History::new(pool)
        .load(Source::MtaSubway, first.trips[0].created_at, None)
        .await
        .unwrap();
    assert_eq!(history.trips[0].id, first.trips[0].id);
    assert!(history.trips[0].shape_ids.is_empty());
    assert!(history.network.unwrap().shapes.is_empty());
}

#[sqlx::test]
async fn cloned_ingestors_serialize_commits_and_publication_for_one_source(pool: sqlx::PgPool) {
    let (ingestor, live) = ingestor(&pool).await;
    sqlx::raw_sql("CREATE FUNCTION wait_first_run() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.original_id = 'ingestion-trip' THEN PERFORM pg_advisory_xact_lock(928732); END IF; RETURN NEW; END $$; CREATE TRIGGER wait_first_run BEFORE INSERT ON realtime.trip FOR EACH ROW EXECUTE FUNCTION wait_first_run();")
        .execute(&pool).await.unwrap();
    let mut blocker = pool.acquire().await.unwrap();
    sqlx::query("SELECT pg_advisory_lock(928732)")
        .execute(&mut *blocker)
        .await
        .unwrap();
    let first = tokio::spawn({
        let ingestor = ingestor.clone();
        async move { ingestor.ingest(snapshot()).await }
    });
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            let waiting: (bool,) = sqlx::query_as("SELECT EXISTS (SELECT 1 FROM pg_locks WHERE locktype = 'advisory' AND objid = 928732 AND NOT granted AND database = (SELECT oid FROM pg_database WHERE datname = current_database()))")
                .fetch_one(&pool).await.unwrap();
            if waiting.0 { break; }
            tokio::task::yield_now().await;
        }
    }).await.unwrap();
    let mut second_input = snapshot();
    second_input.trips[0].0.original_id = "second-run".into();
    second_input.positions.clear();
    let mut second = tokio::spawn(async move { ingestor.ingest(second_input).await });
    let premature = tokio::time::timeout(std::time::Duration::from_millis(200), &mut second).await;
    sqlx::query("SELECT pg_advisory_unlock(928732)")
        .execute(&mut *blocker)
        .await
        .unwrap();
    let first = first.await.unwrap().unwrap();
    assert!(
        premature.is_err(),
        "the second source commit must wait for the first"
    );
    let second = second.await.unwrap().unwrap();
    assert!(first.committed_at <= second.committed_at);
    let published = live.get(Source::MtaSubway).unwrap();
    assert!(std::sync::Arc::ptr_eq(&published, &second));
    assert_eq!(published.trips[0].original_id, "second-run");
}

#[sqlx::test]
async fn same_provider_run_identity_is_independent_across_sources(pool: sqlx::PgPool) {
    let stores = support::test_owners(pool);
    stores
        .network
        .persist(&support::mta_subway_dataset())
        .await
        .unwrap();
    let subway = stores.ingestor.ingest(snapshot()).await.unwrap();
    let bus_dataset = support::mta_bus_dataset();
    stores.network.persist(&bus_dataset).await.unwrap();
    let mut bus = snapshot();
    bus.source = Source::MtaBus;
    bus.trips[0].0.route_id = bus_dataset.routes[0].id.clone();
    bus.trips[0].0.data =
        TripData::MtaBus(backend::operations::types::trip::MtaBusData { deviation: None });
    bus.trips[0].1.clear();
    bus.positions.clear();
    let bus = stores.ingestor.ingest(bus).await.unwrap();
    assert_ne!(subway.trips[0].id, bus.trips[0].id);
    assert_eq!(subway.trips[0].original_id, bus.trips[0].original_id);
    assert_eq!(subway.trips[0].created_at, bus.trips[0].created_at);
}

#[sqlx::test]
async fn mixed_source_payloads_reject_before_writes_and_keep_previous_publication(
    pool: sqlx::PgPool,
) {
    let (ingestor, live) = ingestor(&pool).await;
    let committed = ingestor.ingest(snapshot()).await.unwrap();
    let mut mixed = snapshot();
    mixed.positions[0].data =
        PositionData::NjtBus(backend::operations::types::position::NjtBusPositionData {
            occupancy_status: backend::operations::types::position::OccupancyStatus::Empty,
        });
    let error = ingestor.ingest(mixed).await.err().unwrap();
    assert!(error.to_string().contains("Observation has another source"));
    assert!(std::sync::Arc::ptr_eq(
        &committed,
        &live.get(Source::MtaSubway).unwrap()
    ));
    let sources: Vec<Source> = sqlx::query_scalar("SELECT source FROM realtime.vehicle_position")
        .fetch_all(&pool)
        .await
        .unwrap();
    assert_eq!(sources, vec![Source::MtaSubway]);
}
