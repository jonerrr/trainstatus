use crate::geometry::Geom;
use crate::network::types::route::MtaBusDirection;
use crate::network::types::route::Route;
use crate::network::types::route::RouteData;
use crate::network::types::stop::Borough;
use crate::network::types::stop::RouteStop;
use crate::network::types::stop::RouteStopData;
use crate::network::types::stop::Stop;
use crate::network::types::stop::StopData;
use crate::network::types::stop::StopType;
use crate::network::types::stop::{NearbyStop, NjtBusStopDirection};
use crate::source::Source;
use serde::Serialize;
use utoipa::ToSchema;

#[derive(Serialize, ToSchema)]
pub struct Catalog {
    pub source: Source,
    pub routes: Vec<CatalogRoute>,
    pub stops: Vec<CatalogStop>,
}

#[derive(Serialize, ToSchema)]
pub struct CatalogRoute {
    #[schema(example = "1")]
    pub id: String,
    #[schema(example = "1")]
    pub short_name: String,
    #[schema(example = "Broadway - 7 Avenue Local")]
    pub long_name: String,
    #[schema(example = "#EE352E")]
    pub color: String,
    #[schema(example = "#FFFFFF")]
    pub text_color: String,
    pub data: CatalogRouteData,
}

#[derive(Serialize, ToSchema)]
pub struct MtaBusCatalogRoute {
    pub sort_key: i32,
    pub service_types: Vec<String>,
    pub borough: Option<Borough>,
    pub name_prefix: String,
    pub name_number: i32,
    pub name_suffix: Option<String>,
    /// Destination headsigns; `direction_id` matches a service trip's `direction`.
    pub directions: Vec<MtaBusDirection>,
}

#[derive(Serialize, ToSchema)]
#[serde(tag = "source", rename_all = "snake_case")]
pub enum CatalogRouteData {
    MtaSubway,
    MtaBus(MtaBusCatalogRoute),
    NjtBus,
}

#[derive(Serialize, ToSchema)]
pub struct CatalogStop {
    #[schema(example = "101")]
    pub id: String,
    #[schema(example = "Van Cortlandt Park-242 St")]
    pub name: String,
    /// WGS 84 latitude in degrees.
    pub lat: f64,
    /// WGS 84 longitude in degrees.
    pub lon: f64,
    pub nearby_stops: Vec<NearbyStop>,
    pub data: StopData,
    /// Routes serving this stop, ordered by route ID.
    pub routes: Vec<CatalogStopRoute>,
}

#[derive(Serialize, ToSchema)]
pub struct CatalogStopRoute {
    pub route_id: String,
    /// Static position of the stop along the route.
    pub stop_sequence: i16,
    pub data: CatalogStopRouteData,
}

#[derive(Serialize, ToSchema)]
#[serde(tag = "source", rename_all = "snake_case")]
pub enum CatalogStopRouteData {
    MtaSubway {
        stop_type: StopType,
    },
    MtaBus {
        /// Published route direction IDs serving this stop. Empty means unknown.
        direction_ids: Vec<i16>,
    },
    NjtBus {
        directions: Vec<NjtBusStopDirection>,
    },
}

impl Catalog {
    pub fn new(source: Source, routes: &[Route], stops: &[Stop]) -> Self {
        Self {
            source,
            routes: routes.iter().map(CatalogRoute::from).collect(),
            stops: stops.iter().map(CatalogStop::from).collect(),
        }
    }
}

impl From<&Route> for CatalogRoute {
    fn from(route: &Route) -> Self {
        Self {
            id: route.id.clone(),
            short_name: route.short_name.clone(),
            long_name: route.long_name.clone(),
            color: route.color.clone(),
            text_color: route.text_color.clone(),
            data: match &route.data {
                RouteData::MtaSubway => CatalogRouteData::MtaSubway,
                RouteData::NjtBus => CatalogRouteData::NjtBus,
                RouteData::MtaBus(data) => CatalogRouteData::MtaBus(MtaBusCatalogRoute {
                    sort_key: data.sort_key,
                    service_types: data.service_types.clone(),
                    borough: data.borough,
                    name_prefix: data.name_prefix.clone(),
                    name_number: data.name_number,
                    name_suffix: data.name_suffix.clone(),
                    directions: data.directions.clone(),
                }),
            },
        }
    }
}

impl From<&Stop> for CatalogStop {
    fn from(stop: &Stop) -> Self {
        let (lon, lat) = point(&stop.geom);
        Self {
            id: stop.id.clone(),
            name: stop.name.clone(),
            lat,
            lon,
            nearby_stops: stop.nearby_stops.clone(),
            data: stop.data.clone(),
            routes: stop.routes.iter().map(CatalogStopRoute::from).collect(),
        }
    }
}

// A stop already supplies its ID. Omit that redundant field and the MTA bus
// shape candidates used only by trajectory resolution.
impl From<&RouteStop> for CatalogStopRoute {
    fn from(route_stop: &RouteStop) -> Self {
        Self {
            route_id: route_stop.route_id.clone(),
            stop_sequence: route_stop.stop_sequence,
            data: match &route_stop.data {
                RouteStopData::MtaSubway { stop_type } => CatalogStopRouteData::MtaSubway {
                    stop_type: stop_type.clone(),
                },
                RouteStopData::MtaBus { direction_ids, .. } => CatalogStopRouteData::MtaBus {
                    direction_ids: direction_ids.clone(),
                },
                RouteStopData::NjtBus { directions } => CatalogStopRouteData::NjtBus {
                    directions: directions.clone(),
                },
            },
        }
    }
}

fn point(geom: &Geom) -> (f64, f64) {
    match &geom.0 {
        geo::Geometry::Point(point) => (point.x(), point.y()),
        other => geo::Centroid::centroid(other).map_or((0., 0.), |point| (point.x(), point.y())),
    }
}

pub fn encode(
    source: Source,
    routes: &[Route],
    stops: &[Stop],
) -> anyhow::Result<crate::network::catalog::CatalogResponse> {
    Ok(crate::network::catalog::CatalogResponse::new(
        bytes::Bytes::from(serde_json::to_vec(&Catalog::new(source, routes, stops))?),
    ))
}
