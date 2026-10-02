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
mod tests {
    use super::*;
    use crate::models::{
        route::{MtaBusRouteData, Route},
        stop::{NjtBusStopData, RouteStop, RouteStopData, Stop},
    };

    fn subway_route() -> Route {
        Route {
            id: "1".into(),
            long_name: "Broadway".into(),
            short_name: "1".into(),
            color: "EE352E".into(),
            text_color: "FFFFFF".into(),
            data: RouteData::MtaSubway,
        }
    }

    fn njt_stop() -> Stop {
        Stop {
            id: "100".into(),
            name: "Journal Square".into(),
            geom: Geom(geo::Geometry::Point(geo::Point::new(-74.06, 40.73))),
            transfers: Vec::new(),
            data: StopData::NjtBus(NjtBusStopData {
                stop_code: "100".into(),
            }),
            routes: Vec::new(),
        }
    }

    #[test]
    fn persisted_bus_revision_keeps_route_shape_membership() {
        let stop = Stop {
            id: "1".into(),
            name: "Bay Ridge".into(),
            geom: Geom(geo::Geometry::Point(geo::Point::new(-74.0, 40.6))),
            transfers: Vec::new(),
            data: StopData::MtaBus(crate::models::stop::MtaBusStopData {
                bearing: None,
                is_boardable: true,
                direction: crate::models::stop::CompassDirection::Unknown,
            }),
            routes: vec![RouteStop {
                route_id: "B63".into(),
                stop_id: "1".into(),
                stop_sequence: 1,
                data: RouteStopData::MtaBus {
                    headsign: "Bay Ridge".into(),
                    direction: 0,
                    opposite_stop_id: None,
                    shape_ids: vec!["shape-a".into()],
                },
            }],
        };
        let route = Route {
            id: "B63".into(),
            long_name: "B63".into(),
            short_name: "B63".into(),
            color: "B933AD".into(),
            text_color: "FFFFFF".into(),
            data: RouteData::MtaBus(MtaBusRouteData {
                sort_key: 0,
                service_types: vec!["local".into()],
                borough: None,
                name_prefix: "B".into(),
                name_number: 63,
                name_suffix: None,
                directions: Vec::new(),
                shape_ids: vec!["shape-a".into()],
            }),
        };
        let revision = StaticTransitRevision::try_from_persisted(
            Source::MtaBus,
            vec![route],
            vec![stop],
            Vec::new(),
            HashMap::new(),
            HashMap::new(),
        )
        .expect("bus static rows are enough to rebuild the index");
        assert_eq!(
            revision.routes["B63"].shape_ids,
            vec!["shape-a".to_string()]
        );
        assert_eq!(
            revision.route_stop_shapes[&("B63".into(), "1".into())],
            vec!["shape-a".to_string()]
        );
    }

    #[test]
    fn njt_revision_without_saved_patterns_is_not_reused() {
        assert!(
            StaticTransitRevision::try_from_persisted(
                Source::NjtBus,
                vec![subway_route()],
                vec![njt_stop()],
                Vec::new(),
                HashMap::new(),
                HashMap::new(),
            )
            .is_none()
        );
    }

    #[test]
    fn njt_revision_keeps_saved_patterns_and_stop_remap() {
        let mut patterns = HashMap::new();
        patterns.insert(
            "trip-1".into(),
            TripPattern {
                route_id: "87".into(),
                direction: 0,
                shape_id: Some("shape-1".into()),
            },
        );
        let mut remap = HashMap::new();
        remap.insert("child".into(), "100".into());
        let revision = StaticTransitRevision::try_from_persisted(
            Source::NjtBus,
            vec![subway_route()],
            vec![njt_stop()],
            Vec::new(),
            patterns,
            remap,
        )
        .expect("saved NJT patterns make the stored revision reusable");
        assert_eq!(
            revision.trip_patterns["trip-1"].shape_id.as_deref(),
            Some("shape-1")
        );
        assert_eq!(revision.stop_remap["child"], "100");
    }

    #[test]
    fn empty_tables_are_not_a_revision() {
        assert!(
            StaticTransitRevision::try_from_persisted(
                Source::MtaSubway,
                Vec::new(),
                Vec::new(),
                Vec::new(),
                HashMap::new(),
                HashMap::new(),
            )
            .is_none()
        );
    }
}
