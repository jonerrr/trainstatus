use super::schedule::ScheduledTrip;
use crate::network::index::TripPattern;
use crate::network::types::route::Route;
use crate::network::types::shape::Shape;
use crate::network::types::stop::RouteStop;
use crate::network::types::stop::Stop;
use crate::source::Source;

pub struct NetworkDataset {
    pub source: Source,
    pub routes: Vec<Route>,
    pub stops: Vec<Stop>,
    pub route_stops: Vec<RouteStop>,
    pub shapes: Vec<Shape>,
    pub scheduled_trips: Vec<ScheduledTrip>,
    pub trip_patterns: std::collections::HashMap<String, TripPattern>,
    pub stop_remap: std::collections::HashMap<String, String>,
}
impl NetworkDataset {
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
}
