use crate::network::dataset::NetworkDataset;
use crate::network::index::NetworkRevision;
use crate::operations::IngestionChanges;
use crate::operations::SourceSnapshot;
use crate::operations::engine::trajectory_worker;
use crate::source::Source;
use crate::trajectory::HotSnapshot;
use crate::trajectory::TrajectoryService;
use std::{sync::Arc, time::Duration};
use tokio::sync::{Semaphore, mpsc, watch};

fn trajectories() -> TrajectoryService {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .connect_lazy("postgres://unused:unused@127.0.0.1:1/unused")
        .unwrap();
    TrajectoryService::new(pool)
}

fn persisted(generation: usize) -> Arc<SourceSnapshot> {
    Arc::new(SourceSnapshot {
        source: Source::NjtBus,
        committed_at: chrono::Utc::now(),
        trips: vec![],
        stop_times: vec![],
        positions: vec![],
        changes: IngestionChanges {
            trips: generation,
            ..Default::default()
        },
        network_revision: Arc::new(NetworkRevision::from_dataset(&NetworkDataset::new(
            Source::NjtBus,
        ))),
    })
}

#[tokio::test(start_paused = true)]
async fn busy_trajectory_worker_collapses_pending_snapshots_without_blocking_sender() {
    let (tx, rx) = watch::channel(None);
    let (started, mut observed) = mpsc::unbounded_channel();
    let release = Arc::new(Semaphore::new(0));
    let cache = trajectories();
    let task = tokio::spawn(trajectory_worker(Source::NjtBus, rx, cache, {
        let release = release.clone();
        move |snapshot| {
            let started = started.clone();
            let release = release.clone();
            async move {
                started.send(snapshot.changes.trips).unwrap();
                release.acquire().await.unwrap().forget();
                Ok(HotSnapshot::empty())
            }
        }
    }));
    tx.send(Some(persisted(1))).unwrap();
    assert_eq!(observed.recv().await, Some(1));
    // Sending is synchronous even while derivation holds the first snapshot.
    tx.send(Some(persisted(2))).unwrap();
    tx.send(Some(persisted(3))).unwrap();
    release.add_permits(1);
    assert_eq!(observed.recv().await, Some(3));
    release.add_permits(1);
    tokio::task::yield_now().await;
    tokio::time::advance(Duration::from_secs(300)).await;
    tokio::task::yield_now().await;
    assert!(
        observed.try_recv().is_err(),
        "there must be no independent trajectory timer"
    );
    drop(tx);
    task.await.unwrap();
}

#[tokio::test(start_paused = true)]
async fn derivation_error_keeps_hot_generation_and_empty_snapshot_clears_it() {
    let cache = trajectories();
    let mut previous = HotSnapshot::empty();
    let id = uuid::Uuid::now_v7();
    previous.prev_states.insert(
        id,
        crate::trajectory::TrajectoryState {
            t_unix: 1.0,
            s_m: 12.0,
            v_mps: 0.0,
        },
    );
    cache.publish_live(Source::NjtBus, previous).await;
    let (tx, rx) = watch::channel(None);
    let (started, mut observed) = mpsc::unbounded_channel();
    let task = tokio::spawn(trajectory_worker(
        Source::NjtBus,
        rx,
        cache.clone(),
        move |snapshot| {
            let started = started.clone();
            async move {
                started.send(snapshot.changes.trips).unwrap();
                if snapshot.changes.trips == 1 {
                    anyhow::bail!("failed generation")
                }
                Ok(HotSnapshot::empty())
            }
        },
    ));
    tx.send(Some(persisted(1))).unwrap();
    assert_eq!(observed.recv().await, Some(1));
    tokio::task::yield_now().await;
    assert_eq!(
        cache
            .snapshot(Source::NjtBus, None)
            .await
            .unwrap()
            .prev_states[&id]
            .s_m,
        12.0
    );
    tx.send(Some(persisted(2))).unwrap();
    assert_eq!(observed.recv().await, Some(2));
    tokio::task::yield_now().await;
    assert!(
        cache
            .snapshot(Source::NjtBus, None)
            .await
            .unwrap()
            .prev_states
            .is_empty()
    );
    drop(tx);
    task.await.unwrap();
}
