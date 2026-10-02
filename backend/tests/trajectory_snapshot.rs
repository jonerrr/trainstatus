mod common;
use backend::{
    models::{
        geom::Geom,
        position::{MtaBusPositionData, PositionData, VehiclePosition},
        source::Source,
        trip::{MtaBusData, StopTime, StopTimeData, Trip, TripData},
    },
    realtime::{IngestionChanges, PersistedSnapshot, TrajectoryDeriver},
    static_index::StaticTransitRevision,
    trajectory::{TrajectoryCache, TrajectoryEngine},
};
use chrono::{Duration, Utc};
use std::{collections::HashSet, sync::Arc};

fn fixture() -> PersistedSnapshot {
    let now = Utc::now();
    let id = uuid::Uuid::now_v7();
    let mut revision = StaticTransitRevision::from_dataset(&common::mta_bus_dataset());
    let stop_ids = ["504409", "901701"];
    for (i, stop) in stop_ids.iter().enumerate() {
        revision.stops.get_mut(*stop).unwrap().geom =
            Geom::from(geo::Point::new(-74.0 + i as f64 * 0.01, 40.7));
    }
    revision.shapes.insert(
        "committed-shape".into(),
        Geom::from(geo::LineString::from(vec![(-74.0, 40.7), (-73.99, 40.7)])),
    );
    PersistedSnapshot {
        source: Source::MtaBus,
        trips: vec![Trip {
            id,
            original_id: "test".into(),
            vehicle_id: "bus".into(),
            route_id: "B94".into(),
            shape_ids: vec!["committed-shape".into()],
            direction: 0,
            created_at: now,
            updated_at: now,
            data: TripData::MtaBus(MtaBusData { deviation: None }),
        }],
        stop_times: stop_ids
            .iter()
            .enumerate()
            .map(|(i, stop)| StopTime {
                trip_id: id,
                stop_id: (*stop).into(),
                arrival: now + Duration::seconds(i as i64 * 180),
                departure: now + Duration::seconds(i as i64 * 180),
                data: StopTimeData::MtaBus,
            })
            .collect(),
        positions: vec![VehiclePosition {
            vehicle_id: "bus".into(),
            trip_id: Some(id),
            stop_id: None,
            updated_at: now,
            geom: Some(Geom::from(geo::Point::new(-74.0, 40.7))),
            data: PositionData::MtaBus(MtaBusPositionData {
                bearing: 0.0,
                passengers: Some(12),
                capacity: None,
                status: None,
                phase: None,
            }),
        }],
        changed_trip_ids: HashSet::from([id]),
        changes: IngestionChanges::default(),
        static_revision: Arc::new(revision),
    }
}

#[tokio::test]
async fn trajectory_from_persisted_snapshot() {
    let snapshot = fixture();
    let id = snapshot.trips[0].id;
    let deriver = TrajectoryDeriver::new(
        Arc::new(TrajectoryEngine::new()),
        Arc::new(TrajectoryCache::new()),
    );
    let hot = deriver
        .derive(snapshot)
        .await
        .expect("derive committed snapshot without a DB");
    assert_eq!(hot.render_units.len(), 1);
    let unit = hot.render_units.values().next().unwrap();
    assert_eq!(unit.trip_id, id.to_string());
    assert_eq!(unit.passengers, Some(12));
    assert!(unit.positions.windows(2).any(|p| p[0] != p[1]));
    assert!(hot.prev_states.contains_key(&id));
}

#[tokio::test]
async fn trajectory_uses_pinned_stop_geometry_and_color() {
    use backend::trajectory::snapshot_from_persisted_trip;
    let snapshot = fixture();
    let cache = TrajectoryCache::new();
    let first = snapshot_from_persisted_trip(
        &snapshot.trips[0],
        snapshot.stop_times.clone(),
        vec![],
        &snapshot.static_revision,
        &cache,
        Utc::now(),
    )
    .await
    .unwrap();
    let mut replacement = (*snapshot.static_revision).clone();
    replacement.routes.get_mut("B94").unwrap().color = "ABCDEF".into();
    replacement.stops.get_mut("504409").unwrap().geom = Geom::from(geo::Point::new(-73.995, 40.7));
    let second = snapshot_from_persisted_trip(
        &snapshot.trips[0],
        snapshot.stop_times.clone(),
        vec![],
        &replacement,
        &cache,
        Utc::now(),
    )
    .await
    .unwrap();
    assert!(first.stops[0].stop_distance_m < 1.0);
    assert!(
        second.stops[0].stop_distance_m > 300.0,
        "stop projection must not reuse another revision's stop coordinate"
    );
    assert_eq!(second.route_color, "ABCDEF");
    assert!(second.stops[1].stop_distance_m <= second.shape_length_m);
}

