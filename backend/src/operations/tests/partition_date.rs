use crate::network::assets::ShapeAssets;
use crate::network::dataset::NetworkDataset;
use crate::network::index::NetworkRevision;
use crate::operations::active::{ActiveState, PreparedPoll, apply_prepared, prepare_poll};
use crate::operations::identity::RunId;
use crate::operations::types::trip::{NjtBusData, TripData, TripRun};
use crate::source::Source;
use chrono::{NaiveDate, TimeZone, Utc};

#[test]
fn metadata_corrections_preserve_gtfs_partition_and_run_identity() {
    let service_day = NaiveDate::from_ymd_opt(2026, 3, 1).unwrap();
    let created = Utc.with_ymd_and_hms(2026, 3, 2, 3, 0, 0).unwrap();
    let run_id = RunId::scheduled_bus(Source::NjtBus, "27h", service_day);
    let mut trip = TripRun {
        id: run_id,
        original_id: "27h".into(),
        vehicle_id: "bus".into(),
        route_id: "1".into(),
        shape_ids: vec![],
        direction: 0,
        created_at: created,
        updated_at: created,
        partition_date: service_day,
        data: TripData::NjtBus(NjtBusData {
            deviation: None,
            headsign: "A".into(),
        }),
    };
    let revision = NetworkRevision::from_dataset(&NetworkDataset::new(Source::NjtBus));
    let root = std::env::temp_dir().join(format!("ops-partition-{}", uuid::Uuid::new_v4()));
    let assets = ShapeAssets::open_root(&root, Source::NjtBus).unwrap();
    let mut state = ActiveState::new(
        Source::NjtBus,
        crate::operations::gps::GpsStore::open(root.join("blocks")).unwrap(),
    );
    let PreparedPoll { frame, .. } = prepare_poll(
        &state,
        &[(trip.clone(), vec![])],
        vec![],
        &revision,
        &Default::default(),
        &assets,
        &Default::default(),
    )
    .unwrap();
    apply_prepared(&mut state, frame);
    assert_eq!(state.get_trip(&run_id).unwrap().partition_date, service_day);
    assert_eq!(state.get_trip(&run_id).unwrap().id, run_id);
    trip.created_at += chrono::Duration::minutes(1);
    trip.direction = 1;
    let PreparedPoll {
        trips: published,
        frame,
        ..
    } = prepare_poll(
        &state,
        &[(trip.clone(), vec![])],
        vec![],
        &revision,
        &Default::default(),
        &assets,
        &Default::default(),
    )
    .unwrap();
    let corrected = &published[0];
    assert_eq!(corrected.id, run_id);
    assert_eq!(corrected.partition_date, service_day);
    assert_eq!(corrected.created_at, trip.created_at);
    assert_eq!(corrected.direction, 1);
    apply_prepared(&mut state, frame);
    assert_eq!(state.get_trip(&run_id).unwrap().partition_date, service_day);
    std::fs::remove_dir_all(root).unwrap();
}
