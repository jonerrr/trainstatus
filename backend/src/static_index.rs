use std::{collections::HashMap, sync::Arc};

use crate::{
    models::{
        geom::Geom,
        route::RouteData,
        source::Source,
        static_dataset::StaticDataset,
        stop::{RouteStopData, StopData},
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