#[tokio::test]
async fn trajectory_bad_trip_isolated_and_empty_snapshot_clears_source() {
    let mut snapshot = fixture();
    let valid_id = snapshot.trips[0].id;
    let mut malformed = snapshot.trips[0].clone();
    malformed.id = uuid::Uuid::now_v7();
    malformed.shape_ids = vec!["nonfinite".into()];
    let mut revision = (*snapshot.static_revision).clone();
    revision.shapes.insert(
        "nonfinite".into(),
        Geom::from(geo::LineString::from(vec![(f64::NAN, 40.7), (-73.9, 40.7)])),
    );
    snapshot.static_revision = Arc::new(revision);
    snapshot.trips.push(malformed);
    let cache = Arc::new(TrajectoryCache::new());
    let deriver = TrajectoryDeriver::new(Arc::new(TrajectoryEngine::new()), cache.clone());
    let hot = deriver.derive(snapshot).await.unwrap();
    assert_eq!(hot.render_units.len(), 1);
    assert!(hot.prev_states.contains_key(&valid_id));
    cache.set_hot(Source::MtaBus, hot).await;
    let mut empty = fixture();
    empty.trips.clear();
    empty.stop_times.clear();
    empty.positions.clear();
    let cleared = deriver.derive(empty).await.unwrap();
    assert!(cleared.render_units.is_empty() && cleared.prev_states.is_empty());
}

#[tokio::test]
async fn trajectory_preserves_previous_hot_state_continuity() {
    use backend::trajectory::{HotSnapshot, TrajectoryState};
    let mut snapshot = fixture();
    let id = snapshot.trips[0].id;
    snapshot.positions.clear();
    for stop in &mut snapshot.stop_times {
        stop.arrival += Duration::seconds(60);
        stop.departure += Duration::seconds(60);
    }
    let cache = Arc::new(TrajectoryCache::new());
    let mut previous = HotSnapshot::empty();
    previous.prev_states.insert(
        id,
        TrajectoryState {
            t_unix: Utc::now().timestamp() as f64,
            s_m: 20.0,
            v_mps: 0.0,
        },
    );
    cache.set_hot(Source::MtaBus, previous).await;
    let hot = TrajectoryDeriver::new(Arc::new(TrajectoryEngine::new()), cache)
        .derive(snapshot)
        .await
        .unwrap();
    assert_eq!(hot.render_units.len(), 1);
    assert!(
        (hot.prev_states[&id].s_m - 20.0).abs() < 0.1,
        "continuity must anchor the trip to the previous source generation"
    );
}

