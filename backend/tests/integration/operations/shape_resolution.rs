use crate::support::{TestContext, mta_bus_dataset, test_owners};
use backend::{
    operations::{
        identity::RunId,
        source::CollectedSnapshot,
        types::trip::{MtaBusData, StopTimeData, StopVisit, TripData, TripRun},
    },
    source::Source,
};

const B94_STOP_IDS: [&str; 5] = ["504409", "901701", "904218", "904219", "904979"];
const B94_STOP_COORDS: [(f64, f64); 5] = [
    (-73.948151, 40.744723),
    (-73.945602, 40.745906),
    (-73.950969, 40.724258),
    (-73.944334, 40.74747),
    (-73.954116, 40.730018),
];
const B94_REAL_SHAPE: &str = "B940017";
const DECOY_SHAPE: &str = "TEST_DECOY_SHAPE";

fn collected_trip(route_id: &str, stop_ids: &[&str]) -> CollectedSnapshot {
    let id = RunId::arbitrary("shape-resolution-trip");
    let now = crate::support::fixtures::fixed_time();
    let trip = TripRun {
        id,
        original_id: "shape-resolution-trip".into(),
        vehicle_id: "shape-resolution-vehicle".into(),
        route_id: route_id.into(),
        shape_ids: vec![],
        direction: 0,
        created_at: now,
        updated_at: now,
        partition_date: crate::support::fixtures::eastern_service_date(now),
        data: TripData::MtaBus(MtaBusData { deviation: None }),
    };
    let stops = stop_ids
        .iter()
        .enumerate()
        .map(|(i, stop)| {
            let arrival = now + chrono::Duration::minutes(i as i64 * 2);
            StopVisit {
                trip_id: id,
                stop_sequence: i as i32,
                stop_id: (*stop).into(),
                arrival,
                departure: arrival,
                data: StopTimeData::MtaBus,
            }
        })
        .collect();
    CollectedSnapshot {
        source: Source::MtaBus,
        network_revision: None,
        renormalize: None,
        trips: vec![(trip, stops)],
        positions: vec![],
    }
}

#[tokio::test]
async fn resolves_shape_by_stop_hits_over_raw_distance() {
    let ctx = TestContext::new();
    let storage = ctx.storage();
    let stores = test_owners(storage);
    let mut dataset = mta_bus_dataset();
    let expected = dataset
        .shapes
        .iter()
        .find(|shape| shape.id == B94_REAL_SHAPE)
        .expect("real B94 shape")
        .geom
        .clone();
    stores.network.persist(dataset.clone()).await.unwrap();
    dataset.shapes.push(backend::network::types::shape::Shape {
        id: DECOY_SHAPE.into(),
        source: Source::MtaBus,
        geom: backend::geometry::Geom::from(geo::LineString::from(B94_STOP_COORDS.to_vec())),
    });
    let mut revision = backend::network::index::NetworkRevision::from_dataset(&dataset);
    std::sync::Arc::make_mut(&mut revision.routes)
        .get_mut("B94")
        .unwrap()
        .shape_ids
        .push(DECOY_SHAPE.into());
    stores
        .network
        .network_index()
        .publish_bundle(vec![revision]);
    let committed = stores
        .ingestor
        .ingest(collected_trip("B94", &B94_STOP_IDS))
        .await
        .unwrap();
    assert_eq!(committed.trips[0].shape_ids.len(), 1);
    let selected = committed
        .network_revision
        .shape(&committed.trips[0].shape_ids[0])
        .unwrap()
        .unwrap();
    assert_eq!(selected.as_ref(), &expected);
    assert_eq!(committed.changes.resolved_shapes, 1);
}

#[tokio::test]
async fn falls_back_to_distance_when_no_hit_data_exists() {
    let ctx = TestContext::new();
    let storage = ctx.storage();
    let stores = test_owners(storage);
    let dataset = mta_bus_dataset();
    let mut revision = backend::network::index::NetworkRevision::from_dataset(&dataset);
    stores.network.persist(dataset).await.unwrap();
    std::sync::Arc::make_mut(&mut revision.route_stop_shapes).clear();
    stores
        .network
        .network_index()
        .publish_bundle(vec![revision]);
    let committed = stores
        .ingestor
        .ingest(collected_trip("B94", &B94_STOP_IDS))
        .await
        .unwrap();
    assert!(
        committed.trips[0].shape_ids.iter().all(|id| committed
            .network_revision
            .shape(id)
            .unwrap()
            .is_some())
    );
    assert!(!committed.trips[0].shape_ids.is_empty());
    assert_eq!(committed.changes.resolved_shapes, 1);
}
