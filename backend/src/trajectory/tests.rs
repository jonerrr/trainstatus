#[cfg(test)]
mod collapse_tests {
    use super::super::builder::collapse_backtracking_knots_with_stats;
    use super::super::types::TrajectoryKnot;

    #[test]
    fn collapse_drops_backtrack_keeps_dwell() {
        let knots = vec![
            TrajectoryKnot::new(0.0, 0.0, None),
            TrajectoryKnot::new(10.0, 100.0, Some(0.0)),
            TrajectoryKnot::new(40.0, 100.0, Some(0.0)),
            TrajectoryKnot::new(50.0, 50.0, None),
            TrajectoryKnot::new(60.0, 120.0, None),
        ];
        let (out, _removed) = collapse_backtracking_knots_with_stats(&knots);
        assert_eq!(out.len(), 4);
        assert_eq!(out[0].t_event, 0.0);
        assert_eq!(out[1].t_event, 10.0);
        assert_eq!(out[2].t_event, 40.0); // Dwell knot preserved!
        assert_eq!(out[3].t_event, 60.0);
        for i in 1..out.len() {
            assert!(out[i].s_m >= out[i - 1].s_m);
        }
    }
}

#[cfg(test)]
mod continuity_tests {
    use super::super::continuity::{ContinuityDiscardReason, apply};
    use super::super::types::{TrajectoryConfig, TrajectoryKnot, TrajectoryState};

    #[test]
    fn hold_prevents_distance_regression() {
        let knots = vec![
            TrajectoryKnot::new(100.0, 200.0, None),
            TrajectoryKnot::new(110.0, 150.0, None),
        ];
        let prev = TrajectoryState {
            t_unix: 90.0,
            s_m: 180.0,
            v_mps: 2.0,
        };
        let config = TrajectoryConfig::default();
        let (out, stats) = apply(knots, Some(prev), &config, 95.0);
        assert!(!stats.prev_state_discarded);
        for i in 1..out.len() {
            assert!(out[i].s_m + 1e-6 >= out[i - 1].s_m);
        }
    }

    #[test]
    fn discards_stale_prev_state() {
        let knots = vec![
            TrajectoryKnot::new(100.0, 200.0, None),
            TrajectoryKnot::new(110.0, 250.0, None),
        ];
        let prev = TrajectoryState {
            t_unix: 0.0,
            s_m: 180.0,
            v_mps: 2.0,
        };
        let config = TrajectoryConfig {
            max_prev_state_age_s: 30.0,
            ..Default::default()
        };
        let (_, stats) = apply(knots, Some(prev), &config, 100.0);
        assert!(stats.prev_state_discarded);
        assert_eq!(
            stats.discard_reason,
            Some(ContinuityDiscardReason::StalePrevState)
        );
    }

    #[test]
    fn discards_large_backward_gap() {
        let knots = vec![
            TrajectoryKnot::new(100.0, 100.0, None),
            TrajectoryKnot::new(110.0, 150.0, None),
        ];
        let prev = TrajectoryState {
            t_unix: 95.0,
            s_m: 400.0,
            v_mps: 2.0,
        };
        let config = TrajectoryConfig {
            max_backward_gap_m: 50.0,
            ..Default::default()
        };
        let (_, stats) = apply(knots, Some(prev), &config, 100.0);
        assert!(stats.prev_state_discarded);
        assert_eq!(
            stats.discard_reason,
            Some(ContinuityDiscardReason::BackwardGap)
        );
    }

    #[test]
    fn discards_large_forward_jump() {
        let knots = vec![
            TrajectoryKnot::new(100.0, 500.0, None),
            TrajectoryKnot::new(200.0, 900.0, None),
        ];
        let prev = TrajectoryState {
            t_unix: 95.0,
            s_m: 50.0,
            v_mps: 0.0,
        };
        let config = TrajectoryConfig {
            max_forward_jump_m: 100.0,
            ..Default::default()
        };
        let (_, stats) = apply(knots, Some(prev), &config, 100.0);
        assert!(stats.prev_state_discarded);
        assert_eq!(
            stats.discard_reason,
            Some(ContinuityDiscardReason::ForwardJump)
        );
    }
}

#[cfg(test)]
mod bbox_tests {
    use super::super::types::{bbox_intersects, compute_path_bbox};