#[tokio::test]
async fn trajectory_subway_connected_segments_preserve_consist_and_platform_data() {
    use backend::{
        models::{
            stop::StopData,
            trip::{Consist, MtaSubwayStopTimeData, MtaSubwayTripData},
        },
        trajectory::snapshot_from_persisted_trip,
    };
    let now = Utc::now();
    let mut trip = fixture().trips.remove(0);
    trip.route_id = "A".into();
    trip.direction = 1;
    trip.shape_ids = vec!["first".into(), "second".into(), "third".into()];
    trip.data = TripData::MtaSubway(MtaSubwayTripData {
        consist: Some(Consist {
            car_count: 8,
            car_length_feet: 60,
        }),
        consist_cars: vec![],
    });
    let mut revision = StaticTransitRevision::from_dataset(&common::mta_subway_dataset());
    if let StopData::MtaSubway(data) = &mut revision.stops.get_mut("101").unwrap().data {
        data.platform_edges = vec![backend::models::stop::PlatformEdge {
            id: "pinned-edge".into(),
            length_ft: 600.0,
            car_markers: vec![],
            egress_points: vec![],
        }];
    }
    let a = (-74.0, 40.7);
    let b = (-73.99, 40.7);
    let c = (-73.98, 40.7);
    let d = (-73.97, 40.7);
    for (id, coords) in [
        ("first", vec![b, a]),
        ("second", vec![c, b]),
        ("third", vec![c, d]),
    ] {
        revision
            .shapes
            .insert(id.into(), Geom::from(geo::LineString::from(coords)));
    }
    revision.stops.get_mut("101").unwrap().geom = Geom::from(geo::Point::from(a));
    revision.stops.get_mut("102").unwrap().geom = Geom::from(geo::Point::from(d));
    let stop_times = ["101", "102"]
        .iter()
        .enumerate()
        .map(|(i, id)| StopTime {
            trip_id: trip.id,
            stop_id: (*id).into(),
            arrival: now + Duration::seconds(i as i64 * 180),
            departure: now + Duration::seconds(i as i64 * 180 + 30),
            data: StopTimeData::MtaSubway(MtaSubwayStopTimeData {
                scheduled_track: None,
                actual_track: None,
                platform_edges: vec!["pinned-edge".into()],
            }),
        })
        .collect::<Vec<_>>();
    let cache = Arc::new(TrajectoryCache::new());
    let input =
        snapshot_from_persisted_trip(&trip, stop_times.clone(), vec![], &revision, &cache, now)
            .await
            .unwrap();
    assert_eq!(input.shape, geo::LineString::from(vec![a, b, c, d]));
    assert_eq!(input.consist_car_count, Some(8));
    assert!((input.consist_length_m.unwrap() - 146.304).abs() < 0.001);
    let StopData::MtaSubway(stop) = &input.stops[0].stop_data else {
        panic!("subway stop data preserved")
    };
    let StopData::MtaSubway(indexed_stop) = &revision.stops["101"].data else {
        unreachable!()
    };
    assert_eq!(stop.platform_edges.len(), indexed_stop.platform_edges.len());
    assert_eq!(stop.platform_edges[0].id, "pinned-edge");
    let StopTimeData::MtaSubway(data) = &input.stops[0].stop_time_data else {
        unreachable!()
    };
    assert_eq!(data.platform_edges, ["pinned-edge"]);
    let snapshot = PersistedSnapshot {
        source: Source::MtaSubway,
        trips: vec![trip],
        stop_times,
        positions: vec![],
        changed_trip_ids: HashSet::new(),
        changes: IngestionChanges::default(),
        static_revision: Arc::new(revision),
    };
    let hot = TrajectoryDeriver::new(Arc::new(TrajectoryEngine::new()), cache)
        .derive(snapshot)
        .await
        .unwrap();
    assert_eq!(
        hot.render_units.len(),
        8,
        "subway consist expands into one render unit per car"
    );
}

#[tokio::test]
async fn trajectory_disconnected_subway_segments_are_rejected() {
    use backend::trajectory::snapshot_from_persisted_trip;
    let snapshot = fixture();
    let mut trip = snapshot.trips[0].clone();
    let mut revision = (*snapshot.static_revision).clone();
    revision.source = Source::MtaSubway;
    trip.shape_ids.push("disconnected".into());
    revision.shapes.insert(
        "disconnected".into(),
        Geom::from(geo::LineString::from(vec![(-73.9, 40.7), (-73.89, 40.7)])),
    );
    let error = snapshot_from_persisted_trip(
        &trip,
        snapshot.stop_times,
        vec![],
        &revision,
        &TrajectoryCache::new(),
        Utc::now(),
    )
    .await
    .err()
    .unwrap();
    assert!(error.to_string().contains("Disconnected"));
}

#[tokio::test]
async fn trajectory_live_hot_miss_returns_empty_without_database() {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .acquire_timeout(std::time::Duration::from_millis(100))
        .connect_lazy("postgres://invalid:invalid@127.0.0.1:1/invalid")
        .unwrap();
    let redis = bb8::Pool::builder()
        .build_unchecked(bb8_redis::RedisConnectionManager::new("redis://127.0.0.1:1").unwrap());
    let state = common::app_state(pool.clone(), redis);
    let (router, _) = utoipa_axum::router::OpenApiRouter::new()
        .nest("/api/v1", backend::api::router(state))
        .split_for_parts();
    let server = axum_test::TestServer::new(router);
    let response = server.get("/api/v1/trajectories/mta_bus").await;
    response.assert_status_ok();
    let rows =
        arrow::ipc::reader::StreamReader::try_new(std::io::Cursor::new(response.as_bytes()), None)
            .unwrap()
            .map(|batch| batch.unwrap().num_rows())
            .sum::<usize>();
    assert_eq!(rows, 0);
    assert_eq!(
        pool.size(),
        0,
        "live miss cannot acquire a PostgreSQL connection"
    );
}

#[tokio::test]
async fn trajectory_bus_uses_one_resolved_whole_route_shape() {
    use backend::trajectory::snapshot_from_persisted_trip;
    let mut snapshot = fixture();
    snapshot.trips[0].shape_ids.push("committed-shape".into());
    let error = snapshot_from_persisted_trip(
        &snapshot.trips[0],
        snapshot.stop_times,
        vec![],
        &snapshot.static_revision,
        &TrajectoryCache::new(),
        Utc::now(),
    )
    .await
    .err()
    .unwrap();
    assert!(error.to_string().contains("one resolved whole-route shape"));
}

