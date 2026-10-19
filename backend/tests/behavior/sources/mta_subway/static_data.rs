use crate::support::contracts;
use crate::support::fixture_root;
use crate::support::mta_subway_dataset;
use backend::fixtures;
use backend::fixtures::FixtureKind;
use backend::source::Source;

#[test]
fn static_fixture_normalizes_routes_stops_and_shapes() {
    let dataset = mta_subway_dataset();
    contracts::assert_static_dataset_contract(&dataset);
    let root = fixture_root();
    let manifest =
        fixtures::load_manifest_for(&root, Source::MtaSubway, FixtureKind::Static, "basic")
            .unwrap();
    fixtures::assert_expected_json(
        &root,
        &manifest,
        "dataset",
        fixtures::static_dataset_expected_value(&dataset),
    );
}

#[test]
fn named_platform_exits_keep_street_destinations_and_accessible_paths() {
    let root = fixture_root();
    let basic = fixtures::load_manifest_for(&root, Source::MtaSubway, FixtureKind::Static, "basic")
        .unwrap();
    let named =
        fixtures::load_manifest_for(&root, Source::MtaSubway, FixtureKind::Static, "named_exits")
            .unwrap();
    let dataset = backend::sources::mta_subway::static_data::build_static_dataset_from_fixtures(
        fixtures::read_json_payload(&root, &basic, "infrastructure").unwrap(),
        fixtures::read_json_payload(&root, &named, "exit_strategy").unwrap(),
    )
    .unwrap();
    let catalog = backend::api::dto::catalog::Catalog::new(
        Source::MtaSubway,
        &dataset.routes,
        &dataset.stops,
    );
    let json = serde_json::to_value(catalog).unwrap();
    let stop = json["stops"]
        .as_array()
        .unwrap()
        .iter()
        .find(|stop| stop["id"] == "181")
        .unwrap();
    let edge = stop["data"]["platform_edges"]
        .as_array()
        .unwrap()
        .iter()
        .find(|edge| edge["id"] == "181-PM-A2")
        .unwrap();
    let points = edge["egress_points"].as_array().unwrap();
    assert_eq!(
        points[0]["street_exits"],
        serde_json::json!([{ "id": 1, "intersection": ["Stuyvesant Av"], "accessible": false }])
    );
    assert_eq!(
        points[1]["street_exits"],
        serde_json::json!([{ "id": 0, "intersection": ["Utica Av"], "accessible": true }])
    );
    assert_eq!(
        points[2]["street_exits"],
        serde_json::json!([{ "id": 0, "intersection": ["Utica Av"], "accessible": false }])
    );
}
