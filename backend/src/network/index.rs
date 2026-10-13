use super::dataset::NetworkDataset;
use crate::geometry::Geom;
use crate::network::types::route::Route;
use crate::network::types::route::RouteData;
use crate::network::types::shape::Shape;
use crate::network::types::stop::RouteStopData;
use crate::network::types::stop::Stop;
use crate::network::types::stop::StopData;
use crate::source::Source;
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
    pub catalog: Option<Arc<crate::network::catalog::CatalogResponse>>,
    pub routes: Arc<HashMap<String, IndexedRoute>>,
    pub stops: Arc<HashMap<String, IndexedStop>>,
    pub shapes: Arc<HashMap<String, Geom>>,
    pub route_stop_shapes: Arc<HashMap<(String, String), Vec<String>>>,
    pub trip_patterns: Arc<HashMap<String, TripPattern>>,
    pub stop_remap: Arc<HashMap<String, String>>,
    pub scheduled_trips: Arc<HashMap<String, HashMap<String, super::schedule::ScheduledTripEntry>>>,
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
                .map(|trip| super::schedule::ScheduledTripEntry { trip, expires_at }),
        )
    }

    pub(crate) fn from_dataset_with_schedules(
        dataset: &NetworkDataset,
        schedules: impl IntoIterator<Item = super::schedule::ScheduledTripEntry>,
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
                    route.id.clone(),
                    IndexedRoute {
                        color: route.color.clone(),
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
                    stop.id.clone(),
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
                    (route_stop.route_id.clone(), route_stop.stop_id.clone()),
                    shape_ids,
                )
            })
            .collect();

        Self {
            source: dataset.source,
            catalog: None,
            routes: Arc::new(routes),
            stops: Arc::new(stops),
            shapes: Arc::new(shapes),
            route_stop_shapes: Arc::new(route_stop_shapes),
            trip_patterns: Arc::new(dataset.trip_patterns.clone()),
            stop_remap: Arc::new(dataset.stop_remap.clone()),
            scheduled_trips: Arc::new(schedule_index(schedules)),
        }
    }

    pub fn set_schedules(
        &mut self,
        trips: impl IntoIterator<Item = super::schedule::ScheduledTripEntry>,
    ) {
        self.scheduled_trips = Arc::new(schedule_index(trips));
    }

    pub fn scheduled_trip(
        &self,
        trip_id: &str,
        service_date: &str,
    ) -> Option<&super::schedule::ScheduledTrip> {
        let entry = self.scheduled_trips.get(trip_id)?.get(service_date)?;
        (entry.expires_at > chrono::Utc::now()).then_some(&entry.trip)
    }

    /// Rebuild the in-memory revision from rows already stored by the last import.
    ///
    /// Returns nothing when the tables are empty, and when an NJT revision has no
    /// trip patterns. Those patterns are required to place live trips, and an
    /// empty map means this process has not saved them yet.
    pub fn try_from_persisted(
        source: Source,
        routes: Vec<Route>,
        stops: Vec<Stop>,
        shapes: Vec<Shape>,
        trip_patterns: HashMap<String, TripPattern>,
        stop_remap: HashMap<String, String>,
    ) -> Option<Self> {
        if routes.is_empty() && stops.is_empty() {
            return None;
        }
        if source == Source::NjtBus && trip_patterns.is_empty() {
            return None;
        }
        let route_stops = stops
            .iter()
            .flat_map(|stop| stop.routes.iter().cloned())
            .collect();
        let dataset = NetworkDataset {
            source,
            routes,
            stops,
            route_stops,
            shapes,
            scheduled_trips: Vec::new(),
            trip_patterns,
            stop_remap,
        };
        Some(Self::from_dataset(&dataset))
    }
}

fn schedule_index(
    trips: impl IntoIterator<Item = super::schedule::ScheduledTripEntry>,
) -> HashMap<String, HashMap<String, super::schedule::ScheduledTripEntry>> {
    let mut index: HashMap<String, HashMap<String, super::schedule::ScheduledTripEntry>> =
        HashMap::new();
    for entry in trips {
        let trip = &entry.trip;
        index
            .entry(trip.trip_id.clone())
            .or_default()
            .insert(trip.start_date.clone(), entry);
    }
    index
}

#[derive(Clone, Default)]
pub struct NetworkIndex {
    revisions: Arc<std::sync::RwLock<HashMap<Source, Arc<NetworkRevision>>>>,
}

impl NetworkIndex {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn publish(&self, revision: NetworkRevision) {
        self.publish_bundle(vec![revision]);
    }

    pub fn publish_bundle(&self, revisions: Vec<NetworkRevision>) {
        let mut current = self.revisions.write().unwrap();
        for revision in revisions {
            current.insert(revision.source, Arc::new(revision));
        }
    }

    pub fn get(&self, source: Source) -> Option<Arc<NetworkRevision>> {
        self.revisions.read().unwrap().get(&source).cloned()
    }
}

#[cfg(test)]
#[path = "tests/index.rs"]
mod tests;
