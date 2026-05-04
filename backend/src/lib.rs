pub mod api;
pub mod engines;
pub mod integrations;
pub mod macros;
pub mod models;
pub mod sources;
pub mod stores;
pub mod utils;

use std::env::var;
use std::sync::OnceLock;

#[allow(clippy::all)]
pub mod feed {
    include!(concat!(env!("OUT_DIR"), "/transit_realtime.rs"));
}

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone)]
pub struct AppState {
    pub route_store: crate::stores::route::RouteStore,
    pub stop_store: crate::stores::stop::StopStore,
    pub trip_store: crate::stores::trip::TripStore,
    pub stop_time_store: crate::stores::stop_time::StopTimeStore,
    pub position_store: crate::stores::position::PositionStore,
    pub alert_store: crate::stores::alert::AlertStore,
    pub static_cache_store: crate::stores::static_cache::StaticCacheStore,
}

impl AppState {
    pub fn new(
        route_store: crate::stores::route::RouteStore,
        stop_store: crate::stores::stop::StopStore,
        trip_store: crate::stores::trip::TripStore,
        stop_time_store: crate::stores::stop_time::StopTimeStore,
        position_store: crate::stores::position::PositionStore,
        alert_store: crate::stores::alert::AlertStore,
        static_cache_store: crate::stores::static_cache::StaticCacheStore,
    ) -> Self {
        Self {
            route_store,
            stop_store,
            trip_store,
            stop_time_store,
            position_store,
            alert_store,
            static_cache_store,
        }
    }
}

// pub fn mta_api_url() -> &'static str {
//     static API_URL: OnceLock<String> = OnceLock::new();
//     API_URL.get_or_init(|| var("MTA_API_URL").expect("MTA_API_URL must be set"))
// }

pub fn mta_oba_api_key() -> &'static str {
    static API_KEY: OnceLock<String> = OnceLock::new();
    API_KEY.get_or_init(|| var("MTA_OBA_API_KEY").expect("MTA_OBA_API_KEY must be set"))
}

pub fn valhalla_config() -> &'static str {
    static VALHALLA_CONFIG: OnceLock<String> = OnceLock::new();
    VALHALLA_CONFIG
        .get_or_init(|| var("VALHALLA_CONFIG").unwrap_or_else(|_| "/data/valhalla.json".into()))
}

pub fn debug_rt_data() -> &'static bool {
    static DEBUG_RT_DATA: OnceLock<bool> = OnceLock::new();
    DEBUG_RT_DATA.get_or_init(|| var("DEBUG_RT_DATA").is_ok())
}

pub fn api_prefix() -> &'static str {
    static API_PREFIX: OnceLock<String> = OnceLock::new();
    API_PREFIX.get_or_init(|| {
        let raw = var("API_PREFIX").unwrap_or_else(|_| "/api".into());
        let trimmed = raw.trim();

        if trimmed.is_empty() || trimmed == "/" {
            return "/".into();
        }

        format!("/{}", trimmed.trim_matches('/'))
    })
}

pub fn prefixed_path(prefix: &str, path: &str) -> String {
    if prefix == "/" {
        return format!("/{}", path.trim_start_matches('/'));
    }

    format!(
        "{}/{}",
        prefix.trim_end_matches('/'),
        path.trim_start_matches('/')
    )
}
