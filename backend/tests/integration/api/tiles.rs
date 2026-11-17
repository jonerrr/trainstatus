//! Active route geometry for the map is served as Arrow trajectories, not SQL vector tiles.

use backend::source::Source;

#[tokio::test]
async fn subway_connected_shapes_reach_arrow_transport() {
    super::trajectories::assert_trajectory(Source::MtaSubway, 8).await;
}

#[tokio::test]
async fn mta_bus_resolved_shape_reaches_arrow_transport() {
    super::trajectories::assert_trajectory(Source::MtaBus, 1).await;
}

#[tokio::test]
async fn njt_bus_resolved_shape_reaches_arrow_transport() {
    super::trajectories::assert_trajectory(Source::NjtBus, 1).await;
}

#[tokio::test]
async fn catalog_stops_publish_map_filter_fields() {
    use crate::support::{TestContext, mta_bus_dataset, mta_subway_dataset, test_owners};
    use backend::network::types::stop::{CompassDirection, StopData};
    use serde_json::Value;

    let ctx = TestContext::new();
    let storage = ctx.storage();
    let stores = test_owners(storage);
    let mut subway = mta_subway_dataset();
    for stop in &mut subway.stops {
        if let StopData::MtaSubway(data) = &mut stop.data {
            data.north_headsign = "Northbound".into();
            data.south_headsign = "Southbound".into();
        }
    }
    stores.network.persist(subway).await.unwrap();
    let mut bus = mta_bus_dataset();
    for stop in &mut bus.stops {
        if let StopData::MtaBus(data) = &mut stop.data {
            data.direction = CompassDirection::N;
        }
    }
    stores.network.persist(bus).await.unwrap();

    for source in [Source::MtaSubway, Source::MtaBus] {
        let response = stores.catalog.response(source).await.unwrap();
        let catalog: Value = serde_json::from_slice(&response.body).unwrap();
        let stops = catalog["stops"].as_array().unwrap();
        assert!(!stops.is_empty(), "{source} catalog must include stops");
        for stop in stops {
            assert!(stop["id"].is_string());
            assert!(stop["name"].is_string());
            if source == Source::MtaSubway {
                assert_eq!(stop["data"]["north_headsign"], "Northbound");
                assert_eq!(stop["data"]["south_headsign"], "Southbound");
            } else {
                assert_eq!(stop["data"]["direction"], "n");
            }
        }
    }
}
