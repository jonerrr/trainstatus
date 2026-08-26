use backend::{
    feed::vehicle_position::OccupancyStatus,
    integrations::gtfs_realtime::remap_realtime_stop_ids,
    models::{
        position::{NjtBusPositionData, PositionData, VehiclePosition},
        source::Source,
        trip::{NjtBusData, StopTime, StopTimeData, Trip, TripData},
    },
    stores::static_cache::StaticCacheStore,
};
use chrono::Utc;
use std::collections::HashMap;
use uuid::Uuid;

use crate::common::{flush_redis, setup_redis};

fn trip() -> Trip {
    let now = Utc::now();
    Trip {
        id: Uuid::now_v7(),
        original_id: "fixture_trip".to_string(),
        vehicle_id: "fixture_vehicle".to_string(),
        route_id: "1".to_string(),
        shape_ids: vec![],
        direction: 0,
        created_at: now,
        updated_at: now,
        data: TripData::NjtBus(NjtBusData {
            deviation: None,
            headsign: "Fixture".to_string(),
        }),
    }
}

fn stop_time(stop_id: &str) -> StopTime {
    let now = Utc::now();
    StopTime {
        trip_id: Uuid::now_v7(),
        stop_id: stop_id.to_string(),
        arrival: now,
        departure: now,
        data: StopTimeData::NjtBus,
    }
}

fn vehicle(stop_id: Option<&str>) -> VehiclePosition {
    VehiclePosition {
        vehicle_id: "fixture_vehicle".to_string(),
        trip_id: None,
        stop_id: stop_id.map(str::to_string),
        updated_at: Utc::now(),
        data: PositionData::NjtBus(NjtBusPositionData {
            occupancy_status: OccupancyStatus::default(),
        }),
        geom: None,
    }
}

/// A published remap rewrites collapsed child gate ids to their canonical stop,
/// leaves unmapped ids untouched, and covers both stop_times and positions.
#[tokio::test]
async fn remaps_child_stop_ids_to_canonical() {
    let redis_pool = setup_redis().await;
    flush_redis(&redis_pool).await;
    let cache = StaticCacheStore::new(redis_pool);

    // 16957 is a gate child collapsed into representative 16339; 500 is a plain
    // 1:1 stop with no remap entry.
    let mut remap = HashMap::new();
    remap.insert("16957".to_string(), "16339".to_string());
    cache.set_stop_remap(Source::NjtBus, remap);

    let mut data = vec![(trip(), vec![stop_time("16957"), stop_time("500")])];
    let mut positions = vec![vehicle(Some("16957")), vehicle(Some("500")), vehicle(None)];

    remap_realtime_stop_ids(Source::NjtBus, &cache, &mut data, &mut positions);

    let stop_times = &data[0].1;
    assert_eq!(
        stop_times[0].stop_id, "16339",
        "gate child -> representative"
    );
    assert_eq!(stop_times[1].stop_id, "500", "unmapped id passes through");

    assert_eq!(positions[0].stop_id.as_deref(), Some("16339"));
    assert_eq!(positions[1].stop_id.as_deref(), Some("500"));
    assert_eq!(positions[2].stop_id, None, "None stop_id stays None");
}

/// A source with no published remap resolves every id to identity at no cost.
#[tokio::test]
async fn source_without_remap_is_identity() {
    let redis_pool = setup_redis().await;
    flush_redis(&redis_pool).await;
    let cache = StaticCacheStore::new(redis_pool);

    // Only NjtBus has a remap; MtaBus has none.
    cache.set_stop_remap(
        Source::NjtBus,
        HashMap::from([("16957".to_string(), "16339".to_string())]),
    );

    let mut data = vec![(trip(), vec![stop_time("16957")])];
    let mut positions = vec![vehicle(Some("16957"))];

    remap_realtime_stop_ids(Source::MtaBus, &cache, &mut data, &mut positions);

    assert_eq!(
        data[0].1[0].stop_id, "16957",
        "no remap for source -> identity"
    );
    assert_eq!(positions[0].stop_id.as_deref(), Some("16957"));
}
