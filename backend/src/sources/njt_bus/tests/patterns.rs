use super::*;
fn fixture() -> (gtfs_structures::Gtfs, Vec<PatternFeature>) {
    // TODO: why are these imported in 2 different ways
    let gtfs = gtfs_structures::Gtfs::from_path(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/njt_bus/static/basic/raw/patterns_gtfs.zip"
    ))
    .unwrap();
    let features = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/njt_bus/static/basic/raw/operating_patterns.json"
    )))
    .unwrap();
    (gtfs, features)
}
#[test]
fn exact_patterns_survive_import_and_match_all_fixture_trips() {
    let (gtfs, features) = fixture();
    let data = build_patterns(&gtfs, features);
    assert_eq!(data.shapes.len(), 3);
    assert_eq!(data.trips.len(), gtfs.trips.len());
    for (id, pattern) in data.trips {
        assert_eq!(pattern.shape_id, gtfs.trips[&id].shape_id);
        assert_eq!(pattern.route_id, gtfs.trips[&id].route_id);
    }
}
#[test]
fn missing_duplicate_and_wrong_direction_patterns_are_rejected() {
    let (gtfs, features) = fixture();
    let missing = build_patterns(&gtfs, vec![]);
    assert!(missing.shapes.is_empty());
    assert_eq!(missing.trips.len(), gtfs.trips.len());
    assert!(missing.trips.values().all(|trip| trip.shape_id.is_none()));
    let mut duplicate = features.clone();
    duplicate.push(features[0].clone());
    assert_eq!(build_patterns(&gtfs, duplicate).shapes.len(), 2);
    let mut mismatch = features;
    mismatch[0].attributes.route_dir = Some("invalid".into());
    assert_eq!(build_patterns(&gtfs, mismatch).shapes.len(), 2);
}
#[test]
fn reversed_geometry_is_restored_to_gtfs_travel_order() {
    let (gtfs, mut features) = fixture();
    let expected = build_patterns(&gtfs, features.clone());
    for f in &mut features {
        for path in &mut f.geometry.as_mut().unwrap().paths {
            path.reverse();
        }
    }
    let actual = build_patterns(&gtfs, features);
    assert_eq!(actual.shapes.len(), 3);
    for (a, b) in actual.shapes.iter().zip(&expected.shapes) {
        assert_eq!(a.geom.0, b.geom.0);
    }
}
#[test]
fn multipart_requires_connected_unbranched_endpoints() {
    let a = [-74.0, 40.0];
    let b = [-74.01, 40.01];
    let c = [-74.02, 40.02];
    let line = connected_line(&PatternGeometry {
        paths: vec![vec![a, a, b], vec![c, b]],
    })
    .unwrap();
    assert_eq!(line.0, vec![Coord::from(a), Coord::from(b), Coord::from(c)]);
    assert!(
        connected_line(&PatternGeometry {
            paths: vec![vec![a, b], vec![c, [-75.0, 41.0]]]
        })
        .is_err()
    );
    assert!(
        connected_line(&PatternGeometry {
            paths: vec![vec![a, b], vec![b, c], vec![b, [-75.0, 41.0]]]
        })
        .is_err()
    );
    assert!(
        connected_line(&PatternGeometry {
            paths: vec![vec![a, a]]
        })
        .is_err()
    );
    assert!(
        connected_line(&PatternGeometry {
            paths: vec![vec![a, [f64::NAN, 40.0]]]
        })
        .is_err()
    );
}
#[test]
fn pagination_rejects_errors_duplicates_and_nonprogress() {
    let (_, features) = fixture();
    let mut all = Vec::new();
    assert!(
        append_page(
            &mut all,
            PatternPage {
                features: features[..1].to_vec(),
                exceeded_transfer_limit: true,
                error: None
            }
        )
        .unwrap()
    );
    assert!(
        !append_page(
            &mut all,
            PatternPage {
                features: features[1..].to_vec(),
                exceeded_transfer_limit: false,
                error: None
            }
        )
        .unwrap()
    );
    assert!(
        append_page(
            &mut all,
            PatternPage {
                features: vec![],
                exceeded_transfer_limit: true,
                error: None
            }
        )
        .is_err()
    );
    assert!(
        append_page(
            &mut all,
            PatternPage {
                features: vec![],
                exceeded_transfer_limit: false,
                error: Some(serde_json::json!({"code":500}))
            }
        )
        .is_err()
    );
    assert!(
        append_page(
            &mut all,
            PatternPage {
                features,
                exceeded_transfer_limit: false,
                error: None
            }
        )
        .is_err()
    );
}
