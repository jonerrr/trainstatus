use crate::geometry::Geom;
use crate::network::assets::ShapeAssets;
use crate::network::dataset::NetworkDataset;
use crate::network::index::NetworkRevision;
use crate::network::types::shape::Shape;
use crate::operations::active::{
    AcceptedPoll, ActiveState, DeltaPayload, PreparedPoll, apply_accepted, apply_prepared,
    prepare_poll,
};
use crate::operations::identity::{RunId, RunRef};
use crate::operations::types::position::{NjtBusPositionData, PositionData, VehicleObservation};
use crate::operations::types::trip::{NjtBusData, StopTimeData, StopVisit, TripData, TripRun};
use crate::source::Source;
use chrono::{TimeZone, Utc};

#[test]
fn apply_accepted_replays_retained_run() {
    let run_id = RunId::scheduled_bus(
        Source::NjtBus,
        "trip-replay",
        chrono::NaiveDate::from_ymd_opt(2026, 2, 1).unwrap(),
    );
    let t = Utc.with_ymd_and_hms(2026, 2, 1, 12, 0, 0).unwrap();
    let trip = TripRun {
        id: run_id,
        original_id: "trip-replay".into(),
        vehicle_id: "bus-9".into(),
        route_id: "1".into(),
        shape_ids: vec!["shape".into()],
        direction: 0,
        created_at: t,
        updated_at: t,
        partition_date: t.date_naive(),
        data: TripData::NjtBus(NjtBusData {
            deviation: None,
            headsign: "Replay".into(),
        }),
    };
    let visit = StopVisit {
        trip_id: run_id,
        stop_id: "STOP1".into(),
        stop_sequence: 1,
        arrival: t,
        departure: t,
        data: StopTimeData::NjtBus,
    };
    let root = std::env::temp_dir().join(format!("ops-replay-{}", uuid::Uuid::new_v4()));
    let assets = ShapeAssets::open_root(&root, Source::NjtBus).unwrap();
    let mut dataset = NetworkDataset::new(Source::NjtBus);
    dataset.shapes.push(Shape {
        id: "shape".into(),
        source: Source::NjtBus,
        geom: Geom::from(geo::LineString::from(vec![(-74.0, 40.7), (-73.9, 40.8)])),
    });
    let revision = NetworkRevision::from_dataset(&dataset);
    let gps = crate::operations::gps::GpsStore::open(root.join("blocks")).unwrap();
    let mut accepted = ActiveState::new(Source::NjtBus, gps.clone());
    let mut recovered = ActiveState::new(Source::NjtBus, gps);
    let PreparedPoll { frame, .. } = prepare_poll(
        &accepted,
        &[(trip.clone(), vec![visit.clone()])],
        vec![],
        &revision,
        &Default::default(),
        &assets,
        &Default::default(),
    )
    .unwrap();
    apply_prepared(&mut accepted, frame.clone());
    let mut bytes = Vec::new();
    ciborium::ser::into_writer(&frame, &mut bytes).unwrap();
    let frame: AcceptedPoll = ciborium::de::from_reader(bytes.as_slice()).unwrap();
    apply_accepted(&mut recovered, &frame, &[]).unwrap();

    assert_eq!(recovered.get_trip(&run_id).unwrap().id, run_id);
    assert_eq!(recovered.max_departure(&run_id), Some(t));
    assert_eq!(recovered.shapes_for(&run_id).unwrap().len(), 1);
    let PreparedPoll { frame: repeat, .. } = prepare_poll(
        &recovered,
        &[(trip, vec![visit])],
        vec![],
        &revision,
        &Default::default(),
        &assets,
        &Default::default(),
    )
    .unwrap();
    let DeltaPayload::NjtBus(delta) = &repeat.payload else {
        panic!("wrong source")
    };
    assert!(delta.shadows.is_empty());
    assert!(delta.runs.is_empty());
    assert!(delta.positions.is_empty());
    assert!(delta.gps.is_empty());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn accepted_poll_replay_preserves_gps_history_and_equal_time_enrichment() {
    let at = Utc.with_ymd_and_hms(2026, 4, 1, 12, 0, 0).unwrap();
    let run_id = RunId::scheduled_bus(Source::NjtBus, "gps-repeat", at.date_naive());
    let revision = NetworkRevision::from_dataset(&NetworkDataset::new(Source::NjtBus));
    let trip = TripRun {
        id: run_id,
        original_id: "gps-repeat".into(),
        vehicle_id: "bus-1".into(),
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
    };
    let first = geo::Point::new(-74.0, 40.7);
    let newest = geo::Point::new(-73.9, 40.8);
    let observations = [
        (at, first, false),
        (at, geo::Point::new(-75.0, 41.0), true),
        (at + chrono::Duration::seconds(1), newest, false),
        (
            at - chrono::Duration::seconds(1),
            geo::Point::new(-76.0, 42.0),
            false,
        ),
    ];
    let root = std::env::temp_dir().join(format!("ops-gps-replay-{}", uuid::Uuid::new_v4()));
    let assets = ShapeAssets::open_root(&root, Source::NjtBus).unwrap();
    let gps = crate::operations::gps::GpsStore::open(root.join("blocks")).unwrap();
    let mut accepted = ActiveState::new(Source::NjtBus, gps.clone());
    let mut recovered = ActiveState::new(Source::NjtBus, gps);
    for (index, (updated_at, point, full)) in observations.into_iter().enumerate() {
        let position = VehicleObservation {
            vehicle_id: "bus-1".into(),
            trip_id: Some(RunRef::from(&trip)),
            stop_id: None,
            updated_at,
            data: PositionData::NjtBus(NjtBusPositionData {
                occupancy_status: if full {
                    crate::operations::types::position::OccupancyStatus::Full
                } else {
                    Default::default()
                },
            }),
            geom: Some(Geom::from(point)),
        };
        let PreparedPoll { frame, .. } = prepare_poll(
            &accepted,
            &[(trip.clone(), vec![])],
            vec![position],
            &revision,
            &Default::default(),
            &assets,
            &Default::default(),
        )
        .unwrap();
        apply_prepared(&mut accepted, frame.clone());
        let mut bytes = Vec::new();
        ciborium::ser::into_writer(&frame, &mut bytes).unwrap();
        let frame: AcceptedPoll = ciborium::de::from_reader(bytes.as_slice()).unwrap();
        apply_accepted(&mut recovered, &frame, &[]).unwrap();
        if index == 1 {
            let vehicle = recovered
                .positions()
                .into_iter()
                .find(|p| p.vehicle_id == "bus-1")
                .unwrap();
            assert_eq!(vehicle.geom, Some(Geom::from(first)));
            assert_eq!(
                vehicle.data,
                PositionData::NjtBus(NjtBusPositionData {
                    occupancy_status: crate::operations::types::position::OccupancyStatus::Full,
                })
            );
        }
    }
    let samples: Vec<_> = recovered
        .gps_store
        .all(&recovered.gps_by_run[&run_id])
        .unwrap()
        .into_iter()
        .map(|sample| (sample.observed_at, sample.point))
        .collect();
    assert_eq!(
        samples,
        [(at, first), (at + chrono::Duration::seconds(1), newest)]
    );
    assert_eq!(
        recovered
            .positions()
            .into_iter()
            .find(|p| p.vehicle_id == "bus-1")
            .unwrap()
            .geom,
        Some(Geom::from(newest))
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn unresolved_shape_aliases_survive_wal_without_stale_geometry() {
    let at = Utc.with_ymd_and_hms(2026, 4, 1, 12, 0, 0).unwrap();
    let run_id = RunId::scheduled_bus(Source::NjtBus, "unresolved-shape", at.date_naive());
    let geometry = Geom::from(geo::LineString::from(vec![(-74.0, 40.7), (-73.9, 40.8)]));
    let mut dataset = NetworkDataset::new(Source::NjtBus);
    dataset.shapes.push(Shape {
        id: "known".into(),
        source: Source::NjtBus,
        geom: geometry.clone(),
    });
    let revision = NetworkRevision::from_dataset(&dataset);
    let mut trip = TripRun {
        id: run_id,
        original_id: "unresolved-shape".into(),
        vehicle_id: "bus-1".into(),
        route_id: "1".into(),
        shape_ids: vec!["known".into()],
        direction: 0,
        created_at: at,
        updated_at: at,
        partition_date: at.date_naive(),
        data: TripData::NjtBus(NjtBusData {
            deviation: None,
            headsign: "A".into(),
        }),
    };
    let root = std::env::temp_dir().join(format!("ops-shape-replay-{}", uuid::Uuid::new_v4()));
    let assets = ShapeAssets::open_root(&root, Source::NjtBus).unwrap();
    let gps = crate::operations::gps::GpsStore::open(root.join("blocks")).unwrap();
    let mut accepted = ActiveState::new(Source::NjtBus, gps.clone());
    let mut recovered = ActiveState::new(Source::NjtBus, gps);
    for alias in ["known", "unknown"] {
        trip.shape_ids = vec![alias.into()];
        let PreparedPoll {
            frame,
            trips,
            network_revision,
            ..
        } = prepare_poll(
            &accepted,
            &[(trip.clone(), vec![])],
            vec![],
            &revision,
            &Default::default(),
            &assets,
            &Default::default(),
        )
        .unwrap();
        apply_prepared(&mut accepted, frame.clone());
        let mut bytes = Vec::new();
        ciborium::ser::into_writer(&frame, &mut bytes).unwrap();
        let frame: AcceptedPoll = ciborium::de::from_reader(bytes.as_slice()).unwrap();
        apply_accepted(&mut recovered, &frame, &[]).unwrap();
        assert_eq!(recovered.get_trip(&run_id).unwrap().shape_ids, [alias]);
        if alias == "unknown" {
            assert!(trips[0].shape_ids.is_empty());
        } else {
            let selected = network_revision
                .shape(&trips[0].shape_ids[0])
                .unwrap()
                .unwrap();
            assert_eq!(selected.as_ref(), &geometry);
        }
        assert_eq!(
            recovered.shapes_for(&run_id).unwrap().is_empty(),
            alias == "unknown"
        );
    }

    // A partly available connected chain must not masquerade as complete geometry.
    trip.shape_ids = vec!["known".into(), "unknown".into()];
    let PreparedPoll { frame, trips, .. } = prepare_poll(
        &accepted,
        &[(trip, vec![])],
        vec![],
        &revision,
        &Default::default(),
        &assets,
        &Default::default(),
    )
    .unwrap();
    apply_accepted(&mut recovered, &frame, &[]).unwrap();
    assert!(trips[0].shape_ids.is_empty());
    assert_eq!(
        recovered.get_trip(&run_id).unwrap().shape_ids,
        ["known", "unknown"]
    );
    assert!(recovered.shapes_for(&run_id).unwrap().is_empty());
    std::fs::remove_dir_all(root).unwrap();
}
