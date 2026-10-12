//! Static source catalog served to passenger clients.
//!
//! Field audit (frontend consumers as of the transit API rewrite):
//! - Routes: `id`, names, colors (badges, search, map styling); MTA bus `sort_key`,
//!   `service_types`, `borough`, name parts (sorting, filters, search) and `directions`
//!   (bus destination). Routing-only `shape_ids` are omitted.
//! - Stops: `id`, `name`, `lat`/`lon` (nearby sorting, map), `transfers` (transfer
//!   links), source `data` including subway headsigns and platform edges, and serving
//!   `routes`.
//! - Stop routes: `route_id`, `stop_sequence` (route ordering), and direction, headsign,
//!   subway stop type and bus opposite stop. The parent `stop_id` and routing-only
//!   `shape_ids` are omitted.
//!
//! `data.source` tags are retained so generated TypeScript types stay discriminated.
use crate::{
    models::{
        geom::Geom,
        route::{MtaBusDirection, Route, RouteData},
        source::Source,
        stop::{Borough, RouteStop, RouteStopData, Stop, StopData, StopType, Transfer},
    },
    stores::{response::StaticResponse, route::RouteStore, stop::StopStore},
    utils::source_snapshot::SourceSnapshot,
};
use axum::body::Bytes;
use serde::Serialize;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::Mutex;
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
    pub transfers: Vec<Transfer>,
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
        headsign: String,
        direction: i16,
        /// Estimated from proximity, direction and headsign.
        opposite_stop_id: Option<String>,
    },
    NjtBus {
        headsign: String,
        direction: i16,
        /// Estimated from proximity and direction.
        opposite_stop_id: Option<String>,
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
            transfers: stop.transfers.clone(),
            data: stop.data.clone(),
            routes: stop.routes.iter().map(CatalogStopRoute::from).collect(),
        }
    }
}

impl From<&RouteStop> for CatalogStopRoute {
    fn from(route_stop: &RouteStop) -> Self {
        Self {
            route_id: route_stop.route_id.clone(),
            stop_sequence: route_stop.stop_sequence,
            data: match &route_stop.data {
                RouteStopData::MtaSubway { stop_type } => CatalogStopRouteData::MtaSubway {
                    stop_type: stop_type.clone(),
                },
                RouteStopData::MtaBus {
                    headsign,
                    direction,
                    opposite_stop_id,
                    ..
                } => CatalogStopRouteData::MtaBus {
                    headsign: headsign.clone(),
                    direction: *direction,
                    opposite_stop_id: opposite_stop_id.clone(),
                },
                RouteStopData::NjtBus {
                    headsign,
                    direction,
                    opposite_stop_id,
                } => CatalogStopRouteData::NjtBus {
                    headsign: headsign.clone(),
                    direction: *direction,
                    opposite_stop_id: opposite_stop_id.clone(),
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

/// Encoded catalog with the static snapshots it was derived from.
pub struct CatalogResponse {
    pub body: Bytes,
    pub etag: String,
    routes: Arc<StaticResponse<Route>>,
    stops: Arc<StaticResponse<Stop>>,
}

/// Derives catalogs from the cached static route and stop snapshots, rebuilding only
/// when either snapshot has been replaced.
#[derive(Clone)]
pub struct CatalogStore {
    route_store: RouteStore,
    stop_store: StopStore,
    values: Arc<SourceSnapshot<CatalogResponse>>,
    build_locks: Arc<HashMap<Source, Mutex<()>>>,
}

impl CatalogStore {
    pub fn new(route_store: RouteStore, stop_store: StopStore) -> Self {
        Self {
            route_store,
            stop_store,
            values: Arc::default(),
            build_locks: Arc::new(
                Source::ALL
                    .into_iter()
                    .map(|source| (source, Mutex::new(())))
                    .collect(),
            ),
        }
    }

    pub async fn response(&self, source: Source) -> anyhow::Result<Arc<CatalogResponse>> {
        let routes = self.route_store.response(source).await?;
        let stops = self.stop_store.response(source).await?;
        let current = |value: &CatalogResponse| {
            Arc::ptr_eq(&value.routes, &routes) && Arc::ptr_eq(&value.stops, &stops)
        };
        if let Some(value) = self.values.get(source).filter(|value| current(value)) {
            return Ok(value);
        }
        let _guard = self.build_locks[&source].lock().await;
        if let Some(value) = self.values.get(source).filter(|value| current(value)) {
            return Ok(value);
        }
        let body = Bytes::from(serde_json::to_vec(&Catalog::new(
            source,
            &routes.data,
            &stops.data,
        ))?);
        let etag = blake3::hash(&body).to_hex().to_string();
        Ok(self.values.replace(
            source,
            CatalogResponse {
                body,
                etag,
                routes,
                stops,
            },
        ))
    }
}
