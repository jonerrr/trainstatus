use crate::support::mta_bus_dataset;
use crate::support::test_owners;
use async_trait::async_trait;
use backend::network::controller as static_data;
use backend::network::dataset::NetworkDataset;
use backend::source::Source;
use backend::sources::StaticAdapter;
use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
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

#[sqlx::test]
async fn static_controller_initializes_empty_index_even_when_database_is_fresh(pool: sqlx::PgPool) {
    sqlx::query(
        r#"
        INSERT INTO source (id, name, updated_at)
        VALUES ($1, $2, NOW())
        ON CONFLICT (id) DO UPDATE SET updated_at = NOW()
        "#,
    )
    .bind(Source::MtaBus)
    .bind(Source::MtaBus.as_str())
    .execute(&pool)
    .await
    .expect("fresh source timestamp should save");

    let stores = test_owners(pool.clone());
    let imports = Arc::new(AtomicUsize::new(0));
    let controller = static_data::run(
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
    assert!(route_stop_shapes.iter().all(
        |shape_id| route.shape_ids.contains(shape_id) && revision.shapes.contains_key(shape_id)
    ));
    controller
        .ensure_updated(Source::MtaBus)
        .await
        .expect("fresh initialized revision should be reused");
    assert_eq!(imports.load(Ordering::SeqCst), 1);
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
        let mut dataset = crate::support::njt_bus_dataset();
        dataset
            .scheduled_trips
            .push(backend::network::schedule::ScheduledTrip {
                trip_id: "restart-trip".into(),
                route_id: "87".into(),
                headsign: "Recovered terminal".into(),
                direction_id: 1,
                start_date: "20260528".into(),
                start_time: crate::support::fixed_time(),
                stop_times: vec![],
            });
        Ok(dataset)
    }
}

#[sqlx::test]
async fn njt_restart_restores_without_collection(pool: sqlx::PgPool) {
    let stores = test_owners(pool.clone());
    let mut dataset = crate::support::njt_bus_dataset();
    dataset
        .scheduled_trips
        .push(backend::network::schedule::ScheduledTrip {
            trip_id: "restart-trip".into(),
            route_id: "87".into(),
            headsign: "Recovered terminal".into(),
            direction_id: 1,
            start_date: "20260528".into(),
            start_time: crate::support::fixed_time(),
            stop_times: vec![],
        });
    stores.network.persist(&dataset).await.unwrap();
    let imports = Arc::new(AtomicUsize::new(0));
    for _ in 0..2 {
        let restarted = test_owners(pool.clone());
        let controller = static_data::run(
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
                .scheduled_trip("restart-trip", "20260528")
                .unwrap()
                .headsign,
            "Recovered terminal"
        );
        assert_eq!(imports.load(Ordering::SeqCst), 0);
    }
}

#[sqlx::test]
async fn fresh_and_restored_revisions_share_canonical_keys_and_colors(pool: sqlx::PgPool) {
    let stores = test_owners(pool.clone());
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
    stores.network.persist(&dataset).await.unwrap();
    let fresh = stores.network.network_index().get(Source::MtaBus).unwrap();
    let restarted = test_owners(pool);
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
