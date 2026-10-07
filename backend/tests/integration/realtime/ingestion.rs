use crate::support;

use backend::{
    models::{
        position::{MtaSubwayPositionData, PositionData, VehiclePosition},
        source::Source,
        trip::{MtaSubwayStopTimeData, MtaSubwayTripData, StopTime, StopTimeData, Trip, TripData},
    },
    realtime::{CollectedSnapshot, LiveSnapshots, RealtimeIngestor},
};
use chrono::{Duration, TimeZone, Utc};
use uuid::Uuid;

async fn ingestor(pool: &sqlx::PgPool) -> (RealtimeIngestor, LiveSnapshots) {
    let stores = support::test_stores(pool.clone());
    stores
        .static_data_store
        .persist(&support::mta_subway_dataset())
        .await
        .unwrap();
    (stores.ingestor, stores.live_snapshots)
}

fn snapshot() -> CollectedSnapshot {
    let now = Utc.timestamp_opt(1_780_000_000, 0).unwrap();
    let id = Uuid::now_v7();
    let trip = Trip {
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
        .map(|stop_id| StopTime {
            trip_id: id,
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
    let position = VehiclePosition {
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
    duplicate.0.id = Uuid::now_v7();
    duplicate.0.updated_at += Duration::seconds(1);
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
    assert_eq!(saved.static_revision.source, Source::MtaSubway);
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
    use backend::{models::geom::Geom, static_data::index::StaticTransitRevision};

    let stores = support::test_stores(pool.clone());
    let dataset = support::mta_subway_dataset();
    stores.static_data_store.persist(&dataset).await.unwrap();
    let mut revision = StaticTransitRevision::from_dataset(&dataset);
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
    let index = stores.static_data_store.static_index();
    index.publish(revision.clone());
    let ingestor = RealtimeIngestor::new(pool, stores.live_snapshots.clone(), index.clone());
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
        &first.static_revision,
        &index.get(Source::MtaSubway).unwrap()
    ));
    revision.route_stop_shapes.clear();
    index.publish(revision);
    assert!(!std::sync::Arc::ptr_eq(
        &first.static_revision,
        &index.get(Source::MtaSubway).unwrap()
    ));
    assert!(
        first
            .static_revision
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
