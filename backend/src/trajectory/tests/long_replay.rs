use crate::{
    network::types::stop::{MtaSubwayStopData, NjtBusStopData, StopData},
    operations::types::trip::StopTimeData,
    source::Source,
    trajectory::{
        builders::{mta_subway::MtaSubwayBuilder, njt_bus::NjtBusBuilder},
        compute_trajectory,
        continuity::ContinuityDiscardReason,
        geometry::build_shape_geometry,
        interpolation::{PchipMethod, evaluate_motion},
        types::{TrajectoryConfig, TrajectoryStop, TripSnapshot},
    },
};
use chrono::{DateTime, Utc};

#[derive(serde::Deserialize)]
struct Replay {
    duration_s: i64,
    frame_hz: i64,
    refresh_s: i64,
    subway: Subway,
    njt_prediction: Bus,
}
#[derive(serde::Deserialize)]
struct Subway {
    stop_distances_m: Vec<f64>,
    arrival_offsets_s: Vec<i64>,
    max_speed_mps: f64,
}
#[derive(serde::Deserialize)]
struct Bus {
    first_stop_distance_m: f64,
    first_arrival_s: i64,
    minimum_progress_m: f64,
}
fn replay() -> Replay {
    serde_json::from_str(include_str!(
        "../../../tests/fixtures/trajectory/long-replays.json"
    ))
    .unwrap()
}
fn at(seconds: i64) -> DateTime<Utc> {
    DateTime::from_timestamp(1_780_000_000 + seconds, 0).unwrap()
}
fn trip() -> (TripSnapshot, super::super::geometry::ShapeGeometry) {
    let shape = geo::LineString::from(vec![(-74.0, 40.7), (-73.8, 40.7)]);
    let geometry = build_shape_geometry(&shape, 6538).unwrap();
    (
        TripSnapshot {
            started_at: at(0),
            trip_id: uuid::Uuid::nil(),
            route_id: "replay".into(),
            route_color: "000000".into(),
            direction: 0,
            shape,
            shape_key: geometry.path.key.clone(),
            shape_length_m: geometry.length_m,
            consist_length_m: Some(146.304),
            consist_car_count: Some(8),
            consist_car_length_m: Some(18.288),
            stops: vec![],
            positions: vec![],
            recorded_anchor: None,
            as_of: at(0),
        },
        geometry,
    )
}
#[test]
fn ten_minute_subway_revisions_preserve_continuity_and_bounded_forward_motion() {
    let fixture = replay();
    let (mut trip, geometry) = trip();
    let mut previous = None;
    let mut fastest = 0.0_f64;
    let mut fastest_at = 0.0;
    let mut frames = 0;
    let mut last_distance = None;
    for refresh in (0..fixture.duration_s).step_by(fixture.refresh_s as usize) {
        trip.as_of = at(refresh);
        trip.stops = fixture
            .subway
            .stop_distances_m
            .iter()
            .zip(&fixture.subway.arrival_offsets_s)
            .enumerate()
            .map(|(i, (distance, offset))| TrajectoryStop {
                stop_id: i.to_string(),
                arrival_unix: at(*offset + refresh / 120 * 2).timestamp() as f64,
                departure_unix: at(*offset + refresh / 120 * 2).timestamp() as f64,
                stop_distance_m: *distance,
                stop_time_data: StopTimeData::MtaSubway(
                    crate::operations::types::trip::MtaSubwayStopTimeData {
                        scheduled_track: None,
                        actual_track: None,
                        platform_edges: vec![],
                    },
                ),
                stop_data: StopData::MtaSubway(MtaSubwayStopData {
                    gtfs_stop_id: i.to_string(),
                    station_group_id: i.to_string(),
                    bubble_id: i.to_string(),
                    platform_edges: vec![],
                    line: "A".into(),
                    is_major: false,
                    north_headsign: "North".into(),
                    south_headsign: "South".into(),
                }),
            })
            .collect();
        let computed = compute_trajectory(
            &MtaSubwayBuilder,
            &trip,
            previous.as_ref(),
            &geometry,
            &PchipMethod,
            &TrajectoryConfig::for_source(Source::MtaSubway),
        )
        .unwrap();
        if let Some(previous) = &previous {
            let time = trip.as_of.timestamp() as f64;
            let before = evaluate_motion(&previous.motion, time);
            let after = evaluate_motion(&computed.trajectory.motion, time);
            assert!(
                (before.s_m - after.s_m).abs() < 1e-6,
                "position jump at {refresh}"
            );
            assert!(
                (before.v_mps - after.v_mps).abs() < 1e-6,
                "velocity jump at {refresh}"
            );
        }
        for frame in 0..fixture.refresh_s * fixture.frame_hz {
            let time = trip.as_of.timestamp() as f64 + frame as f64 / fixture.frame_hz as f64;
            let state = evaluate_motion(&computed.trajectory.motion, time);
            if let Some(previous_distance) = last_distance {
                assert!(state.s_m >= previous_distance - 1e-6, "reverse at {time}");
            }
            assert!(state.v_mps >= -1e-6, "negative speed at {time}");
            last_distance = Some(state.s_m);
            frames += 1;
            if state.v_mps > fastest {
                fastest = state.v_mps;
                fastest_at = time;
            }
        }
        previous = Some(computed.trajectory);
    }
    assert!(
        fastest <= fixture.subway.max_speed_mps + 1e-6,
        "subway replay: {frames} frames, peak {fastest:.2} m/s at {fastest_at}; limit {} m/s",
        fixture.subway.max_speed_mps
    );
}
#[test]
fn ten_minute_njt_predictions_preserve_compatible_handoffs_and_make_forward_progress() {
    let fixture = replay();
    let (mut trip, geometry) = trip();
    let mut previous = None;
    let mut first = None;
    let mut last = 0.0;
    let mut stopped = 0;
    let mut last_distance = None;
    for refresh in (60..60 + fixture.duration_s).step_by(fixture.refresh_s as usize) {
        trip.as_of = at(refresh);
        trip.stops = (0..3)
            .map(|i| TrajectoryStop {
                stop_id: i.to_string(),
                arrival_unix: at(fixture.njt_prediction.first_arrival_s
                    + i * 180
                    + refresh / 30 * 5)
                .timestamp() as f64,
                departure_unix: at(fixture.njt_prediction.first_arrival_s
                    + i * 180
                    + refresh / 30 * 5)
                .timestamp() as f64,
                stop_distance_m: fixture.njt_prediction.first_stop_distance_m + i as f64 * 2000.0,
                stop_time_data: StopTimeData::NjtBus,
                stop_data: StopData::NjtBus(NjtBusStopData {
                    stop_code: i.to_string(),
                }),
            })
            .collect();
        let computed = compute_trajectory(
            &NjtBusBuilder,
            &trip,
            previous.as_ref(),
            &geometry,
            &PchipMethod,
            &TrajectoryConfig::for_source(Source::NjtBus),
        )
        .unwrap();
        if let Some(previous) = &previous {
            let time = trip.as_of.timestamp() as f64;
            let before = evaluate_motion(&previous.motion, time);
            let after = evaluate_motion(&computed.trajectory.motion, time);
            if computed.continuity_stats.prev_state_discarded {
                // Rolling ETAs can exceed the configured backward-gap allowance.
                // Such a reset is explicit; compatible revisions must stay continuous.
                assert_eq!(
                    computed.continuity_stats.discard_reason,
                    Some(ContinuityDiscardReason::BackwardGap)
                );
                assert!(
                    before.s_m - after.s_m
                        > TrajectoryConfig::for_source(Source::NjtBus).max_backward_gap_m
                );
                last_distance = None;
            } else {
                assert!(
                    (before.s_m - after.s_m).abs() < 1e-6,
                    "position jump at {refresh}"
                );
                assert!(
                    (before.v_mps - after.v_mps).abs() < 1e-6,
                    "velocity jump at {refresh}"
                );
            }
        }
        for frame in 0..fixture.refresh_s * fixture.frame_hz {
            let state = evaluate_motion(
                &computed.trajectory.motion,
                trip.as_of.timestamp() as f64 + frame as f64 / fixture.frame_hz as f64,
            );
            if let Some(previous_distance) = last_distance {
                assert!(
                    state.s_m >= previous_distance - 1e-6,
                    "reverse at {refresh}, frame {frame}"
                );
            }
            assert!((0.0..=30.0 + 1e-6).contains(&state.v_mps));
            last_distance = Some(state.s_m);
            first.get_or_insert(state.s_m);
            last = state.s_m;
            if state.v_mps < 0.1 {
                stopped += 1;
            }
        }
        previous = Some(computed.trajectory);
    }
    let progress = last - first.unwrap();
    assert!(
        progress >= fixture.njt_prediction.minimum_progress_m,
        "NJT replay: only {progress:.2} m in {} s; {stopped} stationary frames despite advancing trip predictions",
        fixture.duration_s
    );
}