    #[test]
    fn bbox_intersection() {
        let trip = [0.0, 0.0, 10.0, 10.0];
        let query = [5.0, 5.0, 15.0, 15.0];
        assert!(bbox_intersects(trip, query));
        let miss = [-10.0, -10.0, -1.0, -1.0];
        assert!(!bbox_intersects(trip, miss));
    }

    #[test]
    fn path_bbox_from_coords() {
        let path = vec![[-74.0, 40.7], [-73.9, 40.8]];
        let b = compute_path_bbox(&path);
        assert!((b[0] - (-74.0)).abs() < 1e-9);
        assert!((b[2] - (-73.9)).abs() < 1e-9);
    }
}

#[cfg(test)]
mod arrow_tests {
    use super::super::arrow::encode_render_units;
    use super::super::types::{RenderUnit, compute_path_bbox};
    use crate::models::source::Source;

    #[test]
    fn encode_empty_round_trip_schema() {
        let bytes = encode_render_units(&[]).expect("encode");
        assert!(!bytes.is_empty());
    }

    #[test]
    fn encode_single_render_unit() {
        let unit = RenderUnit {
            render_unit_id: "mta_subway:t1:0".into(),
            source: Source::MtaSubway,
            trip_id: "t1".into(),
            route_id: "A".into(),
            icon_key: "rail_head".into(),
            unit_index: Some(0),
            unit_count: Some(1),
            is_head: true,
            length_m: 18.288,
            passengers: None,
            color: [255, 0, 0],
            positions: vec![[-74.0, 40.7], [-73.9, 40.8]],
            timestamps: vec![1000.0, 1010.0],
            bearings: vec![350.0, 10.0],
            path_bbox: compute_path_bbox(&[[-74.0, 40.7], [-73.9, 40.8]]),
        };
        let bytes = encode_render_units(&[unit]).expect("encode");
        assert!(bytes.len() > 64);

        if std::env::var("WRITE_ARROW_FIXTURE").is_ok() {
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../frontend/src/lib/map/fixtures/single_trip.arrow");
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).expect("create fixture dir");
            }
            std::fs::write(&path, &bytes).expect("write fixture");
        }
    }
}

#[cfg(test)]
mod mta_subway_tests {
    use chrono::{DateTime, Utc};
    use geo::{Coord, LineString};
    use uuid::Uuid;

    use super::super::builder::TrajectoryBuilder;
    use super::super::builders::mta_subway::MtaSubwayBuilder;
    use super::super::cache::TrajectoryCache;
    use super::super::continuity::apply;
    use super::super::geometry::{build_shape_geometry, distance_to_coord};
    use super::super::types::{
        Trajectory, TrajectoryConfig, TrajectoryState, TrajectoryStop, TripSnapshot,
        compute_path_bbox, source_projected_epsg_code,
    };
    use super::super::{expand_render_units, source_render_config};
    use crate::models::geom::Geom;
    use crate::models::position::{MtaSubwayPositionData, PositionData, VehiclePosition};
    use crate::models::source::Source;
    use crate::models::stop::{
        CarMarker, EgressPoint, EgressType, MtaSubwayStopData, PlatformDirection, PlatformEdge,
        StopData, VerticalDirection,
    };
    use crate::models::trip::{MtaSubwayStopTimeData, StopTimeData};

    #[test]
    fn match_platform_edge_picks_best_candidate() {
        let edges = vec![
            platform_edge(
                "fallback",
                600.0,
                vec![car_marker(300.0, 50.0, PlatformDirection::North)],
            ),
            platform_edge(
                "exact",
                600.0,
                vec![car_marker(480.0, 120.0, PlatformDirection::North)],
            ),
        ];

        let matched = MtaSubwayBuilder::match_platform_edge(&edges, 1, 146.304).unwrap();
        assert_eq!(matched.platform_edge_id, "exact");
    }