#[tokio::test]
async fn trajectory_nonfinite_position_does_not_hide_other_trips() {
    let mut snapshot = fixture();
    let id = snapshot.trips[0].id;
    let mut broken = snapshot.trips[0].clone();
    broken.id = uuid::Uuid::now_v7();
    let mut position = snapshot.positions[0].clone();
    position.trip_id = Some(broken.id);
    position.geom = Some(Geom::from(geo::Point::new(f64::INFINITY, 40.7)));
    snapshot.trips.push(broken);
    snapshot.positions.push(position);
    let hot = TrajectoryDeriver::new(
        Arc::new(TrajectoryEngine::new()),
        Arc::new(TrajectoryCache::new()),
    )
    .derive(snapshot)
    .await
    .unwrap();
    assert_eq!(hot.prev_states.len(), 1);
    assert!(hot.prev_states.contains_key(&id));
}

#[tokio::test]
async fn trajectory_real_subway_fixture_preserves_ordered_segment_junctions() {
    use backend::{
        fixtures::{FixtureKind, load_manifest_for, read_json_payload},
        sources::mta_subway::realtime::build_realtime_from_fixture,
        trajectory::snapshot_from_persisted_trip,
    };
    let root = common::fixture_root();
    let manifest =
        load_manifest_for(&root, Source::MtaSubway, FixtureKind::Realtime, "basic").unwrap();
    let at = chrono::DateTime::from_timestamp(1783352000, 0).unwrap();
    let collected =
        build_realtime_from_fixture(read_json_payload(&root, &manifest, "trips").unwrap(), at)
            .unwrap();
    let revision = StaticTransitRevision::from_dataset(&common::mta_subway_dataset());
    let cache = TrajectoryCache::new();
    assert_eq!(collected.trips.len(), 719);
    let mut failures = Vec::new();
    let mut gap_junctions = 0;
    for (trip, stops) in collected.trips {
        // Equivalent to the old ordered ST_MakeLine(LineString...) assembly:
        // retain each segment's coordinate order, and collapse only its first
        // node when it exactly repeats the preceding segment's final node.
        let mut expected = Vec::new();
        for id in &trip.shape_ids {
            let geo::Geometry::LineString(line) = &revision.shapes[id].0 else {
                panic!("fixture shape must be a line")
            };
            let shared = expected.last() == line.0.first();
            if !expected.is_empty() && !shared {
                gap_junctions += 1;
            }
            expected.extend(line.0.iter().skip(usize::from(shared)).copied());
        }
        match snapshot_from_persisted_trip(&trip, stops, vec![], &revision, &cache, at).await {
            Ok(input) => assert_eq!(
                input.shape.0, expected,
                "provider segment order and both distinct junction endpoints must survive for {}",
                trip.original_id
            ),
            Err(error) => failures.push(format!("{}: {error}", trip.original_id)),
        }
    }
    assert!(
        gap_junctions > 100,
        "fixture must exercise real station junction gaps"
    );
    assert!(
        failures.is_empty(),
        "{} of 719 real feed trips failed construction; first failures: {:?}",
        failures.len(),
        &failures[..failures.len().min(5)]
    );
}

#[tokio::test]
async fn trajectory_subway_rejects_unrelated_topological_legs_even_with_shared_coordinates() {
    use backend::trajectory::snapshot_from_persisted_trip;
    let snapshot = fixture();
    let mut trip = snapshot.trips[0].clone();
    let mut revision = (*snapshot.static_revision).clone();
    revision.source = Source::MtaSubway;
    trip.shape_ids = vec!["76-75".into(), "254-255".into()];
    revision.shapes.insert(
        "76-75".into(),
        Geom::from(geo::LineString::from(vec![(-74.0, 40.7), (-73.99, 40.7)])),
    );
    revision.shapes.insert(
        "254-255".into(),
        Geom::from(geo::LineString::from(vec![(-73.99, 40.7), (-73.98, 40.7)])),
    );
    let result = snapshot_from_persisted_trip(
        &trip,
        snapshot.stop_times,
        vec![],
        &revision,
        &TrajectoryCache::new(),
        Utc::now(),
    )
    .await;
    assert!(
        result.is_err(),
        "different station identities must not join just because coordinates coincide"
    );
}
