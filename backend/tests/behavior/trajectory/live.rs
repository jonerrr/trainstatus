use crate::support;
use backend::fixtures::FixtureKind;
use backend::fixtures::load_manifest_for;
use backend::fixtures::read_json_payload;
use backend::geometry::Geom;
use backend::network::index::NetworkRevision;
use backend::network::types::stop::StopData;
use backend::operations::IngestionChanges;
use backend::operations::SourceSnapshot;
use backend::operations::types::position::MtaBusPositionData;
use backend::operations::types::position::PositionData;
use backend::operations::types::position::VehicleObservation;
use backend::operations::types::trip::Consist;
use backend::operations::types::trip::MtaBusData;
use backend::operations::types::trip::MtaSubwayStopTimeData;
use backend::operations::types::trip::MtaSubwayTripData;
use backend::operations::types::trip::StopTimeData;
use backend::operations::types::trip::StopVisit;
use backend::operations::types::trip::TripData;
use backend::operations::types::trip::TripRun;
use backend::source::Source;
use backend::sources::mta_subway::realtime::build_realtime_from_fixture;
use backend::trajectory::TrajectoryCache;
use backend::trajectory::TrajectoryCalculator;
use backend::trajectory::TrajectoryDeriver;
use backend::trajectory::snapshot_from_persisted_trip;
use chrono::{Duration, Utc};
use std::sync::Arc;

