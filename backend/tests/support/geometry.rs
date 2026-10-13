use backend::network::dataset::NetworkDataset;
// Matched geometry inputs for output contracts. Adapter normalization has separate coverage.

use backend::geometry::Geom;
use backend::network::types::shape::Shape;
use backend::operations::CollectedSnapshot;
use backend::operations::CollectedTrip as TripRun;
use backend::operations::CollectedVisit as StopVisit;
use backend::operations::types::trip::Consist;
use backend::operations::types::trip::MtaBusData;
use backend::operations::types::trip::MtaSubwayStopTimeData;
use backend::operations::types::trip::MtaSubwayTripData;
use backend::operations::types::trip::NjtBusData;
use backend::operations::types::trip::StopTimeData;
use backend::operations::types::trip::TripData;
use backend::source::Source;
use chrono::Duration;

pub async fn ingest_case(pool: sqlx::PgPool, source: Source) -> super::TestOwners {
    let stores = super::test_owners(pool);
    let (dataset, input) = input(source);
    stores.network.persist(&dataset).await.unwrap();
    stores.ingestor.ingest(input).await.unwrap();
    stores
}

fn input(source: Source) -> (NetworkDataset, CollectedSnapshot) {
    let (mut dataset, route, trip_data, stop_data) = match source {
        Source::MtaSubway => (
            super::mta_subway_dataset(),
            "A",
            TripData::MtaSubway(MtaSubwayTripData {
                consist: Some(Consist {
                    car_count: 8,
                    car_length_feet: 60,
                }),
                consist_cars: vec![],
            }),
            StopTimeData::MtaSubway(MtaSubwayStopTimeData {
                scheduled_track: None,
                actual_track: None,
                platform_edges: vec![],
            }),
        ),
        Source::MtaBus => (
            super::mta_bus_dataset(),
            "B94",
            TripData::MtaBus(MtaBusData { deviation: None }),
            StopTimeData::MtaBus,
        ),
        Source::NjtBus => (
            super::njt_bus_dataset(),
            "87",
            TripData::NjtBus(NjtBusData {
                deviation: None,
                headsign: "Terminal".into(),
            }),
            StopTimeData::NjtBus,
        ),
    };
    // Keep captured source metadata, but use a small matched path and two stops so
    // transport tests do not depend on unrelated static/realtime capture dates.
    let points = [(-74.0, 40.7), (-73.995, 40.7), (-73.99, 40.7)];
    let mut shape_ids = vec!["output-shape".to_owned()];
    let segments = if source == Source::MtaSubway {
        shape_ids.push("output-shape-2".into());
        vec![vec![points[0], points[1]], vec![points[1], points[2]]]
    } else {
        vec![points.to_vec()]
    };
    for (id, coords) in shape_ids.iter().zip(segments) {
        dataset.shapes.push(Shape {
            id: id.clone(),
            source,
            geom: Geom::from(geo::LineString::from(coords)),
            data: serde_json::json!({}),
        });
    }
    let now = super::fixtures::fixed_time();
    let id: backend::operations::RunKey = uuid::Uuid::now_v7().into();
    let stop_ids = if source == Source::MtaSubway {
        vec!["101".to_owned(), "102".to_owned()]
    } else if source == Source::MtaBus {
        vec!["504409".to_owned(), "901701".to_owned()]
    } else {
        dataset
            .stops
            .iter()
            .take(2)
            .map(|stop| stop.id.clone())
            .collect()
    };
    let times = stop_ids
        .iter()
        .enumerate()
        .map(|(i, stop_id)| {
            let stop = dataset
                .stops
                .iter_mut()
                .find(|stop| &stop.id == stop_id)
                .unwrap();
            stop.geom = Geom::from(geo::Point::from(points[i * 2]));
            StopVisit {
                trip_id: id,
                stop_sequence: i as i32,
                stop_id: stop_id.clone(),
                arrival: now + Duration::seconds(30 + i as i64 * 180),
                departure: now + Duration::seconds(40 + i as i64 * 180),
                data: stop_data.clone(),
            }
        })
        .collect();
    let trip = TripRun {
        id,
        original_id: "geometry-output".into(),
        vehicle_id: "output-vehicle".into(),
        route_id: route.into(),
        shape_ids,
        direction: 1,
        created_at: now,
        updated_at: now,
        data: trip_data,
    };
    (
        dataset,
        CollectedSnapshot {
            source,
            network_revision: None,
            renormalize: None,
            trips: vec![(trip, times)],
            positions: vec![],
        },
    )
}
