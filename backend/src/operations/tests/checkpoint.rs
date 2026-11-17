use std::{collections::HashMap, sync::Arc};

use chrono::{TimeZone, Utc};

use super::support::{TempStore, series};
use crate::{
    operations::{
        active::{ActiveState, ArchivedRun, apply_archived_shadow, materialize_run},
        checkpoint::{
            LoadedCheckpoint, MANIFEST_NAME, SourceCheckpointMeta, load_active, write_active,
        },
        history::GpsSample,
        identity::{RunId, RunRef},
        types::{
            position::{MtaSubwayPositionData, PositionData, VehicleObservation},
            trip::{
                MtaSubwayStopTimeData, MtaSubwayTripData, StopTimeData, StopVisit, TripData,
                TripRun,
            },
        },
    },
    source::Source,
    storage::{codec::read, journal::JournalRecordId},
};

fn at() -> chrono::DateTime<Utc> {
    Utc.timestamp_opt(1_780_000_000, 0).unwrap()
}

fn snapshot(context: &TempStore, label: &str, stop: &str, longitude: f64) -> (ActiveState, RunId) {
    let at = at();
    let id = RunId::arbitrary(label);
    let mut active = context.active(Source::MtaSubway);
    let gps = series(
        &active.gps_store,
        [GpsSample {
            observed_at: at,
            point: geo::Point::new(longitude, 40.7),
        }],
    );
    apply_archived_shadow(
        &mut active,
        &HashMap::from([(
            id,
            ArchivedRun {
                trip: TripRun {
                    id,
                    original_id: label.into(),
                    vehicle_id: format!("vehicle-{label}"),
                    route_id: "A".into(),
                    shape_ids: vec![],
                    direction: 1,
                    created_at: at,
                    updated_at: at,
                    partition_date: at.date_naive(),
                    data: TripData::MtaSubway(MtaSubwayTripData {
                        consist: None,
                        consist_cars: vec![],
                    }),
                },
                visits: vec![StopVisit {
                    trip_id: id,
                    stop_sequence: 0,
                    stop_id: stop.into(),
                    arrival: at + chrono::Duration::minutes(5),
                    departure: at + chrono::Duration::minutes(6),
                    data: StopTimeData::MtaSubway(MtaSubwayStopTimeData {
                        scheduled_track: None,
                        actual_track: None,
                        platform_edges: vec![],
                    }),
                }],
                gps,
                shapes: vec![],
                min_call: at + chrono::Duration::minutes(5),
                max_call: at + chrono::Duration::minutes(6),
            },
        )]),
    )
    .unwrap();
    active
        .restore_positions(vec![VehicleObservation {
            vehicle_id: format!("vehicle-{label}"),
            trip_id: Some(RunRef {
                id,
                partition_date: at.date_naive(),
            }),
            stop_id: Some(stop.into()),
            updated_at: at,
            data: PositionData::MtaSubway(MtaSubwayPositionData {
                assigned: true,
                status: None,
            }),
            geom: None,
        }])
        .unwrap();
    (active, id)
}

fn assert_snapshot(active: &ActiveState, id: RunId, label: &str, stop: &str, longitude: f64) {
    let trip = active.get_trip(&id).unwrap();
    assert_eq!(trip.original_id, label);
    let retained = materialize_run(active, &trip);
    assert_eq!(retained.visits[0].stop_id, stop);
    assert_eq!(
        active.gps_store.all(&retained.gps).unwrap()[0].point,
        geo::Point::new(longitude, 40.7)
    );
    assert!(
        active
            .positions()
            .iter()
            .any(|position| position.vehicle_id == format!("vehicle-{label}")
                && position.trip_id == Some(id))
    );
}

#[test]
fn failed_checkpoint_preserves_published_data_and_watermark_until_repaired() {
    let context = TempStore::new();
    let root = context.root.join("checkpoint");
    let (mut first, first_id) = snapshot(&context, "run-a", "101", -73.99);
    let first_watermark = JournalRecordId {
        generation: 0,
        sequence: 1,
    };
    write_active(&root, &mut first, first_watermark, 0, at()).unwrap();
    let LoadedCheckpoint {
        active: loaded,
        head,
    } = load_active(&root, Source::MtaSubway, first.gps_store.clone()).unwrap();
    assert_eq!(head.unwrap().watermark, first_watermark);
    assert_snapshot(&loaded, first_id, "run-a", "101", -73.99);
    assert!(load_active(&root, Source::MtaBus, first.gps_store.clone()).is_err());

    let (mut second, second_id) = snapshot(&context, "run-b", "102", -74.01);
    let second_watermark = JournalRecordId {
        generation: 0,
        sequence: 2,
    };
    // A non-directory generation path fails deterministically without changing the live root.
    let fault = root.join("generations/1");
    std::fs::write(&fault, b"blocked").unwrap();
    assert!(write_active(&root, &mut second, second_watermark, 1, at()).is_err());
    let LoadedCheckpoint {
        active: loaded,
        head,
    } = load_active(&root, Source::MtaSubway, first.gps_store.clone()).unwrap();
    assert_eq!(head.unwrap().watermark, first_watermark);
    assert_snapshot(&loaded, first_id, "run-a", "101", -73.99);
    assert!(loaded.get_trip(&second_id).is_none());
    std::fs::remove_file(fault).unwrap();
    write_active(&root, &mut second, second_watermark, 1, at()).unwrap();
    let LoadedCheckpoint {
        active: loaded,
        head,
    } = load_active(&root, Source::MtaSubway, second.gps_store.clone()).unwrap();
    assert_eq!(head.unwrap().watermark, second_watermark);
    assert_snapshot(&loaded, second_id, "run-b", "102", -74.01);
    assert!(loaded.get_trip(&first_id).is_none());
}

