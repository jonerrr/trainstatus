use super::schedule::ScheduledTrip;
use crate::{
    models::{route::Route, shape::Shape, source::Source, stop::RouteStop, stop::Stop},
    static_data::index::TripPattern,
    utils::validation::ImportReport,
};

pub struct StaticDataset {
    pub source: Source,
    pub routes: Vec<Route>,
    pub stops: Vec<Stop>,
    pub route_stops: Vec<RouteStop>,
    pub shapes: Vec<Shape>,
    pub scheduled_trips: Vec<ScheduledTrip>,
    pub trip_patterns: std::collections::HashMap<String, TripPattern>,
    pub stop_remap: std::collections::HashMap<String, String>,
}
impl StaticDataset {
    pub fn new(source: Source) -> Self {
        Self {
            source,
            routes: Vec::new(),
            stops: Vec::new(),
            route_stops: Vec::new(),
            shapes: Vec::new(),
            scheduled_trips: Vec::new(),
            trip_patterns: std::collections::HashMap::new(),
            stop_remap: std::collections::HashMap::new(),
        }
    }

    pub fn validate(&self) -> anyhow::Result<ImportReport> {
        ImportReport::generate(self)
    }
}
