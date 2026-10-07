use super::{
    HotSnapshot, TrajectoryCache, TrajectoryCalculator, TrajectoryConfig, TrajectoryDeriver,
    compute_trajectory_async, expand_render_units,
};
use crate::{
    models::source::Source, realtime::PersistedSnapshot, stores::trajectory::TrajectoryStore,
};
use chrono::{DateTime, Utc};
use std::sync::Arc;

/// Owns live publication and historical derivation behind one read interface.
#[derive(Clone)]
pub struct TrajectoryService {
    cache: Arc<TrajectoryCache>,
    calculator: Arc<TrajectoryCalculator>,
    store: TrajectoryStore,
    deriver: TrajectoryDeriver,
}
impl TrajectoryService {
    pub fn new(pool: sqlx::PgPool) -> Self {
        let cache = Arc::new(TrajectoryCache::new());
        let calculator = Arc::new(TrajectoryCalculator::new());
        Self {
            store: TrajectoryStore::new(pool, cache.clone()),
            deriver: TrajectoryDeriver::new(calculator.clone(), cache.clone()),
            cache,
            calculator,
        }
    }
    pub async fn derive(&self, snapshot: Arc<PersistedSnapshot>) -> anyhow::Result<HotSnapshot> {
        self.deriver.derive(snapshot).await
    }
    pub async fn publish_live(&self, source: Source, snapshot: HotSnapshot) {
        self.cache.set_hot(source, snapshot).await;
    }
    pub async fn snapshot(
        &self,
        source: Source,
        at: Option<DateTime<Utc>>,
    ) -> anyhow::Result<Arc<HotSnapshot>> {
        let Some(at) = at else {
            return Ok(self
                .cache
                .get_hot(source)
                .await
                .unwrap_or_else(|| Arc::new(HotSnapshot::empty())));
        };
        self.cache
            .get_historical_with(source, at, self.derive_historical(source, at))
            .await
    }

    async fn derive_historical(
        &self,
        source: Source,
        at: DateTime<Utc>,
    ) -> anyhow::Result<Arc<HotSnapshot>> {
        let inputs = self.store.load_historical_inputs(source, at).await?;
        let config = TrajectoryConfig::for_source(source);
        let engine = self.calculator.clone();
        let cache = self.cache.clone();
        let futures: Vec<_> = inputs
            .into_iter()
            .map(|snapshot| {
                let engine = engine.clone();
                let cache = cache.clone();
                async move {
                    let shape_geom = cache
                        .get_shape_geometry(source, &snapshot.shape_key, &snapshot.shape)
                        .await?;
                    let computed = match compute_trajectory_async(&engine, source, &snapshot, None, &cache, &config).await {
                        Ok(computed) => computed,
                        Err(error) => {
                            tracing::warn!(trip_id = %snapshot.trip_id, %error, "Historical trajectory calculation failed");
                            return None;
                        }
                    };
                    let units =
                        expand_render_units(source, &snapshot, &computed.trajectory, &shape_geom);
                    Some(units)
                }
            })
            .collect();

        let results: Vec<_> = futures::future::join_all(futures)
            .await
            .into_iter()
            .flatten()
            .collect();

        let mut render_units = std::collections::HashMap::new();
        for unit in results.into_iter().flatten() {
            render_units.insert(unit.render_unit_id.clone(), unit);
        }

        Ok(Arc::new(HotSnapshot::historical_from(render_units)))
    }
}
