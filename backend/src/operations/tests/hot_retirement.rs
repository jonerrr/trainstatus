use crate::operations::active::{
    ArchivedRun, apply_archived_shadow, drop_run, prune_stale_positions, runs_for_window,
};
use crate::operations::identity::{RunId, RunRef};
use crate::operations::types::position::{NjtBusPositionData, PositionData, VehicleObservation};
use crate::operations::types::trip::{NjtBusData, StopTimeData, StopVisit, TripData, TripRun};
use crate::source::Source;
use chrono::{DateTime, TimeZone, Utc};
use std::collections::HashMap;

fn archived_run(provider: &str, at: DateTime<Utc>) -> ArchivedRun {
    let id = RunId::scheduled_bus(Source::NjtBus, provider, at.date_naive());
    ArchivedRun {
        trip: TripRun {
            id,
            original_id: provider.into(),
            vehicle_id: format!("{provider}-bus"),
            route_id: "1".into(),
            shape_ids: vec![],
            direction: 0,
            created_at: at,
            updated_at: at,
            partition_date: at.date_naive(),
            data: TripData::NjtBus(NjtBusData {
                deviation: None,
                headsign: "A".into(),
            }),
        },
        visits: vec![StopVisit {
            trip_id: id,
            stop_sequence: 0,
            stop_id: "STOP".into(),
            arrival: at,
            departure: at,
            data: StopTimeData::NjtBus,
        }],
        gps: Default::default(),
        shapes: vec![],
        min_call: at,
        max_call: at,
    }
}

#[test]
fn retired_run_can_be_reopened_from_archive_without_stale_selection() {
    let t = Utc.with_ymd_and_hms(2026, 6, 1, 10, 0, 0).unwrap();
    let archived = archived_run("retire", t);
    let run_id = archived.trip.id;
    let context = super::support::TempStore::new();
    let mut state = context.active(Source::NjtBus);
    let shadows = HashMap::from([(run_id, archived)]);
    apply_archived_shadow(&mut state, &shadows).unwrap();
    assert_eq!(runs_for_window(&state, t, None).len(), 1);

    drop_run(&mut state, run_id);
    assert!(state.get_trip(&run_id).is_none());
    assert!(runs_for_window(&state, t, None).is_empty());

    apply_archived_shadow(&mut state, &shadows).unwrap();
    assert!(state.get_trip(&run_id).is_some());
    assert_eq!(
        runs_for_window(&state, t, None)
            .iter()
            .map(|trip| trip.id)
            .collect::<Vec<_>>(),
        [run_id]
    );
}

#[test]
fn prune_stale_positions_keeps_recent_unlinked_but_drops_archived_vehicle() {
    let t = Utc.with_ymd_and_hms(2026, 6, 1, 10, 0, 0).unwrap();
    let now = t + chrono::Duration::hours(3);
    let live = archived_run("live", t);
    let live_id = live.trip.id;
    let stale_id = RunId::scheduled_bus(Source::NjtBus, "stale", t.date_naive());
    let context = super::support::TempStore::new();
    let mut state = context.active(Source::NjtBus);
    apply_archived_shadow(&mut state, &HashMap::from([(live_id, live)])).unwrap();

    let obs = |vehicle: &str, trip_id: Option<RunId>, at: DateTime<Utc>| VehicleObservation {
        vehicle_id: vehicle.into(),
        trip_id: trip_id.map(|id| RunRef {
            id,
            partition_date: t.date_naive(),
        }),
        stop_id: None,
        updated_at: at,
        data: PositionData::NjtBus(NjtBusPositionData {
            occupancy_status: Default::default(),
        }),
        geom: None,
    };
    state
        .restore_positions(vec![
            obs("recent-unlinked", None, now - chrono::Duration::minutes(30)),
            obs("live-bus", Some(live_id), t - chrono::Duration::hours(5)),
            obs("stale-bus", Some(stale_id), t - chrono::Duration::hours(5)),
        ])
        .unwrap();
    prune_stale_positions(&mut state, now);
    let positions = state.positions();
    assert!(
        positions
            .iter()
            .any(|obs| obs.vehicle_id == "recent-unlinked")
    );
    assert!(positions.iter().any(|obs| obs.vehicle_id == "live-bus"));
    assert!(!positions.iter().any(|obs| obs.vehicle_id == "stale-bus"));
}