#[test]
fn checkpoint_reuses_sealed_history_and_recovers_a_new_partial_page() {
    let context = TempStore::new();
    let root = context.root.join("checkpoint");
    let (mut active, id) = snapshot(&context, "run", "101", -73.99);
    let accepted: Vec<_> = (1..256)
        .map(|offset| {
            (
                id,
                GpsSample {
                    observed_at: at() + chrono::Duration::seconds(offset),
                    point: geo::Point::new(-73.99, 40.7),
                },
            )
        })
        .collect();
    let seals = active
        .gps_store
        .prepare_seals(&active.gps_by_run, &HashMap::new(), &accepted)
        .unwrap();
    for (_, sample) in accepted {
        Arc::make_mut(active.gps_by_run.get_mut(&id).unwrap()).insert(sample);
    }
    for seal in seals {
        active
            .gps_store
            .apply_seal(Arc::make_mut(active.gps_by_run.get_mut(&id).unwrap()), seal);
    }
    let write = |active: &mut ActiveState, sequence| {
        write_active(
            &root,
            active,
            JournalRecordId {
                generation: 0,
                sequence,
            },
            0,
            at(),
        )
        .unwrap();
        read::<SourceCheckpointMeta>(&root.join(MANIFEST_NAME)).unwrap()
    };
    let first = write(&mut active, 1);
    let second = write(&mut active, 2);
    assert_eq!(
        first.runs[0].passenger.location,
        second.runs[0].passenger.location
    );
    let gps_store = active.gps_store.clone();
    let gps_blocks = |gps: &crate::operations::gps::GpsSeries| {
        let mut blocks = std::collections::HashSet::new();
        gps_store.live_blocks(gps, &mut blocks).unwrap();
        blocks
    };
    assert_eq!(gps_blocks(&first.gps[&id]), gps_blocks(&second.gps[&id]));
    let observed_at = at() + chrono::Duration::seconds(256);
    Arc::make_mut(active.gps_by_run.get_mut(&id).unwrap()).insert(GpsSample {
        observed_at,
        point: geo::Point::new(-74.01, 40.7),
    });
    let third = write(&mut active, 3);
    assert_eq!(
        second.runs[0].passenger.location,
        third.runs[0].passenger.location
    );
    assert!(!gps_blocks(&second.gps[&id]).is_disjoint(&gps_blocks(&third.gps[&id])));
    let LoadedCheckpoint {
        active: loaded,
        head,
    } = load_active(&root, Source::MtaSubway, active.gps_store.clone()).unwrap();
    assert_eq!(
        head.unwrap().watermark,
        JournalRecordId {
            generation: 0,
            sequence: 3
        }
    );
    let all = loaded.gps_store.all(&loaded.gps_by_run[&id]).unwrap();
    assert_eq!(
        all.iter()
            .map(|sample| sample.observed_at)
            .collect::<Vec<_>>(),
        (0..257)
            .map(|offset| at() + chrono::Duration::seconds(offset))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        loaded
            .gps_store
            .latest(&loaded.gps_by_run[&id], observed_at)
            .unwrap()
            .unwrap()
            .point,
        geo::Point::new(-74.01, 40.7)
    );
    assert_eq!(
        loaded
            .gps_store
            .latest(&loaded.gps_by_run[&id], at())
            .unwrap()
            .unwrap()
            .point,
        geo::Point::new(-73.99, 40.7)
    );
}

#[test]
fn checkpoint_preserves_gps_without_passenger_record() {
    let context = TempStore::new();
    let root = context.root.join("checkpoint");
    let mut active = context.active(Source::MtaSubway);
    let id = RunId::arbitrary("gps-only");
    let sample = GpsSample {
        observed_at: at(),
        point: geo::Point::new(-73.99, 40.7),
    };
    active
        .gps_by_run
        .insert(id, Arc::new(series(&active.gps_store, [sample.clone()])));
    write_active(
        &root,
        &mut active,
        JournalRecordId {
            generation: 0,
            sequence: 1,
        },
        0,
        at(),
    )
    .unwrap();
    let LoadedCheckpoint { active: loaded, .. } =
        load_active(&root, Source::MtaSubway, active.gps_store.clone()).unwrap();
    let samples = loaded.gps_store.all(&loaded.gps_by_run[&id]).unwrap();
    assert_eq!(samples.len(), 1);
    assert_eq!(samples[0].observed_at, sample.observed_at);
    assert_eq!(samples[0].point, sample.point);
}
