use crate::support::{app_state_from_stores, fixtures::fixed_time, geometry::ingest_case};
use arrow::array::{Array, FixedSizeListArray, Float64Array, Int16Array, ListArray, StringArray};
use axum_test::TestServer;
use backend::models::source::Source;

async fn assert_trajectory(pool: sqlx::PgPool, source: Source, expected_units: usize) {
    let (stores, _cache) = ingest_case(pool.clone(), source).await;
    let trip_id = stores.live_snapshots.get(source).unwrap().trips[0]
        .id
        .to_string();
    let state = app_state_from_stores(pool, stores);
    let cache = state.trajectory_cache.clone();
    let server = TestServer::new(backend::api::router(state).split_for_parts().0);
    let response = server
        .get(&format!(
            "/trajectories/{source}?at={}",
            fixed_time().timestamp()
        ))
        .await;
    response.assert_status_ok();
    assert_eq!(
        response.headers()[http::header::CONTENT_TYPE],
        "application/vnd.apache.arrow.stream"
    );
    let batches =
        arrow::ipc::reader::StreamReader::try_new(std::io::Cursor::new(response.as_bytes()), None)
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
    assert_eq!(
        batches.iter().map(|batch| batch.num_rows()).sum::<usize>(),
        expected_units,
        "{source} render units"
    );
    for batch in &batches {
        let sources = batch
            .column_by_name("source")
            .unwrap()
            .as_any()
            .downcast_ref::<StringArray>()
            .unwrap();
        let trips = batch
            .column_by_name("trip_id")
            .unwrap()
            .as_any()
            .downcast_ref::<StringArray>()
            .unwrap();
        let icons = batch
            .column_by_name("icon_key")
            .unwrap()
            .as_any()
            .downcast_ref::<StringArray>()
            .unwrap();
        let counts = batch
            .column_by_name("unit_count")
            .unwrap()
            .as_any()
            .downcast_ref::<Int16Array>()
            .unwrap();
        let times = batch
            .column_by_name("timestamps")
            .unwrap()
            .as_any()
            .downcast_ref::<ListArray>()
            .unwrap();
        let paths = batch
            .column_by_name("positions")
            .unwrap()
            .as_any()
            .downcast_ref::<ListArray>()
            .unwrap();
        for row in 0..batch.num_rows() {
            assert_eq!(sources.value(row), source.as_str());
            assert_eq!(trips.value(row), trip_id);
            if source == Source::MtaSubway {
                assert_eq!(counts.value(row), 8);
                assert!(matches!(icons.value(row), "rail_head" | "rail_car"));
            } else {
                assert!(counts.is_null(row));
                assert_eq!(icons.value(row), "bus");
            }
            let time_values = times.value(row);
            let time_values = time_values.as_any().downcast_ref::<Float64Array>().unwrap();
            assert!(time_values.len() > 1);
            assert!(time_values.values().iter().all(|t| t.is_finite()));
            assert!(time_values.values().windows(2).all(|w| w[0] <= w[1]));
            let path = paths.value(row);
            let pairs = path.as_any().downcast_ref::<FixedSizeListArray>().unwrap();
            assert_eq!(pairs.len(), time_values.len());
            let coords = pairs
                .values()
                .as_any()
                .downcast_ref::<Float64Array>()
                .unwrap();
            assert!(coords.values().iter().all(|v| v.is_finite()));
            assert!(
                coords
                    .values()
                    .chunks_exact(2)
                    .collect::<Vec<_>>()
                    .windows(2)
                    .any(|w| w[0] != w[1]),
                "{source} moving trajectory"
            );
        }
    }
    // Exercise the live transport/filter path using the computed render generation.
    let hot = cache.get_historical(source, fixed_time()).await.unwrap();
    cache.set_hot(source, hot).await;
    let live = server.get(&format!("/trajectories/{source}")).await;
    live.assert_status_ok();
    let live_count =
        arrow::ipc::reader::StreamReader::try_new(std::io::Cursor::new(live.as_bytes()), None)
            .unwrap()
            .map(|batch| batch.unwrap().num_rows())
            .sum::<usize>();
    assert_eq!(live_count, expected_units);
    let excluded = server
        .get(&format!("/trajectories/{source}?route_ids=missing"))
        .await;
    excluded.assert_status_ok();
    let count =
        arrow::ipc::reader::StreamReader::try_new(std::io::Cursor::new(excluded.as_bytes()), None)
            .unwrap()
            .map(|batch| batch.unwrap().num_rows())
            .sum::<usize>();
    assert_eq!(count, 0);
}

#[sqlx::test]
async fn subway_segments_render_one_arrow_unit_per_car(pool: sqlx::PgPool) {
    assert_trajectory(pool, Source::MtaSubway, 8).await;
}
#[sqlx::test]
async fn mta_bus_route_renders_one_moving_arrow_unit(pool: sqlx::PgPool) {
    assert_trajectory(pool, Source::MtaBus, 1).await;
}
#[sqlx::test]
async fn njt_bus_route_renders_one_moving_arrow_unit(pool: sqlx::PgPool) {
    assert_trajectory(pool, Source::NjtBus, 1).await;
}