    #[test]
    fn live_anchor_prevents_prev_state_discard_when_next_stop_is_far() {
        let (trip, shape_geom) = trip_with_anchor(
            "EN_ROUTE",
            vec![stop(
                "STOP1",
                200.0,
                650.0,
                vec!["platform-a"],
                vec![
                    platform_edge(
                        "platform-a",
                        328.084,
                        vec![car_marker(480.0, 164.042, PlatformDirection::North)],
                    ),
                    platform_edge(
                        "platform-b",
                        328.084,
                        vec![car_marker(300.0, 164.042, PlatformDirection::North)],
                    ),
                ],
            )],
            300.0,
        );

        let builder = MtaSubwayBuilder;
        let generated = builder
            .generate_knots(&trip, None, &shape_geom, &TrajectoryCache::new())
            .expect("knots");
        let prev = TrajectoryState {
            t_unix: 95.0,
            s_m: 305.0,
            v_mps: 4.0,
        };
        let config = TrajectoryConfig {
            max_forward_jump_m: 100.0,
            ..Default::default()
        };

        let (_, stats) = apply(generated.knots, Some(prev), &config, 100.0);
        assert!(!stats.prev_state_discarded);
        assert!(generated.stats.live_anchor_gap_m.unwrap_or_default() > 200.0);
    }

    #[test]
    fn expand_render_units_emits_one_row_per_subway_car() {
        let (trip, shape_geom) = trip_with_anchor("EN_ROUTE", vec![], 220.0);
        let distances_m = vec![220.0, 240.0, 260.0];
        let path: Vec<[f64; 2]> = distances_m
            .iter()
            .filter_map(|distance| {
                distance_to_coord(*distance, &shape_geom.wgs84_line, &shape_geom.cum_dist)
                    .map(|coord| [coord.x, coord.y])
            })
            .collect();
        let trajectory = Trajectory {
            trip_id: trip.trip_id.to_string(),
            route_id: trip.route_id.clone(),
            color: [0, 57, 166],
            path: path.clone(),
            timestamps: vec![100.0, 110.0, 120.0],
            distances_m: distances_m.clone(),
            bearings: vec![0.0, 5.0, 10.0],
            path_bbox: compute_path_bbox(&path),
        };

        let units = expand_render_units(Source::MtaSubway, &trip, &trajectory, &shape_geom);
        let config = source_render_config(Source::MtaSubway);

        assert_eq!(units.len(), 8);
        assert_eq!(units[0].icon_key, config.primary_icon_key);
        assert_eq!(units[1].icon_key, config.trailing_icon_key);
        assert_eq!(units[0].unit_index, Some(0));
        assert_eq!(units[0].unit_count, Some(8));
        assert_eq!(units[0].length_m, 18.288);
        assert!(units[0].positions.len() >= units[1].positions.len());
        assert!(
            units
                .iter()
                .all(|unit| unit.bearings.len() == unit.timestamps.len())
        );
        assert!(
            units
                .iter()
                .all(|unit| unit.positions.len() == unit.timestamps.len())
        );
    }

    #[test]
    fn at_stop_anchor_produces_zero_velocity_knot_and_monotone_path() {
        let (trip, shape_geom) = trip_with_anchor(
            "AT_STOP",
            vec![stop(
                "STOP1",
                160.0,
                300.0,
                vec!["platform-a"],
                vec![platform_edge(
                    "platform-a",
                    328.084,
                    vec![car_marker(480.0, 164.042, PlatformDirection::North)],
                )],
            )],
            300.0,
        );

        let builder = MtaSubwayBuilder;
        let generated = builder
            .generate_knots(&trip, None, &shape_geom, &TrajectoryCache::new())
            .expect("knots");

        assert!(
            generated
                .knots
                .iter()
                .any(|k| (k.t_event - 100.0).abs() < 1e-6 && k.v_clamp == Some(0.0))
        );

        for i in 1..generated.knots.len() {
            assert!(generated.knots[i].s_m + 1e-6 >= generated.knots[i - 1].s_m);
        }
    }

    #[test]
    fn overlapping_station_profiles_stay_monotone_with_anchor() {
        let (trip, shape_geom) = trip_with_anchor(
            "EN_ROUTE",
            vec![
                stop(
                    "STOP1",
                    150.0,
                    350.0,
                    vec!["platform-a"],
                    vec![platform_edge(
                        "platform-a",
                        328.084,
                        vec![car_marker(480.0, 164.042, PlatformDirection::North)],
                    )],
                ),
                stop(
                    "STOP2",
                    180.0,
                    380.0,
                    vec!["platform-b"],
                    vec![platform_edge(
                        "platform-b",
                        328.084,
                        vec![car_marker(480.0, 164.042, PlatformDirection::North)],
                    )],
                ),
            ],
            300.0,
        );

        let builder = MtaSubwayBuilder;
        let generated = builder
            .generate_knots(&trip, None, &shape_geom, &TrajectoryCache::new())
            .expect("knots");
        assert!(generated.stats.backtracking_knots_removed > 0);

        let prev = TrajectoryState {
            t_unix: 95.0,
            s_m: 298.0,
            v_mps: 3.0,
        };
        let (out, stats) = apply(
            generated.knots,
            Some(prev),
            &TrajectoryConfig::default(),
            100.0,
        );
        assert!(!stats.prev_state_discarded);
        for i in 1..out.len() {
            assert!(out[i].s_m + 1e-6 >= out[i - 1].s_m);
        }
    }

