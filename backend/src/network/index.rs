use super::publication::NetworkPublication;
use super::retired::{RetiredNetworkIndex, RetiredRoute, RetiredShape, RetiredStop};
use super::{
    catalog::CatalogResponse,
    dataset::NetworkDataset,
    schedule::{ScheduledOccurrence, ScheduledTripEntry},
    types::{
        route::{Route, RouteData},
        stop::{RouteStopData, Stop, StopData},
    },
};
use crate::geometry::Geom;
use crate::source::Source;
use chrono::{DateTime, Utc};
use std::{collections::HashMap, sync::Arc};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct TripPattern {
    pub route_id: String,
    pub direction: i16,
    pub shape_id: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct TripPatternRevision {
    pub patterns: HashMap<String, TripPattern>,
    pub stop_remap: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct IndexedRoute {
    pub color: String,
    pub shape_ids: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct IndexedStop {
    pub data: StopData,
    pub geom: Geom,
}

#[derive(Debug, Clone)]
pub struct NetworkRevision {
    pub source: Source,
    /// Installed by import/restore before publishing; absent on dataset-only revisions.
    pub catalog: Option<Arc<CatalogResponse>>,
    pub routes: Arc<HashMap<String, IndexedRoute>>,
    pub stops: Arc<HashMap<String, IndexedStop>>,
    pub shapes: super::shape_index::ShapeIndex,
    pub route_stop_shapes: Arc<HashMap<(String, String), Vec<String>>>,
    pub trip_patterns: Arc<HashMap<String, TripPattern>>,
    pub stop_remap: Arc<HashMap<String, String>>,
    pub scheduled_trips: Arc<HashMap<String, HashMap<String, ScheduledTripEntry>>>,
}

impl NetworkRevision {
    /// Build a new revision with fresh schedule expiration. Import/restore use
    /// persisted entries instead so earlier service dates keep their own expiry.
    pub fn from_dataset(dataset: &NetworkDataset) -> Self {
        let expires_at = chrono::Utc::now() + chrono::Duration::hours(48);
        Self::from_dataset_with_schedules(
            dataset,
            dataset
                .scheduled_trips
                .iter()
                .cloned()
                .map(|trip| ScheduledTripEntry { trip, expires_at }),
        )
    }

    pub fn from_dataset_with_schedules(
        dataset: &NetworkDataset,
        schedules: impl IntoIterator<Item = ScheduledTripEntry>,
    ) -> Self {
        let routes = dataset
            .routes
            .iter()
            .map(|route| {
                let shape_ids = match &route.data {
                    RouteData::MtaBus(data) => data.shape_ids.clone(),
                    RouteData::MtaSubway | RouteData::NjtBus => Vec::new(),
                };
                (
                    route.id.to_uppercase(),
                    IndexedRoute {
                        color: crate::utils::color::normalize_hex_color(&route.color),
                        shape_ids,
                    },
                )
            })
            .collect();
        let stops = dataset
            .stops
            .iter()
            .map(|stop| {
                (
                    stop.id.to_uppercase(),
                    IndexedStop {
                        data: stop.data.clone(),
                        geom: stop.geom.clone(),
                    },
                )
            })
            .collect();
        let shapes = dataset
            .shapes
            .iter()
            .map(|shape| (shape.id.clone(), shape.geom.clone()))
            .collect();
        let route_stop_shapes = dataset
            .route_stops
            .iter()
            .map(|route_stop| {
                let shape_ids = match &route_stop.data {
                    RouteStopData::MtaBus { shape_ids, .. } => shape_ids.clone(),
                    RouteStopData::MtaSubway { .. } | RouteStopData::NjtBus { .. } => Vec::new(),
                };
                (
                    (
                        route_stop.route_id.to_uppercase(),
                        route_stop.stop_id.to_uppercase(),
                    ),
                    shape_ids,
                )
            })
            .collect();

        Self {
            source: dataset.source,
            catalog: None,
            routes: Arc::new(routes),
            stops: Arc::new(stops),
            shapes: super::shape_index::ShapeIndex::from_prepared(shapes),
            route_stop_shapes: Arc::new(route_stop_shapes),
            trip_patterns: Arc::new(dataset.trip_patterns.clone()),
            stop_remap: Arc::new(dataset.stop_remap.clone()),
            scheduled_trips: Arc::new(schedule_index(schedules)),
        }
    }
    pub fn shape(&self, id: &str) -> anyhow::Result<Option<Arc<Geom>>> {
        self.shapes.get(id)
    }

    pub fn with_stops(&self, stops: Vec<Stop>, catalog: CatalogResponse) -> Self {
        let mut revision = self.clone();
        let mut route_stop_shapes = HashMap::new();
        let stops = stops
            .into_iter()
            .map(|stop| {
                for route_stop in stop.routes {
                    let shape_ids = match route_stop.data {
                        RouteStopData::MtaBus { shape_ids, .. } => shape_ids,
                        RouteStopData::MtaSubway { .. } | RouteStopData::NjtBus { .. } => {
                            Vec::new()
                        }
                    };
                    route_stop_shapes.insert(
                        (
                            route_stop.route_id.to_uppercase(),
                            route_stop.stop_id.to_uppercase(),
                        ),
                        shape_ids,
                    );
                }
                (
                    stop.id.to_uppercase(),
                    IndexedStop {
                        data: stop.data,
                        geom: stop.geom,
                    },
                )
            })
            .collect();
        revision.stops = Arc::new(stops);
        revision.route_stop_shapes = Arc::new(route_stop_shapes);
        revision.catalog = Some(Arc::new(catalog));
        revision
    }

    pub fn set_schedules(&mut self, trips: impl IntoIterator<Item = ScheduledTripEntry>) {
        self.scheduled_trips = Arc::new(schedule_index(trips));
    }

    pub fn scheduled_trip(
        &self,
        trip_id: &str,
        service_date: &str,
        at: DateTime<Utc>,
    ) -> Option<ScheduledOccurrence<'_>> {
        let entry = self.scheduled_trips.get(trip_id)?.get(service_date)?;
        (entry.expires_at > at).then(|| entry.trip.occurrence())
    }

    pub fn try_from_sections(
        source: Source,
        routes: Vec<Route>,
        stops: Vec<Stop>,
        shapes: super::shape_index::ShapeIndex,
        trip_patterns: HashMap<String, TripPattern>,
        stop_remap: HashMap<String, String>,
    ) -> Option<Self> {
        if routes.is_empty() && stops.is_empty() {
            return None;
        }
        if source == Source::NjtBus && trip_patterns.is_empty() {
            return None;
        }
        let mut route_stop_shapes = HashMap::new();
        let stops = stops
            .into_iter()
            .map(|stop| {
                for route_stop in stop.routes {
                    let shape_ids = match route_stop.data {
                        RouteStopData::MtaBus { shape_ids, .. } => shape_ids,
                        RouteStopData::MtaSubway { .. } | RouteStopData::NjtBus { .. } => {
                            Vec::new()
                        }
                    };
                    route_stop_shapes.insert(
                        (
                            route_stop.route_id.to_uppercase(),
                            route_stop.stop_id.to_uppercase(),
                        ),
                        shape_ids,
                    );
                }
                (
                    stop.id.to_uppercase(),
                    IndexedStop {
                        data: stop.data,
                        geom: stop.geom,
                    },
                )
            })
            .collect();
        let routes = routes
            .into_iter()
            .map(|route| {
                let shape_ids = match route.data {
                    RouteData::MtaBus(data) => data.shape_ids,
                    RouteData::MtaSubway | RouteData::NjtBus => Vec::new(),
                };
                (
                    route.id.to_uppercase(),
                    IndexedRoute {
                        color: crate::utils::color::normalize_hex_color(&route.color),
                        shape_ids,
                    },
                )
            })
            .collect();
        Some(Self {
            source,
            catalog: None,
            routes: Arc::new(routes),
            stops: Arc::new(stops),
            shapes,
            route_stop_shapes: Arc::new(route_stop_shapes),
            trip_patterns: Arc::new(trip_patterns),
            stop_remap: Arc::new(stop_remap),
            scheduled_trips: Arc::new(HashMap::new()),
        })
    }
}

fn schedule_index(
    trips: impl IntoIterator<Item = ScheduledTripEntry>,
) -> HashMap<String, HashMap<String, ScheduledTripEntry>> {
    let mut index: HashMap<String, HashMap<String, ScheduledTripEntry>> = HashMap::new();
    let mut templates = HashMap::new();
    let mut pointers = HashMap::new();
    for mut entry in trips {
        let original = &entry.trip.template;
        let key = Arc::as_ptr(original);
        let canonical = pointers.entry(key).or_insert_with(|| {
            let canonical = templates
                .entry(original.clone())
                .or_insert_with(|| original.clone());
            (original.clone(), canonical.clone())
        });
        entry.trip.template = canonical.1.clone();
        let trip = &entry.trip;
        index
            .entry(trip.template.trip_id.clone())
            .or_default()
            .insert(trip.start_date.format("%Y%m%d").to_string(), entry);
    }
    index
}

#[derive(Clone, Default)]
struct NetworkRegistry {
    revisions: HashMap<Source, Arc<NetworkRevision>>,
    retired: RetiredNetworkIndex,
    source_updated_at: HashMap<Source, DateTime<Utc>>,
}

#[derive(Clone, Default)]
pub struct NetworkIndex {
    registry: Arc<std::sync::RwLock<NetworkRegistry>>,
}

impl NetworkIndex {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn publish_bundle(&self, revisions: Vec<NetworkRevision>) {
        let mut registry = self.registry.write().unwrap();
        for revision in revisions {
            registry
                .revisions
                .insert(revision.source, Arc::new(revision));
        }
    }

    pub fn publish_publication(
        &self,
        publication: &NetworkPublication,
        revisions: Vec<NetworkRevision>,
        retired: RetiredNetworkIndex,
    ) {
        let mut registry = self.registry.write().unwrap();
        for revision in revisions {
            registry
                .revisions
                .insert(revision.source, Arc::new(revision));
        }
        registry.retired = retired;
        registry.source_updated_at = publication
            .sources
            .iter()
            .map(|(source, source_ref)| (*source, source_ref.updated_at))
            .collect();
    }

    pub fn upsert_revision(&self, revision: NetworkRevision, updated_at: DateTime<Utc>) {
        let source = revision.source;
        let mut registry = self.registry.write().unwrap();
        registry.revisions.insert(source, Arc::new(revision));
        registry.source_updated_at.insert(source, updated_at);
    }

    pub fn source_updated_at(&self, source: Source) -> Option<DateTime<Utc>> {
        self.registry
            .read()
            .unwrap()
            .source_updated_at
            .get(&source)
            .copied()
    }

    pub fn get(&self, source: Source) -> Option<Arc<NetworkRevision>> {
        self.registry
            .read()
            .unwrap()
            .revisions
            .get(&source)
            .cloned()
    }

    pub fn route_color(&self, source: Source, route_id: &str) -> Option<String> {
        self.with_registry(|registry| {
            let key = route_id.to_uppercase();
            registry
                .revisions
                .get(&source)
                .and_then(|revision| revision.routes.get(&key))
                .map(|route| route.color.clone())
                .or_else(|| {
                    registry
                        .retired
                        .routes
                        .get(&source)?
                        .get(&key)
                        .map(|route| route.color.clone())
                })
        })
    }

    pub fn stop(&self, source: Source, stop_id: &str) -> Option<IndexedStop> {
        self.with_registry(|registry| {
            let key = stop_id.to_uppercase();
            registry
                .revisions
                .get(&source)
                .and_then(|revision| revision.stops.get(&key).cloned())
                .or_else(|| {
                    let retired = registry.retired.stops.get(&source)?.get(&key)?;
                    Some(IndexedStop {
                        data: retired.data.clone(),
                        geom: retired.geom.clone(),
                    })
                })
        })
    }

    pub fn shape_geom(&self, source: Source, shape_id: &str) -> anyhow::Result<Option<Arc<Geom>>> {
        let (revision, retired) = self.with_registry(|registry| {
            (
                registry.revisions.get(&source).cloned(),
                registry
                    .retired
                    .shapes
                    .get(&source)
                    .and_then(|shapes| shapes.get(shape_id))
                    .cloned(),
            )
        });
        if let Some(revision) = revision {
            if let Some(shape) = revision.shape(shape_id)? {
                return Ok(Some(shape));
            }
            if let Some(shape) = retired {
                return revision.shapes.get_asset(shape.asset_id).map(Some);
            }
        } else if retired.is_some() {
            anyhow::bail!("retired shape has no source revision");
        }
        Ok(None)
    }

    pub fn route(&self, source: Source, route_id: &str) -> Option<IndexedRoute> {
        self.with_registry(|registry| {
            let key = route_id.to_uppercase();
            registry
                .revisions
                .get(&source)
                .and_then(|revision| revision.routes.get(&key).cloned())
                .or_else(|| {
                    let retired = registry.retired.routes.get(&source)?.get(&key)?;
                    Some(IndexedRoute {
                        color: retired.color.clone(),
                        shape_ids: retired.shape_ids.clone(),
                    })
                })
        })
    }

    pub fn current_route_color(&self, source: Source, route_id: &str) -> Option<String> {
        let registry = self.registry.read().unwrap();
        registry
            .revisions
            .get(&source)?
            .routes
            .get(&route_id.to_uppercase())
            .map(|route| route.color.clone())
    }

    pub fn current_stop(&self, source: Source, stop_id: &str) -> Option<IndexedStop> {
        let registry = self.registry.read().unwrap();
        registry
            .revisions
            .get(&source)?
            .stops
            .get(&stop_id.to_uppercase())
            .cloned()
    }

    pub fn current_shape_geom(
        &self,
        source: Source,
        shape_id: &str,
    ) -> anyhow::Result<Option<Arc<Geom>>> {
        match self.get(source) {
            Some(revision) => revision.shape(shape_id),
            None => Ok(None),
        }
    }

    pub fn current_route(&self, source: Source, route_id: &str) -> Option<IndexedRoute> {
        let registry = self.registry.read().unwrap();
        registry
            .revisions
            .get(&source)?
            .routes
            .get(&route_id.to_uppercase())
            .cloned()
    }

    pub fn retired_route_color(&self, source: Source, route_id: &str) -> Option<String> {
        let registry = self.registry.read().unwrap();
        registry
            .retired
            .routes
            .get(&source)?
            .get(&route_id.to_uppercase())
            .map(|route| route.color.clone())
    }

    pub fn retired_stop(&self, source: Source, stop_id: &str) -> Option<IndexedStop> {
        let registry = self.registry.read().unwrap();
        let retired = registry
            .retired
            .stops
            .get(&source)?
            .get(&stop_id.to_uppercase())?;
        Some(IndexedStop {
            data: retired.data.clone(),
            geom: retired.geom.clone(),
        })
    }

    pub fn retired_route(&self, source: Source, route_id: &str) -> Option<IndexedRoute> {
        let registry = self.registry.read().unwrap();
        let retired = registry
            .retired
            .routes
            .get(&source)?
            .get(&route_id.to_uppercase())?;
        Some(IndexedRoute {
            color: retired.color.clone(),
            shape_ids: retired.shape_ids.clone(),
        })
    }

    pub fn retired_stop_record(&self, source: Source, stop_id: &str) -> Option<RetiredStop> {
        let registry = self.registry.read().unwrap();
        registry
            .retired
            .stops
            .get(&source)?
            .get(&stop_id.to_uppercase())
            .cloned()
    }

    pub fn retired_route_record(&self, source: Source, route_id: &str) -> Option<RetiredRoute> {
        let registry = self.registry.read().unwrap();
        registry
            .retired
            .routes
            .get(&source)?
            .get(&route_id.to_uppercase())
            .cloned()
    }

    pub fn retired_shape_record(&self, source: Source, shape_id: &str) -> Option<RetiredShape> {
        let registry = self.registry.read().unwrap();
        registry.retired.shapes.get(&source)?.get(shape_id).cloned()
    }

    fn with_registry<T>(&self, f: impl FnOnce(&NetworkRegistry) -> T) -> T {
        let registry = self.registry.read().unwrap();
        f(&registry)
    }
}

#[cfg(test)]
#[path = "tests/index.rs"]
mod tests;
