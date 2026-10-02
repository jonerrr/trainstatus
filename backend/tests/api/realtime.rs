use axum_test::TestServer;
use backend::models::{
    source::Source,
    trip::{MtaSubwayStopTimeData, MtaSubwayTripData, StopTime, StopTimeData, Trip, TripData},
};
use backend::realtime::CollectedSnapshot;
use chrono::{Duration, TimeZone, Utc};
use uuid::Uuid;

use crate::common::{app_state, mta_subway_dataset, setup_redis, test_stores};

#[sqlx::test]
async fn explicit_at_uses_final_arrival_time(pool: sqlx::PgPool) {
    let redis_pool = setup_redis().await;
    let stores = test_stores(pool.clone(), redis_pool.clone());
    mta_subway_dataset()
        .persist(
            &stores.route_store,
            &stores.stop_store,
            &stores.static_cache_store,
        )
        .await
        .expect("static fixture should persist");

    let at = Utc.with_ymd_and_hms(2026, 1, 1, 12, 0, 0).single().unwrap();
    let trip_id = Uuid::now_v7();
    let trip = Trip {
        id: trip_id,
        original_id: "historical-final".into(),
        vehicle_id: "historical-vehicle".into(),
        route_id: "A".into(),
        shape_ids: vec![],
        direction: 1,
        created_at: at,
        updated_at: at + Duration::hours(8),
        data: TripData::MtaSubway(MtaSubwayTripData {
            consist: None,
            consist_cars: vec![],
        }),
    };
    let stops: Vec<StopTime> = [("101", Duration::minutes(20)), ("102", Duration::hours(6))]
        .into_iter()
        .map(|(stop_id, offset)| StopTime {
            trip_id,
            stop_id: stop_id.into(),
            arrival: at + offset,
            departure: at + offset,
            data: StopTimeData::MtaSubway(MtaSubwayStopTimeData {
                scheduled_track: None,
                actual_track: None,
                platform_edges: vec![],
            }),
        })
        .collect();
    let departure_trip = Trip {
        id: Uuid::now_v7(),
        original_id: "historical-departure".into(),
        vehicle_id: "departure-vehicle".into(),
        ..trip.clone()
    };
    let departure_stop = StopTime {
        trip_id: departure_trip.id,
        arrival: at - Duration::minutes(1),
        departure: at,
        ..stops[0].clone()
    };
    stores
        .ingestor
        .ingest(CollectedSnapshot {
            source: Source::MtaSubway,
            trips: vec![(trip, stops), (departure_trip, vec![departure_stop])],
            positions: vec![],
        })
        .await
        .expect("retained trip should save");

    let (router, _) = backend::api::router(app_state(pool, redis_pool)).split_for_parts();
    let server = TestServer::new(router);
    let trips = server
        .get(&format!("/trips/mta_subway?at={}", at.timestamp()))
        .await;
    trips.assert_status_ok();
    let trips: Vec<Trip> = trips.json();
    let mut names = trips
        .iter()
        .map(|trip| trip.original_id.as_str())
        .collect::<Vec<_>>();
    names.sort_unstable();
    assert_eq!(names, ["historical-departure", "historical-final"]);

    for route_filter in ["", "&route_ids=A"] {
        let response = server
            .get(&format!(
                "/stop_times/mta_subway?at={}{route_filter}",
                at.timestamp()
            ))
            .await;
        response.assert_status_ok();
        let stops: Vec<StopTime> = response.json();
        assert_eq!(
            stops.len(),
            2,
            "only in-window final arrival/departure stops are relevant"
        );
        assert!(stops.iter().all(|stop| stop.stop_id == "101"));
        assert_eq!(stops[0].arrival, at - Duration::minutes(1));
        assert_eq!(stops[0].departure, at);
    }
    let excluded = server
        .get(&format!(
            "/stop_times/mta_subway?at={}&route_ids=other",
            at.timestamp()
        ))
        .await;
    excluded.assert_status_ok();
    excluded.assert_text("[]");
}

pub async fn assert_realtime_endpoints(server: &TestServer) {
    for source in ["mta_subway", "mta_bus", "njt_bus"] {
        server
            .get(&format!("/trips/{source}"))
            .await
            .assert_status_ok();
        server
            .get(&format!("/trips/{source}?at=1710000000"))
            .await
            .assert_status_ok();
        server
            .get(&format!("/positions/{source}"))
            .await
            .assert_status_ok();
        server
            .get(&format!("/alerts/{source}"))
            .await
            .assert_status_ok();
        server
            .get(&format!("/stop_times/{source}?route_ids=A"))
            .await
            .assert_status_ok();
    }

    for source in ["mta_bus", "njt_bus"] {
        let response = server.get(&format!("/stop_times/{source}")).await;
        response.assert_status_ok();
        response.assert_text("[]");
    }
}
