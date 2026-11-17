use crate::network::{
    controller,
    dataset::NetworkDataset,
    index::{NetworkIndex, TripPattern},
    types::route::{Route, RouteData},
    types::stop::{NjtBusStopData, Stop, StopData},
};
use crate::operations::{
    engine::RealtimeEngine,
    ingestor::OperationsIngestor,
    live::LiveSnapshots,
    source::{CollectedSnapshot, RealtimeSource, RealtimeSourceConfig},
    types::trip::{NjtBusData, TripData, TripRun},
};
use crate::source::Source;
use crate::sources::StaticAdapter;
use crate::storage::store::Storage;
use crate::trajectory::service::TrajectoryService;
use std::{sync::Arc, time::Duration};

struct LiteralSource {
    fail: bool,
}
#[async_trait::async_trait]
impl RealtimeSource for LiteralSource {
    fn config(&self) -> RealtimeSourceConfig {
        RealtimeSourceConfig {
            source: Source::NjtBus,
            refresh_interval: Duration::from_secs(3600),
        }
    }
    async fn collect(
        &self,
        revision: Arc<crate::network::index::NetworkRevision>,
    ) -> anyhow::Result<CollectedSnapshot> {
        if self.fail {
            anyhow::bail!("collection failed")
        }
        let now = chrono::Utc::now();
        let service_date = now.date_naive();
        let key = crate::operations::identity::RunId::scheduled_bus(
            Source::NjtBus,
            "literal-trip",
            service_date,
        );
        Ok(CollectedSnapshot {
            source: Source::NjtBus,
            network_revision: Some(revision),
            renormalize: None,
            trips: vec![(
                TripRun {
                    id: key,
                    original_id: "literal-trip".into(),
                    vehicle_id: "bus".into(),
                    route_id: "A".into(),
                    shape_ids: vec![],
                    direction: 0,
                    created_at: now,
                    updated_at: now,
                    partition_date: now.date_naive(),
                    data: TripData::NjtBus(NjtBusData {
                        deviation: None,
                        headsign: "Terminal".into(),
                    }),
                },
                vec![],
            )],
            positions: vec![],
        })
    }
}

struct LiteralStatic;
#[async_trait::async_trait]
impl StaticAdapter for LiteralStatic {
    fn source(&self) -> Source {
        Source::NjtBus
    }
    fn refresh_interval(&self) -> Duration {
        Duration::from_secs(3600)
    }
    async fn collect(&self) -> anyhow::Result<NetworkDataset> {
        let mut dataset = NetworkDataset::new(Source::NjtBus);
        dataset.routes.push(Route {
            id: "A".into(),
            long_name: "Route A".into(),
            short_name: "A".into(),
            color: "FFFFFF".into(),
            text_color: "000000".into(),
            data: RouteData::NjtBus,
        });
        dataset.stops.push(Stop {
            id: "STOP".into(),
            name: "Terminal".into(),
            geom: geo::Point::new(-74.0, 40.7).into(),
            nearby_stops: vec![],
            routes: vec![],
            data: StopData::NjtBus(NjtBusStopData {
                stop_code: "1".into(),
            }),
        });
        dataset.trip_patterns.insert(
            "literal-trip".into(),
            TripPattern {
                route_id: "A".into(),
                direction: 0,
                shape_id: None,
            },
        );
        Ok(dataset)
    }
}

fn temp_storage() -> Storage {
    let dir = std::env::temp_dir().join(format!("ops-engine-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    Storage::open(&dir).unwrap()
}

#[tokio::test]
async fn successful_collection_publishes_snapshot() {
    let storage = temp_storage();
    let live = LiveSnapshots::default();
    let index = NetworkIndex::new();
    let network = crate::network::store::Network::new(storage.clone(), index.clone()).unwrap();
    let controller = controller::run(&network, vec![Arc::new(LiteralStatic)]).await;
    let ingestor =
        OperationsIngestor::new(storage.clone(), live.clone(), controller.network_index()).unwrap();
    let trajectories = TrajectoryService::new(ingestor.history());
    let engine = RealtimeEngine::new(ingestor, controller, trajectories);
    let source = Arc::new(LiteralSource { fail: false });
    let snapshot = engine.collect_once(source.as_ref()).await.unwrap();
    assert_eq!(snapshot.trips[0].original_id, "literal-trip");
    assert!(live.get(Source::NjtBus).is_some());
}
