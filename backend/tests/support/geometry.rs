//! Matched geometry inputs for output contracts. Adapter normalization has separate coverage.

use backend::{
    models::{
        geom::Geom,
        shape::Shape,
        source::Source,
        static_dataset::StaticDataset,
        trip::{
            Consist, MtaBusData, MtaSubwayStopTimeData, MtaSubwayTripData, NjtBusData, StopTime,
            StopTimeData, Trip, TripData,
        },
    },
    realtime::CollectedSnapshot,
};
use chrono::Duration;

pub async fn ingest_case(
    pool: sqlx::PgPool,
    source: Source,
) -> (super::TestStores, super::TestRedis) {
    let cache = super::TestRedis::start().await.unwrap();
    let stores = super::test_stores(pool, cache.pool());
    let (dataset, input) = input(source);
    dataset
        .persist(
            &stores.route_store,
            &stores.stop_store,
            &stores.static_cache_store,
        )
        .await
        .unwrap();
    stores.ingestor.ingest(input).await.unwrap();
    (stores, cache)
}

fn input(source: Source) -> (StaticDataset, CollectedSnapshot) {
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
    let id = uuid::Uuid::now_v7();
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
            StopTime {
                trip_id: id,
                stop_id: stop_id.clone(),
                arrival: now + Duration::seconds(30 + i as i64 * 180),
                departure: now + Duration::seconds(40 + i as i64 * 180),
                data: stop_data.clone(),
            }
        })
        .collect();
    let trip = Trip {
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
            trips: vec![(trip, times)],
            positions: vec![],
        },
    )
}
