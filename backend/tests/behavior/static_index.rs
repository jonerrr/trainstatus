use crate::support::mta_bus_dataset;
use backend::{
    models::source::Source,
    static_index::{StaticTransitIndex, StaticTransitRevision},
};

#[test]
fn static_index_publishes_one_coherent_revision() {
    let dataset = mta_bus_dataset();
    let index = StaticTransitIndex::new();
    index.publish(StaticTransitRevision::from_dataset(&dataset));

    let revision = index.get(Source::MtaBus).expect("published revision");
    assert_eq!(revision.routes["B100"].shape_ids, ["B1000113", "B1000120"]);
    assert_eq!(
        revision.route_stop_shapes[&("B100".into(), "300226".into())],
        ["B1000120"]
    );
    assert!(matches!(
        revision.stops["300226"].geom.0,
        geo::Geometry::Point(_)
    ));
    assert!(matches!(
        revision.shapes["B1000120"].0,
        geo::Geometry::LineString(_)
    ));
}
