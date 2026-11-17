use std::collections::HashMap;
use std::sync::Arc;

use moka::sync::Cache;

use super::assets::ShapeAssets;
use crate::geometry::Geom;
use crate::storage::content::ContentId;

const CACHE_BUDGET: u64 = 32 * 1024 * 1024;
type GeometryCache = Cache<ContentId, (Arc<Geom>, u32)>;

fn geometry_cache() -> GeometryCache {
    Cache::builder()
        .max_capacity(CACHE_BUDGET)
        .weigher(|_: &ContentId, (_, weight): &(Arc<Geom>, u32)| *weight)
        .build()
}

#[derive(Debug, Clone)]
enum Backing {
    Prepared(Arc<HashMap<String, Arc<Geom>>>),
    Durable {
        assets: ShapeAssets,
        ids: Arc<HashMap<String, ContentId>>,
        cache: GeometryCache,
    },
}

/// Binding views share one byte-weighted cache, keyed by immutable content rather than aliases.
#[derive(Debug, Clone)]
pub struct ShapeIndex {
    backing: Backing,
}

impl ShapeIndex {
    pub fn from_prepared(shapes: HashMap<String, Geom>) -> Self {
        Self {
            backing: Backing::Prepared(Arc::new(
                shapes
                    .into_iter()
                    .map(|(id, geom)| (id, Arc::new(geom)))
                    .collect(),
            )),
        }
    }

    pub fn from_assets(assets: ShapeAssets, ids: HashMap<String, ContentId>) -> Self {
        Self {
            backing: Backing::Durable {
                assets,
                ids: Arc::new(ids),
                cache: geometry_cache(),
            },
        }
    }

    pub fn asset_id(&self, id: &str) -> Option<ContentId> {
        match &self.backing {
            Backing::Prepared(_) => None,
            Backing::Durable { ids, .. } => ids.get(id).copied(),
        }
    }

    pub fn with_bindings(&self, ids: HashMap<String, ContentId>, assets: ShapeAssets) -> Self {
        let cache = match &self.backing {
            Backing::Prepared(_) => geometry_cache(),
            Backing::Durable { cache, .. } => cache.clone(),
        };
        Self {
            backing: Backing::Durable {
                assets,
                ids: Arc::new(ids),
                cache,
            },
        }
    }

    pub fn get_asset(&self, id: ContentId) -> anyhow::Result<Arc<Geom>> {
        let Backing::Durable { assets, cache, .. } = &self.backing else {
            anyhow::bail!("asset lookup requires durable shape assets");
        };
        let (geometry, _) = cache
            .try_get_with(id, || -> anyhow::Result<_> {
                let (geometry, weight) = assets.get_weighted(id)?;
                Ok((Arc::new(geometry), u32::try_from(weight)?))
            })
            .map_err(|error| anyhow::anyhow!("{error:#}"))?;
        Ok(geometry)
    }

    pub fn contains_key(&self, id: &str) -> bool {
        match &self.backing {
            Backing::Prepared(shapes) => shapes.contains_key(id),
            Backing::Durable { ids, .. } => ids.contains_key(id),
        }
    }

    pub fn len(&self) -> usize {
        match &self.backing {
            Backing::Prepared(shapes) => shapes.len(),
            Backing::Durable { ids, .. } => ids.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn get(&self, id: &str) -> anyhow::Result<Option<Arc<Geom>>> {
        match &self.backing {
            Backing::Prepared(shapes) => Ok(shapes.get(id).cloned()),
            Backing::Durable { ids, .. } => ids
                .get(id)
                .copied()
                .map(|key| self.get_asset(key))
                .transpose(),
        }
    }
}
