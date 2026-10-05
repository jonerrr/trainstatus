use crate::support::{contracts, fixture_root, mta_bus_dataset};
use backend::{
    fixtures::{self, FixtureKind},
    models::source::Source,
};

#[test]
fn static_fixture_normalizes_routes_stops_and_shapes() {
    let dataset = mta_bus_dataset();
    contracts::assert_static_dataset_contract(&dataset);
    let root = fixture_root();
    let manifest =
        fixtures::load_manifest_for(&root, Source::MtaBus, FixtureKind::Static, "basic").unwrap();
    fixtures::assert_expected_json(
        &root,
        &manifest,
        "dataset",
        fixtures::static_dataset_expected_value(&dataset),
    );
}
