use chrono::{DateTime, Duration, Utc};

use super::support::TempStore;
use crate::{
    network::{assets::ShapeAssets, dataset::NetworkDataset, index::NetworkRevision},
    operations::{
        active::{
            AcceptedPoll, ActiveState, PreparedPoll, apply_accepted, apply_prepared, prepare_poll,
        },
        identity::{RunId, RunRef},
        types::{
            position::{NjtBusPositionData, PositionData, VehicleObservation},
            trip::{NjtBusData, TripData, TripRun},
        },
    },
    source::Source,
};

struct Replay {
    active: ActiveState,
    frames: Vec<Vec<u8>>,
    trip: TripRun,
    revision: NetworkRevision,
    assets: ShapeAssets,
}

impl Replay {
    fn accept(&mut self, vehicle: &str, time: DateTime<Utc>, point: geo::Point<f64>) {
        let position = VehicleObservation {
            vehicle_id: vehicle.into(),
            trip_id: Some(RunRef::from(&self.trip)),
            stop_id: None,
            updated_at: time,
            data: PositionData::NjtBus(NjtBusPositionData {
                occupancy_status: Default::default(),
            }),
            geom: Some(point.into()),
        };
        let PreparedPoll { frame, .. } = prepare_poll(
            &self.active,
            &[(self.trip.clone(), vec![])],
            vec![position],
            &self.revision,
            &Default::default(),
            &self.assets,
            &Default::default(),
        )
        .unwrap();
        let mut bytes = Vec::new();
        ciborium::ser::into_writer(&frame, &mut bytes).unwrap();
        self.frames.push(bytes);
        apply_prepared(&mut self.active, frame);
    }
}

#[test]
fn sealed_history_preserves_late_fixes_first_coordinates_and_future_exclusion_on_replay() {
    let context = TempStore::new();
    let at = DateTime::from_timestamp(1_700_000_000, 0).unwrap();
    let id = RunId::scheduled_bus(Source::NjtBus, "long-gps", at.date_naive());
    let trip = TripRun {
        id,
        original_id: "long-gps".into(),
        vehicle_id: "bus".into(),
        route_id: "1".into(),
        shape_ids: vec![],
        direction: 0,
        created_at: at,
        updated_at: at,
        partition_date: at.date_naive(),
        data: TripData::NjtBus(NjtBusData {
            deviation: None,
            headsign: "Terminus".into(),
        }),
    };
    let revision = NetworkRevision::from_dataset(&NetworkDataset::new(Source::NjtBus));
    let assets = ShapeAssets::open_root(&context.root, Source::NjtBus).unwrap();
    let mut replay = Replay {
        active: context.active(Source::NjtBus),
        frames: Vec::new(),
        trip,
        revision,
        assets,
    };
    for offset in 0..513 {
        replay.accept(
            "bus",
            at + Duration::seconds(offset),
            geo::Point::new(-74.0 + offset as f64 / 1_000_000.0, 40.7),
        );
    }
    let late_at = at + Duration::milliseconds(300_500);
    let late_point = geo::Point::new(-73.5, 40.8);
    replay.accept("late-report", late_at, late_point);
    for offset in 513..769 {
        replay.accept(
            "bus",
            at + Duration::seconds(offset),
            geo::Point::new(-74.0 + offset as f64 / 1_000_000.0, 40.7),
        );
    }
    // A third vehicle cannot rewrite a coordinate already present in an overlapping sealed page.
    replay.accept("duplicate-report", late_at, geo::Point::new(-72.0, 42.0));
    let mut recovered = context.active(Source::NjtBus);
    for frame in replay.frames {
        let frame: AcceptedPoll = ciborium::de::from_reader(frame.as_slice()).unwrap();
        apply_accepted(&mut recovered, &frame, &[]).unwrap();
    }
    let series = &recovered.gps_by_run[&id];
    let mut expected: Vec<_> = (0..769)
        .map(|offset| at + Duration::seconds(offset))
        .collect();
    expected.push(late_at);
    expected.sort_unstable();
    assert_eq!(
        recovered
            .gps_store
            .all(series)
            .unwrap()
            .iter()
            .map(|sample| sample.observed_at)
            .collect::<Vec<_>>(),
        expected
    );
    let late = recovered
        .gps_store
        .latest(series, late_at)
        .unwrap()
        .unwrap();
    assert_eq!(late.observed_at, late_at);
    assert_eq!(late.point, late_point);
    assert_eq!(
        recovered
            .gps_store
            .latest(series, late_at - Duration::microseconds(1))
            .unwrap()
            .unwrap()
            .observed_at,
        at + Duration::seconds(300)
    );
    assert_eq!(
        recovered
            .gps_store
            .latest(series, at + Duration::seconds(400))
            .unwrap()
            .unwrap()
            .observed_at,
        at + Duration::seconds(400)
    );
    assert!(
        recovered
            .gps_store
            .latest(series, at - Duration::microseconds(1))
            .unwrap()
            .is_none()
    );
}
