use backend::models::route::{Route, RouteData};
use backend::models::source::Source;
use backend::models::trip::{StopTime, StopTimeData, Trip, TripData};
use backend::stores::route::RouteStore;
use backend::stores::trip::TripStore;
use bb8_redis::RedisConnectionManager;
use chrono::Utc;
use uuid::Uuid;

#[sqlx::test]
async fn test_save_trip_to_db(pool: sqlx::PgPool) {
    // Setup redis (assuming local redis for test runner)
    let manager = RedisConnectionManager::new("redis://localhost").unwrap();
    let redis_pool = bb8::Pool::builder().build(manager).await.unwrap();

    let trip_store = TripStore::new(pool.clone(), redis_pool.clone());
    let route_store = RouteStore::new(pool.clone(), redis_pool.clone());

    let source = Source::MtaSubway;

    // 1. Seed a route (needed for FK)
    let route = Route {
        id: "1".to_string(),
        long_name: "Broadway - 7 Avenue Local".to_string(),
        short_name: "1".to_string(),
        color: "EE352E".to_string(),
        data: RouteData::MtaSubway,
        geom: None,
    };
    route_store
        .save_all(source, &[route])
        .await
        .expect("Failed to seed route");

    // 2. Create a trip
    let now = Utc::now();
    let arrival = now + chrono::Duration::minutes(5);
    let trip_id = Uuid::now_v7();
    let trip = Trip {
        id: trip_id,
        original_id: "test_trip".to_string(),
        vehicle_id: "test_vehicle".to_string(),
        route_id: "1".to_string(),
        direction: 1,
        created_at: now,
        updated_at: now,
        data: TripData::MtaSubway,
    };

    let stop_time = StopTime {
        trip_id,
        stop_id: "101".to_string(),
        arrival,
        departure: arrival,
        data: StopTimeData::MtaSubway(backend::models::trip::MtaSubwayStopTimeData {
            scheduled_track: None,
            actual_track: None,
        }),
    };

    // For this test, I'll just save the trip without stop times to avoid needing too many FKs
    // Wait, TripStore::save_all takes a slice of (Trip, Vec<StopTime>)

    // Let's seed a stop too
    let stop_store = backend::stores::stop::StopStore::new(pool.clone(), redis_pool.clone());
    let stop = backend::models::stop::Stop {
        id: "101".to_string(),
        name: "Test Stop".to_string(),
        geom: geo::Point::new(0.0, 0.0).into(),
        transfers: vec![],
        data: backend::models::stop::StopData::MtaSubway(
            backend::models::stop::MtaSubwayStopData {
                ada: true,
                north_headsign: "North".to_string(),
                south_headsign: "South".to_string(),
                notes: None,
                borough: backend::models::stop::Borough::Manhattan,
            },
        ),
        routes: vec![],
    };
    stop_store
        .save_all(source, &[stop])
        .await
        .expect("Failed to seed stop");

    let data = vec![(trip.clone(), vec![stop_time])];
    trip_store
        .save_all(source, &data)
        .await
        .expect("Failed to save trip");

    // 3. Verify
    let saved_trips = trip_store
        .get_all(source, None)
        .await
        .expect("Failed to get trips");
    assert!(!saved_trips.is_empty());
    assert_eq!(saved_trips[0].original_id, "test_trip");
}
