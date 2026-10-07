use crate::support::mta_bus_dataset;
use backend::{
    models::source::Source,
    static_data::index::{StaticTransitIndex, StaticTransitRevision},
};

#[test]
fn static_index_publishes_one_coherent_revision() {
    let dataset = mta_bus_dataset();
    let index = StaticTransitIndex::new();
    index.publish(StaticTransitRevision::from_dataset(&dataset));

    let revision = index.get(Source::MtaBus).expect("published revision");
    let route = &revision.routes["B100"];
    assert!(!route.shape_ids.is_empty());
    let route_stop_shapes = &revision.route_stop_shapes[&("B100".into(), "300226".into())];
    assert!(!route_stop_shapes.is_empty());
    assert!(
        route_stop_shapes
            .iter()
            .all(|shape_id| route.shape_ids.contains(shape_id))
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
