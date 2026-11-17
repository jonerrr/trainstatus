use super::schedule::ScheduledTrip;
use crate::network::index::TripPattern;
use crate::network::types::route::Route;
use crate::network::types::shape::Shape;
use crate::network::types::stop::RouteStop;
use crate::network::types::stop::Stop;
use crate::source::Source;
use std::collections::HashMap;

/// Collected import records, validated and written in one transaction. The committed
/// `NetworkRevision` retains only shared lookup indexes and the resulting catalog;
/// collectors never publish a revision directly.
#[derive(Clone)]
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
/// Join `route_stops` onto each stop's `routes` field (collectors leave `routes` empty).
pub fn attach_stop_routes(stops: &mut [Stop], route_stops: &[RouteStop]) {
    let mut memberships: HashMap<String, Vec<RouteStop>> = HashMap::new();
    for membership in route_stops {
        memberships
            .entry(membership.stop_id.to_uppercase())
            .or_default()
            .push(membership.clone());
    }
    for stop in stops {
        stop.id = stop.id.to_uppercase();
        let mut routes = memberships.remove(&stop.id).unwrap_or_default();
        routes.sort_by(|left, right| {
            left.route_id
                .cmp(&right.route_id)
                .then_with(|| left.stop_sequence.cmp(&right.stop_sequence))
        });
        stop.routes = routes;
    }
}

pub fn normalize_import_dataset(mut dataset: NetworkDataset) -> anyhow::Result<NetworkDataset> {
    for route in &mut dataset.routes {
        route.id = route.id.to_uppercase();
        route.color = crate::utils::color::normalize_hex_color(&route.color);
        route.text_color = crate::utils::color::normalize_hex_color(&route.text_color);
    }
    for route_stop in &mut dataset.route_stops {
        route_stop.route_id = route_stop.route_id.to_uppercase();
        route_stop.stop_id = route_stop.stop_id.to_uppercase();
    }
    attach_stop_routes(&mut dataset.stops, &dataset.route_stops);
    Ok(dataset)
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
