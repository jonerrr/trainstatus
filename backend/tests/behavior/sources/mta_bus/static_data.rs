use crate::support::contracts;
use crate::support::fixture_root;
use crate::support::mta_bus_dataset;
use backend::fixtures;
use backend::fixtures::FixtureKind;
use backend::network::types::route::RouteData;
use backend::source::Source;

#[test]
fn static_fixture_normalizes_routes_stops_and_shapes() {
    let dataset = mta_bus_dataset();
    contracts::assert_static_dataset_contract(&dataset);
    for (id, direction_id, expected) in [
        ("B82+", 0, "Spring Crk Seaview via Kings"),
        ("B15", 0, "JFK Airport"),
        ("Q30", 1, "Jamaica LIRR"),
        ("Q70+", 0, "LaGuardia Link Fare Free Terminals B-C"),
        ("B1", 1, "Manhattan Beach Kingsboro CC"),
        ("X38", 1, "Coney Isl Sea Gte via 23 St-FDR/Crop"),
    ] {
        let route = dataset.routes.iter().find(|route| route.id == id).unwrap();
        let RouteData::MtaBus(data) = &route.data else {
            panic!("MTA bus route")
        };
        assert_eq!(
            data.directions
                .iter()
                .find(|direction| direction.direction_id == direction_id)
                .unwrap()
                .headsign,
            expected
        );
    }
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
