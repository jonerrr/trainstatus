use std::{collections::HashMap, sync::Arc};

use crate::{
    models::{
        geom::Geom,
        route::{Route, RouteData},
        shape::Shape,
        source::Source,
        static_dataset::StaticDataset,
        stop::{RouteStopData, Stop, StopData},
    },
    utils::source_snapshot::SourceSnapshot,
};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct TripPattern {
    pub route_id: String,
    pub direction: i16,
    pub shape_id: Option<String>,
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
pub struct StaticTransitRevision {
    pub source: Source,
    pub routes: HashMap<String, IndexedRoute>,
    pub stops: HashMap<String, IndexedStop>,
    pub shapes: HashMap<String, Geom>,
    pub route_stop_shapes: HashMap<(String, String), Vec<String>>,
    pub trip_patterns: HashMap<String, TripPattern>,
    pub stop_remap: HashMap<String, String>,
}

impl StaticTransitRevision {
    pub fn from_dataset(dataset: &StaticDataset) -> Self {
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
            routes,
            stops,
            shapes,
            route_stop_shapes,
            trip_patterns: dataset.trip_patterns.clone(),
            stop_remap: dataset.stop_remap.clone(),
        }
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
        let dataset = StaticDataset {
            source,
            routes,
            stops,
            route_stops,
            shapes,
            cached_trips: Vec::new(),
            trip_patterns,
            stop_remap,
        };
        Some(Self::from_dataset(&dataset))
    }
}

#[derive(Clone, Default)]
pub struct StaticTransitIndex {
    revisions: Arc<SourceSnapshot<StaticTransitRevision>>,
}

impl StaticTransitIndex {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn publish(&self, revision: StaticTransitRevision) {
        self.revisions.replace(revision.source, revision);
    }

    pub fn get(&self, source: Source) -> Option<Arc<StaticTransitRevision>> {
        self.revisions.get(source)
    }
}

#[cfg(test)]
#[path = "tests/static_index.rs"]
mod tests;
