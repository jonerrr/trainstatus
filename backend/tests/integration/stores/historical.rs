use crate::support::{mta_subway_dataset, test_stores};
use backend::{
    models::{
        source::Source,
        trip::{MtaSubwayStopTimeData, MtaSubwayTripData, StopTime, StopTimeData, Trip, TripData},
    },
    realtime::CollectedSnapshot,
};
use chrono::{Duration as ChronoDuration, TimeZone, Utc};
use uuid::Uuid;

#[sqlx::test]
async fn final_time_queries_use_arrival_or_departure_with_inclusive_window(pool: sqlx::PgPool) {
    let _redis = crate::support::TestRedis::start().await.unwrap();
    let stores = test_stores(pool, _redis.pool());
    mta_subway_dataset()
        .persist(
            &stores.route_store,
            &stores.stop_store,
            &stores.static_cache_store,
        )
        .await
        .expect("static fixture should persist");
    let at = Utc.with_ymd_and_hms(2026, 1, 1, 12, 0, 0).single().unwrap();
    // Removing departure relevance, either inclusive boundary, or restoring an
    // updated_at predicate must break these retained-data assertions.
    let cases = [
        ("arrival-start", 0, 0),
        ("arrival-end", 240, 241),
        ("departure-only", -1, 0),
        ("before-window", -2, -1),
        ("after-window", 241, 242),
    ];
    let trips = cases
        .into_iter()
        .map(|(name, arrival_minutes, departure_minutes)| {
            let trip_id = Uuid::now_v7();
            let trip = Trip {
                id: trip_id,
                original_id: name.into(),
                vehicle_id: name.into(),
                route_id: "A".into(),
                shape_ids: vec![],
                direction: 1,
                created_at: at - ChronoDuration::hours(9),
                updated_at: at - ChronoDuration::hours(8),
                data: TripData::MtaSubway(MtaSubwayTripData {
                    consist: None,
                    consist_cars: vec![],
                }),
            };
            let stop = StopTime {
                trip_id,
                stop_id: "101".into(),
                arrival: at + ChronoDuration::minutes(arrival_minutes),
                departure: at + ChronoDuration::minutes(departure_minutes),
                data: StopTimeData::MtaSubway(MtaSubwayStopTimeData {
                    scheduled_track: None,
                    actual_track: None,
                    platform_edges: vec![],
                }),
            };
            (trip, vec![stop])
        })
        .collect::<Vec<_>>();
    stores
        .ingestor
        .ingest(CollectedSnapshot {
            source: Source::MtaSubway,
            trips,
            positions: vec![],
        })
        .await
        .expect("trips should save");

    let saved = stores
        .trip_store
        .get_all(Source::MtaSubway, Some(at))
        .await
        .unwrap();
    let mut names = saved
        .iter()
        .map(|trip| trip.original_id.as_str())
        .collect::<Vec<_>>();
    names.sort_unstable();
    assert_eq!(names, ["arrival-end", "arrival-start", "departure-only"]);

    let stops = stores
        .stop_time_store
        .get_all(Source::MtaSubway, Some(at), None)
        .await
        .unwrap();
    assert_eq!(stops.len(), 3);
    let mut arrivals = stops
        .iter()
        .map(|stop| (stop.arrival - at).num_minutes())
        .collect::<Vec<_>>();
    arrivals.sort_unstable();
    assert_eq!(arrivals, [-1, 0, 240]);
    assert!(
        stores
            .trip_store
            .get_all(Source::MtaBus, Some(at))
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        stores
            .stop_time_store
            .get_all(Source::MtaBus, Some(at), None)
            .await
            .unwrap()
            .is_empty()
    );
}
