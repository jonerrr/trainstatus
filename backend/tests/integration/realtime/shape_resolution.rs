use crate::support;

use backend::models::source::Source;
use support::{mta_bus_dataset, test_stores};
use uuid::Uuid;

/// Real B94 stops (from the checked-in Helium fixture) all served by shape
/// `B940017`. Used to build a synthetic realtime trip whose stop sequence
/// unambiguously belongs to that shape.
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

fn collected_trip(route_id: &str, stop_ids: &[&str]) -> backend::realtime::CollectedSnapshot {
    use backend::models::trip::{MtaBusData, StopTime, StopTimeData, Trip, TripData};
    let id = Uuid::now_v7();
    let now = crate::support::fixtures::fixed_time();
    let trip = Trip {
        id,
        original_id: format!("test-trip-{id}"),
        vehicle_id: format!("test-vehicle-{id}"),
        route_id: route_id.into(),
        shape_ids: vec![],
        direction: 0,
        created_at: now,
        updated_at: now,
        data: TripData::MtaBus(MtaBusData { deviation: None }),
    };
    let stops = stop_ids
        .iter()
        .enumerate()
        .map(|(i, stop)| {
            let arrival = now + chrono::Duration::minutes(i as i64 * 2);
            StopTime {
                trip_id: id,
                stop_id: (*stop).into(),
                arrival,
                departure: arrival,
                data: StopTimeData::MtaBus,
            }
        })
        .collect();
    backend::realtime::CollectedSnapshot {
        source: Source::MtaBus,
        trips: vec![(trip, stops)],
        positions: vec![],
    }
}

/// A candidate shape can be geometrically closer to a trip's stops than the
/// real shape (e.g. a coincidence, or here — deliberately — a decoy built
/// exactly through the stop points) while having zero confirmed stop-membership
/// hits. Ingestion must prefer the hit-rich real shape over the
/// distance-closer decoy.
#[sqlx::test]
async fn resolves_shape_by_stop_hits_over_raw_distance(pool: sqlx::PgPool) {
    let stores = test_stores(pool.clone());

    let dataset = mta_bus_dataset();
    stores
        .static_data_store
        .persist(&dataset)
        .await
        .expect("fixture should persist");

    let mut revision = backend::static_data::index::StaticTransitRevision::from_dataset(&dataset);
    revision.shapes.insert(
        DECOY_SHAPE.into(),
        backend::models::geom::Geom::from(geo::LineString::from(B94_STOP_COORDS.to_vec())),
    );
    revision
        .routes
        .get_mut("B94")
        .unwrap()
        .shape_ids
        .push(DECOY_SHAPE.into());
    stores.static_data_store.static_index().publish(revision);
    let committed = stores
        .ingestor
        .ingest(collected_trip("B94", &B94_STOP_IDS))
        .await
        .unwrap();
    assert_eq!(
        committed.trips[0].shape_ids,
        [B94_REAL_SHAPE],
        "membership must beat the geometrically closer decoy"
    );
    assert_eq!(committed.changes.resolved_shapes, 1);
}

/// Before a fresh static import populates `route_stop.data->'shape_ids'`, every
/// candidate has zero hits. Resolution must still fall back to the old average-
/// distance ranking rather than erroring or dropping the trip.
#[sqlx::test]
async fn falls_back_to_distance_when_no_hit_data_exists(pool: sqlx::PgPool) {
    let stores = test_stores(pool.clone());

    let dataset = mta_bus_dataset();
    stores
        .static_data_store
        .persist(&dataset)
        .await
        .expect("fixture should persist");

    let mut revision = backend::static_data::index::StaticTransitRevision::from_dataset(&dataset);
    revision.route_stop_shapes.clear();
    stores.static_data_store.static_index().publish(revision);
    let committed = stores
        .ingestor
        .ingest(collected_trip("B94", &B94_STOP_IDS))
        .await
        .unwrap();
    assert!(
        !committed.trips[0].shape_ids.is_empty(),
        "zero membership hits still resolve a shape through distance"
    );
    assert_eq!(committed.changes.resolved_shapes, 1);
}
