use chrono::{Duration, TimeZone, Utc};
use std::collections::HashMap;

use crate::network::assets::ShapeAssets;
use crate::network::dataset::NetworkDataset;
use crate::network::index::NetworkRevision;
use crate::operations::active::{
    ActiveState, ArchivedRun, PreparedPoll, apply_archived_shadow, apply_prepared, prepare_poll,
    runs_for_window,
};
use crate::operations::identity::RunId;
use crate::operations::types::trip::{NjtBusData, StopTimeData, StopVisit, TripData, TripRun};
use crate::source::Source;

#[test]
fn corrected_call_bounds_replace_the_old_interval_without_duplicating_runs() {
    let at = Utc.with_ymd_and_hms(2026, 10, 6, 12, 0, 0).unwrap();
    let root = std::env::temp_dir().join(format!("ops-time-index-{}", uuid::Uuid::new_v4()));
    let mut state = ActiveState::new(
        Source::NjtBus,
        crate::operations::gps::GpsStore::open(root.join("blocks")).unwrap(),
    );
    let trips: Vec<_> = ["corrected", "untouched"]
        .into_iter()
        .map(|provider| TripRun {
            id: RunId::scheduled_bus(Source::NjtBus, provider, at.date_naive()),
            original_id: provider.into(),
            vehicle_id: provider.into(),
            route_id: "1".into(),
            shape_ids: vec![],
            direction: 0,
            created_at: at,
            updated_at: at,
            partition_date: at.date_naive(),
            data: TripData::NjtBus(NjtBusData {
                deviation: None,
                headsign: "Terminal".into(),
            }),
        })
        .collect();
    let visit = |trip: &TripRun, time| StopVisit {
        trip_id: trip.id,
        stop_sequence: 0,
        stop_id: "STOP".into(),
        arrival: time,
        departure: time,
        data: StopTimeData::NjtBus,
    };
    let shadows: HashMap<_, _> = trips
        .iter()
        .map(|trip| {
            (
                trip.id,
                ArchivedRun {
                    trip: trip.clone(),
                    visits: vec![visit(trip, at)],
                    gps: Default::default(),
                    shapes: vec![],
                    min_call: at,
                    max_call: at,
                },
            )
        })
        .collect();
    apply_archived_shadow(&mut state, &shadows).unwrap();

    let revision = NetworkRevision::from_dataset(&NetworkDataset::new(Source::NjtBus));
    let assets = ShapeAssets::open_root(&root, Source::NjtBus).unwrap();
    for seconds in [60, 120] {
        let PreparedPoll { frame, .. } = prepare_poll(
            &state,
            &[(
                trips[0].clone(),
                vec![visit(&trips[0], at + Duration::seconds(seconds))],
            )],
            vec![],
            &revision,
            &HashMap::new(),
            &assets,
            &HashMap::new(),
        )
        .unwrap();
        apply_prepared(&mut state, frame);
    }
    let mut expected: Vec<_> = trips.iter().map(|trip| trip.id).collect();
    expected.sort();
    assert_eq!(
        runs_for_window(&state, at, None)
            .iter()
            .map(|trip| trip.id)
            .collect::<Vec<_>>(),
        expected
    );

    let corrected_at = at + Duration::hours(5);
    let PreparedPoll { frame, .. } = prepare_poll(
        &state,
        &[(trips[0].clone(), vec![visit(&trips[0], corrected_at)])],
        vec![],
        &revision,
        &HashMap::new(),
        &assets,
        &HashMap::new(),
    )
    .unwrap();
    apply_prepared(&mut state, frame);
    assert_eq!(
        runs_for_window(&state, at, None)
            .iter()
            .map(|trip| trip.id)
            .collect::<Vec<_>>(),
        [trips[1].id]
    );
    assert_eq!(
        runs_for_window(&state, corrected_at, None)
            .iter()
            .map(|trip| trip.id)
            .collect::<Vec<_>>(),
        [trips[0].id]
    );
    std::fs::remove_dir_all(root).unwrap();
}