    fn trip_with_anchor(
        status: &str,
        stops: Vec<TrajectoryStop>,
        anchor_distance_m: f64,
    ) -> (TripSnapshot, super::super::geometry::ShapeGeometry) {
        let line = LineString::new(vec![
            Coord {
                x: -73.991,
                y: 40.750,
            },
            Coord {
                x: -73.979,
                y: 40.750,
            },
        ]);
        let shape_geom =
            build_shape_geometry(&line, source_projected_epsg_code(Source::MtaSubway)).unwrap();
        let anchor_coord = distance_to_coord(
            anchor_distance_m,
            &shape_geom.wgs84_line,
            &shape_geom.cum_dist,
        )
        .unwrap();
        let anchor_position = VehiclePosition {
            vehicle_id: "train-1".into(),
            trip_id: Some(Uuid::nil()),
            stop_id: None,
            updated_at: ts(100),
            data: PositionData::MtaSubway(MtaSubwayPositionData {
                assigned: true,
                status: Some(status.to_string()),
            }),
            geom: Some(Geom(geo::Geometry::Point(geo::Point::new(
                anchor_coord.x,
                anchor_coord.y,
            )))),
        };

        let trip = TripSnapshot {
            trip_id: Uuid::nil(),
            route_id: "A".into(),
            route_color: "#0039A6".into(),
            direction: 1,
            shape: line,
            shape_key: "shape".into(),
            shape_length_m: shape_geom.length_m,
            consist_length_m: Some(146.304),
            consist_car_count: Some(8),
            consist_car_length_m: Some(18.288),
            stops,
            positions: vec![anchor_position],
            as_of: ts(100),
        };

        (trip, shape_geom)
    }

    fn stop(
        stop_id: &str,
        arrival_unix: f64,
        stop_distance_m: f64,
        hinted_platform_edges: Vec<&str>,
        platform_edges: Vec<PlatformEdge>,
    ) -> TrajectoryStop {
        TrajectoryStop {
            stop_id: stop_id.into(),
            arrival_unix,
            departure_unix: arrival_unix,
            stop_distance_m,
            stop_time_data: StopTimeData::MtaSubway(MtaSubwayStopTimeData {
                scheduled_track: None,
                actual_track: None,
                platform_edges: hinted_platform_edges
                    .into_iter()
                    .map(|id| id.to_string())
                    .collect(),
            }),
            stop_data: StopData::MtaSubway(MtaSubwayStopData {
                gtfs_stop_id: stop_id.into(),
                station_group_id: "group".into(),
                bubble_id: "bubble".into(),
                platform_edges,
                line: "A".into(),
                is_major: true,
                north_headsign: "Uptown".into(),
                south_headsign: "Downtown".into(),
            }),
        }
    }

    fn platform_edge(id: &str, length_ft: f32, car_markers: Vec<CarMarker>) -> PlatformEdge {
        PlatformEdge {
            id: id.into(),
            length_ft,
            car_markers,
            egress_points: vec![EgressPoint {
                id: 1,
                egress_type: EgressType::Staircase,
                position_ft: 0.0,
                vertical_direction: VerticalDirection::Up,
            }],
        }
    }

    fn car_marker(
        consist_length_ft: f32,
        position_ft: f32,
        direction: PlatformDirection,
    ) -> CarMarker {
        CarMarker {
            marked_as: "10".into(),
            is_opto: false,
            consist_length_ft: Some(consist_length_ft),
            position_ft,
            direction,
        }
    }

    fn ts(unix: i64) -> DateTime<Utc> {
        DateTime::from_timestamp(unix, 0).unwrap()
    }
}
