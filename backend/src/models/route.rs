use serde::{Deserialize, Serialize};
use sqlx::prelude::FromRow;
use utoipa::ToSchema;

use crate::{
    impl_discriminated_data,
    models::{source::Source, stop::Borough},
};

#[derive(Serialize, Deserialize, ToSchema, FromRow)]
pub struct Route {
    #[schema(example = "1")]
    pub id: String,
    // pub source: Source,
    #[schema(example = "Broadway - 7 Avenue Local")]
    pub long_name: String,
    #[schema(example = "1")]
    pub short_name: String,
    #[schema(example = "#EE352E")]
    pub color: String,
    #[schema(example = "#FFFFFF")]
    pub text_color: String,
    #[sqlx(flatten)]
    pub data: RouteData,
}

/// One of a bus route's two terminals, as printed on the bus's headsign.
#[derive(Debug, Serialize, Deserialize, ToSchema, Clone)]
pub struct MtaBusDirection {
    /// Matches `Trip.direction` for trips on this route.
    pub direction_id: i16,
    #[schema(example = "Manhattan Beach Kingsboro CC")]
    pub destination: String,
    /// Intermediate streets the MTA prints under the destination, e.g. `["Av U"]`.
    #[serde(default)]
    pub via: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema, Clone)]
pub struct MtaBusRouteData {
    pub sort_key: i32,
    pub service_types: Vec<String>,
    /// Borough the route primarily serves. `None` for the routes the MTA does not
    /// assign one to (mostly shuttles and inter-borough expresses).
    pub borough: Option<Borough>,
    /// Letter prefix of the route name, e.g. `Bx` in `Bx12`. Casing is whatever
    /// the feed uses for that route, which is not consistent across the Bronx
    /// routes (both `Bx` and `BX` appear), so compare case-insensitively.
    #[schema(example = "B")]
    pub name_prefix: String,
    #[schema(example = 1)]
    pub name_number: i32,
    /// Variant suffix, e.g. `-SBS` in `M15-SBS` or `A` in `Bx18A`.
    pub name_suffix: Option<String>,
    /// Destination headsigns, ordered by `direction_id`.
    ///
    /// Headsigns live here rather than on `RouteStop` because the upstream feed
    /// exposes no stop-to-direction mapping — a stop's route entry carries only
    /// shape IDs. Consumers resolve a trip's headsign from its `direction`.
    #[serde(default)]
    pub directions: Vec<MtaBusDirection>,
    // TODO: double check if theres a better way to link mta bus shapes to live trips.
    /// All shape IDs associated with this route (from Helium infra, via stop-route data).
    /// Used at realtime to pick the best-fitting shape for a trip when GTFS-RT
    /// does not provide shape_id directly.
    #[serde(default)]
    pub shape_ids: Vec<String>,
}

/// Stop data changes based on the `Source`
#[derive(Serialize, Deserialize, ToSchema)]
#[serde(tag = "source", rename_all = "snake_case")]
pub enum RouteData {
    MtaBus(MtaBusRouteData),
    MtaSubway,
    NjtBus,
}

impl_discriminated_data!(
    RouteData,
    Source,
    {
        MtaBus => MtaBusRouteData,
        MtaSubway,
        NjtBus,
    }
);
