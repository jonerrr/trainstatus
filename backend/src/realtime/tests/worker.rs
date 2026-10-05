use crate::realtime::engine::trajectory_worker;
use crate::realtime::{IngestionChanges, PersistedSnapshot};
use crate::{
    models::{source::Source, static_dataset::StaticDataset},
    static_index::StaticTransitRevision,
    trajectory::{HotSnapshot, TrajectoryCache},
};
use std::{sync::Arc, time::Duration};
use tokio::sync::{Semaphore, mpsc, watch};

fn persisted(generation: usize) -> Arc<PersistedSnapshot> {
    Arc::new(PersistedSnapshot {
        source: Source::NjtBus,
        trips: vec![],
        stop_times: vec![],
        positions: vec![],
        changed_trip_ids: Default::default(),
        changes: IngestionChanges {
            trips: generation,
            ..Default::default()
        },
        static_revision: Arc::new(StaticTransitRevision::from_dataset(&StaticDataset::new(
            Source::NjtBus,
        ))),
    })
}

#[tokio::test(start_paused = true)]
async fn busy_trajectory_worker_collapses_pending_snapshots_without_blocking_sender() {
    let (tx, rx) = watch::channel(None);
    let (started, mut observed) = mpsc::unbounded_channel();
    let release = Arc::new(Semaphore::new(0));
    let cache = Arc::new(TrajectoryCache::new());
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
    let cache = Arc::new(TrajectoryCache::new());
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
    cache.set_hot(Source::NjtBus, previous).await;
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
        cache.get_hot(Source::NjtBus).await.unwrap().prev_states[&id].s_m,
        12.0
    );
    tx.send(Some(persisted(2))).unwrap();
    assert_eq!(observed.recv().await, Some(2));
    tokio::task::yield_now().await;
    assert!(
        cache
            .get_hot(Source::NjtBus)
            .await
            .unwrap()
            .prev_states
            .is_empty()
    );
    drop(tx);
    task.await.unwrap();
}