fn fixture() -> SourceSnapshot {
    let now = Utc::now();
    let id = uuid::Uuid::now_v7();
    let mut revision = NetworkRevision::from_dataset(&support::mta_bus_dataset());
    let stop_ids = ["504409", "901701"];
    for (i, stop) in stop_ids.iter().enumerate() {
        std::sync::Arc::make_mut(&mut revision.stops)
            .get_mut(*stop)
            .unwrap()
            .geom = Geom::from(geo::Point::new(-74.0 + i as f64 * 0.01, 40.7));
    }
    std::sync::Arc::make_mut(&mut revision.shapes).insert(
        "committed-shape".into(),
        Geom::from(geo::LineString::from(vec![(-74.0, 40.7), (-73.99, 40.7)])),
    );
    SourceSnapshot {
        source: Source::MtaBus,
        committed_at: chrono::Utc::now(),
        trips: vec![TripRun {
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
            .map(|(i, stop)| StopVisit {
                trip_id: id,
                stop_sequence: i as i32,
                stop_id: (*stop).into(),
                arrival: now + Duration::seconds(i as i64 * 180),
                departure: now + Duration::seconds(i as i64 * 180),
                data: StopTimeData::MtaBus,
            })
            .collect(),
        positions: vec![VehicleObservation {
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
        changes: IngestionChanges::default(),
        network_revision: Arc::new(revision),
    }
}

#[tokio::test]
async fn trajectory_from_persisted_snapshot() {
    let snapshot = fixture();
    let id = snapshot.trips[0].id;
    let deriver = TrajectoryDeriver::new(
        Arc::new(TrajectoryCalculator::new()),
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
    assert!(
        unit.trajectory
            .motion
            .iter()
            .any(|p| p.a != p.a + p.b + p.c + p.d)
    );
    assert!(hot.plans.contains_key(&id));
}

#[tokio::test]
async fn trajectory_passengers_use_the_latest_report_with_retained_gps_history() {
    let mut snapshot = fixture();
    snapshot.positions[0].updated_at -= Duration::seconds(20);
    let cache = Arc::new(TrajectoryCache::new());
    let deriver = TrajectoryDeriver::new(Arc::new(TrajectoryCalculator::new()), cache.clone());
    let first = deriver.derive(&snapshot).await.unwrap();
    assert_eq!(
        first.render_units.values().next().unwrap().passengers,
        Some(12)
    );
    cache.set_hot(Source::MtaBus, first).await;

    for passengers in [Some(25), None] {
        snapshot.positions[0].updated_at += Duration::seconds(5);
        let PositionData::MtaBus(data) = &mut snapshot.positions[0].data else {
            unreachable!();
        };
        data.passengers = passengers;
        let hot = deriver.derive(&snapshot).await.unwrap();
        assert_eq!(
            hot.render_units.values().next().unwrap().passengers,
            passengers
        );
        cache.set_hot(Source::MtaBus, hot).await;
    }
}

#[tokio::test]
async fn trajectory_uses_pinned_stop_geometry_and_color() {
    let snapshot = fixture();
    let cache = TrajectoryCache::new();
    let first = snapshot_from_persisted_trip(
        &snapshot.trips[0],
        snapshot.stop_times.clone(),
        vec![],
        &snapshot.network_revision,
        &cache,
        Utc::now(),
    )
    .await
    .unwrap();
    let mut replacement = (*snapshot.network_revision).clone();
    std::sync::Arc::make_mut(&mut replacement.routes)
        .get_mut("B94")
        .unwrap()
        .color = "ABCDEF".into();
    std::sync::Arc::make_mut(&mut replacement.stops)
        .get_mut("504409")
        .unwrap()
        .geom = Geom::from(geo::Point::new(-73.995, 40.7));
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
    let mut revision = (*snapshot.network_revision).clone();
    std::sync::Arc::make_mut(&mut revision.shapes).insert(
        "nonfinite".into(),
        Geom::from(geo::LineString::from(vec![(f64::NAN, 40.7), (-73.9, 40.7)])),
    );
    snapshot.network_revision = Arc::new(revision);
    snapshot.trips.push(malformed);
    let cache = Arc::new(TrajectoryCache::new());
    let deriver = TrajectoryDeriver::new(Arc::new(TrajectoryCalculator::new()), cache.clone());
    let hot = deriver.derive(snapshot).await.unwrap();
    assert_eq!(hot.render_units.len(), 1);
    assert!(hot.plans.contains_key(&valid_id));
    cache.set_hot(Source::MtaBus, hot).await;
    let mut empty = fixture();
    empty.trips.clear();
    empty.stop_times.clear();
    empty.positions.clear();
    let cleared = deriver.derive(empty).await.unwrap();
    assert!(cleared.render_units.is_empty() && cleared.plans.is_empty());
}

#[tokio::test]
async fn gps_reconciles_reachable_previous_state_and_discards_unreachable_backward_jump() {
    let mut snapshot = fixture();
    // A sole GPS fix stays stationary, so elapsed setup time cannot change
    // the expected position between derivations.
    snapshot.stop_times.clear();
    for position in &mut snapshot.positions {
        position.updated_at -= Duration::seconds(20);
    }
    let id = snapshot.trips[0].id;
    let cache = Arc::new(TrajectoryCache::new());
    let deriver = TrajectoryDeriver::new(Arc::new(TrajectoryCalculator::new()), cache.clone());
    let first = deriver.derive(&snapshot).await.unwrap();
    let fix = first.plans[&id].gps_fix.as_ref().unwrap();
    let expected = fix.distance_m;
    let accepted_at = fix.accepted_at;
    for (offset, retained_offset) in [(10.0, 10.0), (500.0, 0.0)] {
        let mut previous = first.clone();
        for interval in &mut Arc::make_mut(previous.plans.get_mut(&id).unwrap()).motion {
            interval.a += offset;
        }
        cache.set_hot(Source::MtaBus, previous).await;
        let next = deriver.derive(&snapshot).await.unwrap();
        let plan = &next.plans[&id];
        let state = plan
            .motion
            .iter()
            .find(|p| p.t0 <= plan.generated_at && plan.generated_at <= p.t1)
            .expect("motion covers derivation time")
            .evaluate(plan.generated_at);
        assert!(
            (state.s_m - expected - retained_offset).abs() < 1e-6,
            "previous offset {offset}: expected {}, got {}",
            expected + retained_offset,
            state.s_m
        );
        assert!(state.v_mps.abs() < 1e-6);
        assert_eq!(plan.gps_fix.as_ref().unwrap().accepted_at, accepted_at);
    }
}

#[tokio::test]
async fn trajectory_subway_connected_segments_preserve_consist_and_platform_data() {
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
    let mut revision = NetworkRevision::from_dataset(&support::mta_subway_dataset());
    if let StopData::MtaSubway(data) = &mut std::sync::Arc::make_mut(&mut revision.stops)
        .get_mut("101")
        .unwrap()
        .data
    {
        data.platform_edges = vec![backend::network::types::stop::PlatformEdge {
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
        std::sync::Arc::make_mut(&mut revision.shapes)
            .insert(id.into(), Geom::from(geo::LineString::from(coords)));
    }
    std::sync::Arc::make_mut(&mut revision.stops)
        .get_mut("101")
        .unwrap()
        .geom = Geom::from(geo::Point::from(a));
    std::sync::Arc::make_mut(&mut revision.stops)
        .get_mut("102")
        .unwrap()
        .geom = Geom::from(geo::Point::from(d));
    let stop_times = ["101", "102"]
        .iter()
        .enumerate()
        .map(|(i, id)| StopVisit {
            trip_id: trip.id,
            stop_sequence: i as i32,
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
    let snapshot = SourceSnapshot {
        source: Source::MtaSubway,
        committed_at: chrono::Utc::now(),
        trips: vec![trip],
        stop_times,
        positions: vec![],
        changes: IngestionChanges::default(),
        network_revision: Arc::new(revision),
    };
    let hot = TrajectoryDeriver::new(Arc::new(TrajectoryCalculator::new()), cache)
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
    let snapshot = fixture();
    let mut trip = snapshot.trips[0].clone();
    let mut revision = (*snapshot.network_revision).clone();
    revision.source = Source::MtaSubway;
    trip.shape_ids.push("disconnected".into());
    std::sync::Arc::make_mut(&mut revision.shapes).insert(
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
async fn trajectory_bus_uses_one_resolved_whole_route_shape() {
    let mut snapshot = fixture();
    snapshot.trips[0].shape_ids.push("committed-shape".into());
    let error = snapshot_from_persisted_trip(
        &snapshot.trips[0],
        snapshot.stop_times,
        vec![],
        &snapshot.network_revision,
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
        Arc::new(TrajectoryCalculator::new()),
        Arc::new(TrajectoryCache::new()),
    )
    .derive(snapshot)
    .await
    .unwrap();
    assert_eq!(hot.plans.len(), 1);
    assert!(hot.plans.contains_key(&id));
}

#[tokio::test]
async fn trajectory_real_subway_fixture_preserves_ordered_segment_junctions() {
    let root = support::fixture_root();
    let manifest =
        load_manifest_for(&root, Source::MtaSubway, FixtureKind::Realtime, "basic").unwrap();
    let at = chrono::DateTime::from_timestamp(1783352000, 0).unwrap();
    let collected =
        build_realtime_from_fixture(read_json_payload(&root, &manifest, "trips").unwrap()).unwrap();
    let revision = NetworkRevision::from_dataset(&support::mta_subway_dataset());
    let cache = TrajectoryCache::new();
    let trip_count = collected.trips.len();
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
        let id = uuid::Uuid::now_v7();
        let trip = trip.with_id(id);
        let stops = stops
            .into_iter()
            .map(|stop| stop.with_trip_id(id))
            .collect();
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
        "{} of {trip_count} real feed trips failed construction; first failures: {:?}",
        failures.len(),
        &failures[..failures.len().min(5)]
    );
}

#[tokio::test]
async fn trajectory_subway_rejects_unrelated_topological_legs_even_with_shared_coordinates() {
    let snapshot = fixture();
    let mut trip = snapshot.trips[0].clone();
    let mut revision = (*snapshot.network_revision).clone();
    revision.source = Source::MtaSubway;
    trip.shape_ids = vec!["76-75".into(), "254-255".into()];
    std::sync::Arc::make_mut(&mut revision.shapes).insert(
        "76-75".into(),
        Geom::from(geo::LineString::from(vec![(-74.0, 40.7), (-73.99, 40.7)])),
    );
    std::sync::Arc::make_mut(&mut revision.shapes).insert(
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

#[tokio::test]
async fn captured_subway_trajectories_stay_below_speed_limit_over_ten_minutes() {
    let root = support::fixture_root();
    let manifest =
        load_manifest_for(&root, Source::MtaSubway, FixtureKind::Realtime, "basic").unwrap();
    let collected =
        build_realtime_from_fixture(read_json_payload(&root, &manifest, "trips").unwrap()).unwrap();
    let at = chrono::DateTime::from_timestamp(1783352000, 0).unwrap();
    let revision = NetworkRevision::from_dataset(&support::mta_subway_dataset());
    let cache = TrajectoryCache::new();
    let engine = TrajectoryCalculator::new();
    let config = backend::trajectory::TrajectoryConfig::for_source(Source::MtaSubway);
    let mut worst = (0.0_f64, String::new(), 0.0);
    let mut frames = 0;
    for (trip, stops) in collected.trips {
        let id = uuid::Uuid::now_v7();
        let trip = trip.with_id(id);
        let stops = stops.into_iter().map(|s| s.with_trip_id(id)).collect();
        let input = snapshot_from_persisted_trip(&trip, stops, vec![], &revision, &cache, at)
            .await
            .unwrap();
        let Ok(computed) = backend::trajectory::compute_trajectory_async(
            &engine,
            Source::MtaSubway,
            &input,
            None,
            &cache,
            &config,
        )
        .await
        else {
            continue;
        };
        let motion = &computed.trajectory.motion;
        for frame in 0..18_000 {
            let time = at.timestamp() as f64 + frame as f64 / 30.0;
            let index = motion
                .partition_point(|p| p.t1 <= time)
                .min(motion.len() - 1);
            let speed = if time <= motion[0].t0 || time >= motion.last().unwrap().t1 {
                0.0
            } else {
                motion[index].evaluate(time).v_mps
            };
            frames += 1;
            if speed > worst.0 {
                worst = (speed, trip.original_id.clone(), time);
            }
        }
    }
    assert!(
        frames >= 18_000,
        "captured trips must exercise a full replay"
    );
    assert!(
        worst.0 <= 25.0 + 1e-6,
        "captured subway replay: {frames} frames, peak {:.2} m/s on trip {} at {}",
        worst.0,
        worst.1,
        worst.2
    );
}
