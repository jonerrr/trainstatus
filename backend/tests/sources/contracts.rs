use backend::models::{
    source::Source,
    trip::{MtaSubwayStopTimeData, MtaSubwayTripData, StopTime, StopTimeData, Trip, TripData},
};
use chrono::Utc;
use uuid::Uuid;

use crate::common::{
    contracts, flush_redis, mta_bus_dataset, mta_subway_dataset, setup_redis, test_stores,
};

#[test]
fn static_fixture_datasets_satisfy_shared_contracts() {
    for dataset in [mta_subway_dataset(), mta_bus_dataset()] {
        contracts::assert_static_dataset_contract(&dataset);
    }
}

#[sqlx::test]
async fn static_fixture_datasets_persist_idempotently_and_support_trip_store(pool: sqlx::PgPool) {
    let redis_pool = setup_redis().await;
    flush_redis(&redis_pool).await;
    let stores = test_stores(pool, redis_pool);

    let subway = mta_subway_dataset();
    contracts::assert_static_persistence_contract(&subway, &stores).await;
    contracts::assert_static_persistence_contract(&mta_bus_dataset(), &stores).await;

    let now = Utc::now();
    let trip_id = Uuid::now_v7();
    let trip = Trip {
        id: trip_id,
        original_id: "test_trip".into(),
        vehicle_id: "test_vehicle".into(),
        route_id: "A".into(),
        shape_ids: vec!["A_shape".into()],
        direction: 1,
        created_at: now,
        updated_at: now,
        data: TripData::MtaSubway(MtaSubwayTripData {
            consist: None,
            consist_cars: vec![],
        }),
    };

    let stop_time = StopTime {
        trip_id,
        stop_id: "101".into(),
        arrival: now + chrono::Duration::minutes(5),
        departure: now + chrono::Duration::minutes(5),
        data: StopTimeData::MtaSubway(MtaSubwayStopTimeData {
            scheduled_track: None,
            actual_track: None,
            platform_edges: vec![],
        }),
    };

    stores
        .trip_store
        .save_all(Source::MtaSubway, &[(trip, vec![stop_time])])
        .await
        .expect("trip should save");

    let saved = stores
        .trip_store
        .get_all(Source::MtaSubway, None)
        .await
        .expect("trips should load");
    assert_eq!(saved.len(), 1);
    assert_eq!(saved[0].original_id, "test_trip");
}
