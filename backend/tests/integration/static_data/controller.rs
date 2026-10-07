use crate::support::{mta_bus_dataset, test_stores};
use async_trait::async_trait;
use backend::{
    models::source::Source,
    sources::StaticAdapter,
    static_data::{controller as static_data, dataset::StaticDataset},
};
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

    async fn collect(&self) -> anyhow::Result<StaticDataset> {
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

    let stores = test_stores(pool.clone());
    let imports = Arc::new(AtomicUsize::new(0));
    let controller = static_data::run(
        &pool,
        &stores.stop_store,
        &stores.static_data_store,
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
        .static_index()
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
    async fn collect(&self) -> anyhow::Result<StaticDataset> {
        self.imports.fetch_add(1, Ordering::SeqCst);
        let mut dataset = crate::support::njt_bus_dataset();
        dataset
            .scheduled_trips
            .push(backend::static_data::schedule::ScheduledTrip {
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
async fn njt_upgrade_imports_once_and_next_restart_restores_without_collection(pool: sqlx::PgPool) {
    let stores = test_stores(pool.clone());
    stores
        .static_data_store
        .persist(&crate::support::njt_bus_dataset())
        .await
        .unwrap();
    // Simulate an installation whose old static rows predate schedule persistence.
    sqlx::query("UPDATE source SET schedules_initialized = FALSE WHERE id = $1")
        .bind(Source::NjtBus)
        .execute(&pool)
        .await
        .unwrap();
    let imports = Arc::new(AtomicUsize::new(0));
    for expected_imports in [1, 1] {
        let restarted = test_stores(pool.clone());
        let controller = static_data::run(
            &pool,
            &restarted.stop_store,
            &restarted.static_data_store,
            vec![Arc::new(CountingNjtAdapter {
                imports: imports.clone(),
            })],
        )
        .await;
        controller.ensure_updated(Source::NjtBus).await.unwrap();
        let revision = controller.static_index().get(Source::NjtBus).unwrap();
        assert_eq!(
            revision
                .scheduled_trip("restart-trip", "20260528")
                .unwrap()
                .headsign,
            "Recovered terminal"
        );
        assert_eq!(imports.load(Ordering::SeqCst), expected_imports);
    }
}
