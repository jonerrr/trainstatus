use std::sync::Arc;

use chrono::{DateTime, Utc};

use super::{
    cache::TrajectoryCache,
    calculator::{TrajectoryCalculator, compute_trajectory_async},
    derive::TrajectoryDeriver,
    history::load_historical_inputs,
    render_units::expand_render_units,
    types::{HotSnapshot, TrajectoryConfig},
};
use crate::operations::{history::History, ingestor::SourceSnapshot};
use crate::source::Source;
use crate::telemetry::metrics::{self, TrajectorySummary};

/// Owns live publication and historical derivation behind one read interface.
#[derive(Clone)]
pub struct TrajectoryService {
    cache: Arc<TrajectoryCache>,
    calculator: Arc<TrajectoryCalculator>,
    history: History,
    deriver: TrajectoryDeriver,
}
impl TrajectoryService {
    pub fn new(pool: sqlx::PgPool) -> Self {
        let cache = Arc::new(TrajectoryCache::new());
        let calculator = Arc::new(TrajectoryCalculator::new());
        Self {
            history: History::new(pool),
            deriver: TrajectoryDeriver::new(calculator.clone(), cache.clone()),
            cache,
            calculator,
        }
    }
    pub async fn derive(&self, snapshot: Arc<SourceSnapshot>) -> anyhow::Result<HotSnapshot> {
        self.deriver.derive(snapshot).await
    }
    pub async fn publish_live(&self, source: Source, snapshot: HotSnapshot) {
        let summary = metrics::metrics().map(|_| TrajectorySummary {
            plans: snapshot.plans.len(),
            render_units: snapshot.render_units.len(),
            committed_at: snapshot.committed_at,
        });
        self.cache.set_hot(source, snapshot).await;
        if let (Some(metrics), Some(summary)) = (metrics::metrics(), summary) {
            metrics.trajectory_published(source, summary);
        }
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
        let inputs = load_historical_inputs(&self.history, &self.cache, source, at).await?;
        let config = TrajectoryConfig::for_source(source);
        let engine = self.calculator.clone();
        let cache = self.cache.clone();
        let futures: Vec<_> = inputs
            .into_iter()
            .map(|snapshot| {
                let engine = engine.clone();
                let cache = cache.clone();
                async move {
                    let computed = match compute_trajectory_async(
                        &engine, source, &snapshot, None, &cache, &config,
                    )
                    .await
                    {
                        Ok(computed) => computed,
                        Err(error) => {
                            tracing::warn!(
                                {
                                    trainstatus.source = source.as_str(),
                                    trainstatus.trip.id = %snapshot.trip_id,
                                    error.message = %format_args!("{error:#}"),
                                },
                                "Historical trajectory calculation failed"
                            );
                            return None;
                        }
                    };
                    let units = expand_render_units(source, &snapshot, &computed.trajectory);
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
