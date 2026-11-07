use crate::support::app_state_from_owners;
use crate::support::fixtures::fixed_time;
use crate::support::geometry::ingest_case;
use arrow::array::{Array, FixedSizeListArray, Float64Array, Int16Array, ListArray, StringArray};
use axum_test::TestServer;
use backend::source::Source;

async fn assert_trajectory(pool: sqlx::PgPool, source: Source, expected_units: usize) {
    let stores = ingest_case(pool.clone(), source).await;
    let trip_id = stores.live_snapshots.get(source).unwrap().trips[0]
        .id
        .to_string();
    let state = app_state_from_owners(pool, stores);
    let trajectories = state.trajectories.clone();
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
            .column_by_name("motion")
            .unwrap()
            .as_any()
            .downcast_ref::<ListArray>()
            .unwrap();
        let paths = batch
            .column_by_name("shape_positions")
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
            // Shared shape and trip motion occur once, on their first unit row.
            if row == 0 {
                let motion = times.value(row);
                let intervals = motion
                    .as_any()
                    .downcast_ref::<FixedSizeListArray>()
                    .unwrap();
                let values = intervals
                    .values()
                    .as_any()
                    .downcast_ref::<Float64Array>()
                    .unwrap();
                assert!(values.values().iter().all(|v| v.is_finite()));
                assert!(
                    values
                        .values()
                        .as_chunks::<6>()
                        .0
                        .iter()
                        .all(|v| v[1] > v[0])
                );
                let path = paths.value(row);
                let pairs = path.as_any().downcast_ref::<FixedSizeListArray>().unwrap();
                assert!(pairs.len() >= 2);
                let coords = pairs
                    .values()
                    .as_any()
                    .downcast_ref::<Float64Array>()
                    .unwrap();
                assert!(coords.values().iter().all(|v| v.is_finite()));
                assert!(
                    values
                        .values()
                        .as_chunks::<6>()
                        .0
                        .iter()
                        .any(|v| v[3] + v[4] + v[5] > 0.0)
                );
            } else {
                assert_eq!(times.value_length(row), 0);
                assert_eq!(paths.value_length(row), 0);
            }
        }
    }
    // Exercise the live transport path using the computed render generation.
    let hot = trajectories
        .snapshot(source, Some(fixed_time()))
        .await
        .unwrap();
    trajectories.publish_live(source, (*hot).clone()).await;
    let live = server.get(&format!("/trajectories/{source}")).await;
    live.assert_status_ok();
    let live_count =
        arrow::ipc::reader::StreamReader::try_new(std::io::Cursor::new(live.as_bytes()), None)
            .unwrap()
            .map(|batch| batch.unwrap().num_rows())
            .sum::<usize>();
    assert_eq!(live_count, expected_units);
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
