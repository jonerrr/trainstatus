use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use async_trait::async_trait;

use crate::support::{TestContext, mta_bus_dataset, njt_bus_dataset, test_owners};
use backend::{
    network::{controller, dataset::NetworkDataset},
    source::Source,
    sources::StaticAdapter,
};

struct CountingStaticAdapter {
    imports: Arc<AtomicUsize>,
}

#[async_trait]
impl StaticAdapter for CountingStaticAdapter {
    fn source(&self) -> Source {
        Source::MtaBus
    }

    fn refresh_interval(&self) -> Duration {
        Duration::from_secs(24 * 60 * 60)
    }

    async fn collect(&self) -> anyhow::Result<NetworkDataset> {
        self.imports.fetch_add(1, Ordering::SeqCst);
        Ok(mta_bus_dataset())
    }
}

#[tokio::test]
async fn static_controller_initializes_empty_index_from_storage() {
    let ctx = TestContext::new();
    let storage = ctx.storage();
    let stores = test_owners(storage);
    let imports = Arc::new(AtomicUsize::new(0));
    let controller = controller::run(
        &stores.network,
        vec![Arc::new(CountingStaticAdapter {
            imports: imports.clone(),
        })],
    )
    .await;

    controller
        .ensure_updated(Source::MtaBus)
        .await
        .expect("empty index should trigger initialization");

    let revision = controller
        .network_index()
        .get(Source::MtaBus)
        .expect("initialization must publish a complete revision");
    let route = &revision.routes["B100"];
    assert!(!route.shape_ids.is_empty());
    assert!(revision.stops.contains_key("300226"));
    let route_stop_shapes = &revision.route_stop_shapes[&("B100".into(), "300226".into())];
    assert!(!route_stop_shapes.is_empty());
    assert!(
        route_stop_shapes
            .iter()
            .all(|shape_id| route.shape_ids.contains(shape_id)
                && revision.shapes.get(shape_id).unwrap().is_some())
    );
    controller
        .ensure_updated(Source::MtaBus)
        .await
        .expect("fresh initialized revision should be reused");
    assert_eq!(imports.load(Ordering::SeqCst), 1);
}

fn restart_schedule() -> backend::network::schedule::ScheduledTrip {
    let date = chrono::NaiveDate::from_ymd_opt(2026, 5, 28).unwrap();
    let origin = backend::network::expansion::gtfs_datetime(date, 0).unwrap();
    backend::network::schedule::ScheduledTrip {
        template: Arc::new(backend::network::schedule::TripTemplate {
            trip_id: "restart-trip".into(),
            route_id: "87".into(),
            headsign: "Recovered terminal".into(),
            direction_id: 1,
            start_seconds: u32::try_from((crate::support::fixed_time() - origin).num_seconds())
                .unwrap(),
            stop_times: vec![],
        }),
        start_date: date,
    }
}

struct CountingNjtAdapter {
    imports: Arc<AtomicUsize>,
}

#[async_trait]
impl StaticAdapter for CountingNjtAdapter {
    fn source(&self) -> Source {
        Source::NjtBus
    }
    fn refresh_interval(&self) -> Duration {
        Duration::from_secs(24 * 60 * 60)
    }
    async fn collect(&self) -> anyhow::Result<NetworkDataset> {
        self.imports.fetch_add(1, Ordering::SeqCst);
        let mut dataset = njt_bus_dataset();
        dataset.scheduled_trips.push(restart_schedule());
        Ok(dataset)
    }
}

#[tokio::test]
async fn njt_restart_restores_without_collection() {
    let ctx = TestContext::new();
    let storage = ctx.storage();
    let stores = test_owners(storage.clone());
    let mut dataset = njt_bus_dataset();
    dataset.scheduled_trips.push(restart_schedule());
    stores.network.persist(dataset).await.unwrap();
    let imports = Arc::new(AtomicUsize::new(0));
    for _ in 0..2 {
        let restarted = test_owners(storage.clone());
        let controller = controller::run(
            &restarted.network,
            vec![Arc::new(CountingNjtAdapter {
                imports: imports.clone(),
            })],
        )
        .await;
        controller.ensure_updated(Source::NjtBus).await.unwrap();
        let revision = controller.network_index().get(Source::NjtBus).unwrap();
        assert_eq!(
            revision
                .scheduled_trip("restart-trip", "20260528", crate::support::fixed_time())
                .unwrap()
                .template
                .headsign,
            "Recovered terminal"
        );
        assert_eq!(imports.load(Ordering::SeqCst), 0);
    }
}

#[tokio::test]
async fn fresh_and_restored_revisions_share_canonical_keys_and_colors() {
    let ctx = TestContext::new();
    let storage = ctx.storage();
    let stores = test_owners(storage.clone());
    let mut dataset = mta_bus_dataset();
    for route in &mut dataset.routes {
        route.id = route.id.to_lowercase();
        route.color = "invalid".into();
    }
    for stop in &mut dataset.stops {
        stop.id = stop.id.to_lowercase();
    }
    for relation in &mut dataset.route_stops {
        relation.route_id = relation.route_id.to_lowercase();
        relation.stop_id = relation.stop_id.to_lowercase();
    }
    stores.network.persist(dataset).await.unwrap();
    let fresh = stores.network.network_index().get(Source::MtaBus).unwrap();
    let restarted = test_owners(storage);
    let restored = restarted
        .network
        .restore_revision(Source::MtaBus)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        fresh.catalog.as_ref().unwrap().body,
        restored.catalog.as_ref().unwrap().body
    );
    assert_eq!(fresh.routes.len(), restored.routes.len());
    for (id, route) in fresh.routes.iter() {
        let stored = &restored.routes[id];
        assert_eq!(route.color, stored.color);
        assert_eq!(route.shape_ids, stored.shape_ids);
    }
    assert_eq!(fresh.route_stop_shapes, restored.route_stop_shapes);
    assert!(fresh.stops.keys().all(|id| restored.stops.contains_key(id)));
}
