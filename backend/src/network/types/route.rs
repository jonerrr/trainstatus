use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::network::types::stop::Borough;

#[derive(Serialize, Deserialize, Clone)]
pub struct Route {
    pub id: String,
    // pub source: Source,
    pub long_name: String,
    pub short_name: String,
    pub color: String,
    pub text_color: String,
    pub data: RouteData,
}

/// Default headsign for one of a bus route's published direction IDs.
#[derive(Debug, Serialize, Deserialize, Clone, ToSchema)]
pub struct MtaBusDirection {
    /// Matches `TripRun.direction` for trips on this route.
    pub direction_id: i16,
    pub headsign: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MtaBusRouteData {
    pub sort_key: i32,
    pub service_types: Vec<String>,
    /// Borough the route primarily serves. `None` for the routes the MTA does not
    /// assign one to (mostly shuttles and inter-borough expresses).
    pub borough: Option<Borough>,
    /// Letter prefix of the route name, e.g. `Bx` in `Bx12`. Casing is whatever
    /// the feed uses for that route, which is not consistent across the Bronx
    /// routes (both `Bx` and `BX` appear), so compare case-insensitively.
    pub name_prefix: String,
    pub name_number: i32,
    /// Variant suffix, e.g. `-SBS` in `M15-SBS` or `A` in `Bx18A`.
    pub name_suffix: Option<String>,
    /// Helium display labels, ordered by `direction_id`; shared by trips and stop memberships.
    pub directions: Vec<MtaBusDirection>,
    // TODO: double check if theres a better way to link mta bus shapes to live trips.
    /// All shape IDs associated with this route (from Helium infra, via stop-route data).
    /// Used at realtime to pick the best-fitting shape for a trip when GTFS-RT
    /// does not provide shape_id directly.
    #[serde(default)]
    pub shape_ids: Vec<String>,
}

/// Stop data changes based on the `Source`
#[derive(Serialize, Deserialize, Clone)]
#[serde(tag = "source", rename_all = "snake_case")]
pub enum RouteData {
    MtaBus(MtaBusRouteData),
    MtaSubway,
    NjtBus,
}
