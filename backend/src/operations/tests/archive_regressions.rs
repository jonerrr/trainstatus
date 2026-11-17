use crate::operations::active::ArchivedRun;
use crate::operations::archive::ArchiveStore;
use crate::operations::identity::RunId;
use crate::operations::types::trip::{NjtBusData, StopTimeData, StopVisit, TripData, TripRun};
use crate::source::Source;
use chrono::{NaiveDate, TimeZone, Utc};
use std::collections::HashMap;

fn archived_trip(id: RunId, partition: NaiveDate, call: chrono::DateTime<Utc>) -> ArchivedRun {
    let trip = TripRun {
        id,
        original_id: "archived".into(),
        vehicle_id: "bus".into(),
        route_id: "1".into(),
        shape_ids: vec![],
        direction: 0,
        created_at: call,
        updated_at: call,
        partition_date: partition,
        data: TripData::NjtBus(NjtBusData {
            deviation: None,
            headsign: "Late".into(),
        }),
    };
    let visit = StopVisit {
        trip_id: id,
        stop_id: "STOP1".into(),
        stop_sequence: 1,
        arrival: call,
        departure: call + chrono::Duration::hours(2),
        data: StopTimeData::NjtBus,
    };
    ArchivedRun {
        trip: trip.clone(),
        visits: vec![visit],
        gps: Default::default(),
        shapes: vec![],
        min_call: call,
        max_call: call + chrono::Duration::hours(2),
    }
}

#[test]
fn archived_gps_uses_its_own_window_even_when_all_calls_are_in_the_future() {
    let dir = std::env::temp_dir().join(format!("archive-gps-{}", uuid::Uuid::new_v4()));
    let at = Utc.with_ymd_and_hms(2026, 6, 1, 8, 0, 0).unwrap();
    let id = RunId::scheduled_bus(Source::NjtBus, "gps-window", at.date_naive());
    let mut run = archived_trip(id, at.date_naive(), at + chrono::Duration::hours(2));
    let expected_at = at - chrono::Duration::minutes(3);
    let gps_store = crate::operations::gps::GpsStore::open(dir.join("blocks")).unwrap();
    run.gps = super::support::series(
        &gps_store,
        [
            crate::operations::history::GpsSample {
                observed_at: at - chrono::Duration::minutes(6),
                point: geo::Point::new(-75.0, 40.0),
            },
            crate::operations::history::GpsSample {
                observed_at: expected_at,
                point: geo::Point::new(-74.0, 40.7),
            },
            crate::operations::history::GpsSample {
                observed_at: at + chrono::Duration::seconds(1),
                point: geo::Point::new(-73.0, 41.0),
            },
        ],
    );
    let trip = run.trip.clone();
    let mut store = ArchiveStore::open(dir.clone(), Source::NjtBus, gps_store).unwrap();
    store
        .append_batch(crate::operations::archive::ArchiveBatch {
            source: Source::NjtBus,
            runs: vec![run],
            dictionary: crate::operations::active::RetainedDictionary {
                routes: HashMap::new(),
                stops: HashMap::new(),
            },
        })
        .unwrap();
    let anchors = store.gps_anchors(at, &[trip]).unwrap();
    let anchor = &anchors[&id];
    assert_eq!(anchor.observed_at, expected_at);
    assert_eq!(anchor.point, geo::Point::new(-74.0, 40.7));
    drop(store);
    std::fs::remove_dir_all(dir).unwrap();
}
