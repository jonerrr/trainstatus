use crate::geometry::Geom;
use crate::network::dataset::NetworkDataset;
use crate::network::index::{NetworkIndex, NetworkRevision};
use crate::network::types::route::{Route, RouteData};
use crate::network::types::stop::{NjtBusStopData, Stop, StopData};
use crate::operations::{
    identity::RunId,
    ingestor::OperationsIngestor,
    live::LiveSnapshots,
    source::CollectedSnapshot,
    types::{
        position::{NjtBusPositionData, PositionData, VehicleObservation},
        trip::{StopTimeData, StopVisit, TripData, TripRun},
    },
};
use crate::source::Source;
use crate::storage::store::Storage;
use chrono::{DateTime, Utc};
use std::sync::Arc;

fn dataset() -> NetworkDataset {
    let mut dataset = NetworkDataset::new(Source::NjtBus);
    dataset.routes.push(Route {
        id: "1".into(),
        long_name: "Route 1".into(),
        short_name: "1".into(),
        color: "FFFFFF".into(),
        text_color: "000000".into(),
        data: RouteData::NjtBus,
    });
    dataset.stops.push(Stop {
        id: "STOP1".into(),
        name: "Terminal".into(),
        geom: Geom::from(geo::Point::new(-74.0, 40.7)),
        nearby_stops: vec![],
        routes: vec![],
        data: StopData::NjtBus(NjtBusStopData {
            stop_code: "10001".into(),
        }),
    });
    dataset
}

fn temp_storage() -> (Storage, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!("ops-store-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let storage = Storage::open(&dir).unwrap();
    (storage, dir)
}

#[tokio::test]
async fn ingest_persists_runs_and_history_reloads_after_restart() {
    let (storage, dir) = temp_storage();
    let revision = Arc::new(NetworkRevision::from_dataset(&dataset()));
    let index = NetworkIndex::new();
    index.publish_bundle(vec![(*revision).clone()]);
    let live = LiveSnapshots::default();
    let ingestor = OperationsIngestor::new(storage.clone(), live, index.clone()).unwrap();
    let history = ingestor.history();
    let now = DateTime::<Utc>::from_timestamp(1_780_000_000, 0).unwrap();
    let key = RunId::scheduled_bus(Source::NjtBus, "trip-1", now.date_naive());
    let snapshot = CollectedSnapshot {
        source: Source::NjtBus,
        network_revision: Some(revision),
        renormalize: None,
        trips: vec![(
            TripRun {
                id: key,
                original_id: "trip-1".into(),
                vehicle_id: "bus-1".into(),
                route_id: "1".into(),
                shape_ids: vec![],
                direction: 0,
                created_at: now,
                updated_at: now,
                partition_date: now.date_naive(),
                data: TripData::NjtBus(crate::operations::types::trip::NjtBusData {
                    deviation: None,
                    headsign: "Downtown".into(),
                }),
            },
            vec![StopVisit {
                trip_id: key,
                stop_sequence: 1,
                stop_id: "STOP1".into(),
                arrival: now,
                departure: now,
                data: StopTimeData::NjtBus,
            }],
        )],
        positions: vec![VehicleObservation {
            vehicle_id: "bus-1".into(),
            trip_id: Some(crate::operations::identity::RunRef {
                id: key,
                partition_date: now.date_naive(),
            }),
            stop_id: Some("STOP1".into()),
            updated_at: now,
            data: PositionData::NjtBus(NjtBusPositionData {
                occupancy_status: Default::default(),
            }),
            geom: Some(Geom::from(geo::Point::new(-74.0, 40.7))),
        }],
    };
    let committed = ingestor.ingest(snapshot).await.unwrap();
    assert_eq!(committed.trips.len(), 1);
    let retained = history.load(Source::NjtBus, now, None).await.unwrap();
    assert_eq!(retained.trips.len(), 1);
    assert_eq!(retained.stop_times.len(), 1);
    drop(ingestor);
    drop(history);
    drop(storage);

    let storage = Storage::open(&dir).unwrap();
    let ingestor = OperationsIngestor::new(storage, LiveSnapshots::default(), index).unwrap();
    let history = ingestor.history();
    let retained = history.load(Source::NjtBus, now, None).await.unwrap();
    assert_eq!(retained.trips[0].original_id, "trip-1");
}
