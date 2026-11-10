pub mod alerts;
pub mod api;
pub mod app;
#[cfg(feature = "fixture-capture")]
pub mod fixtures;
pub mod geometry;
pub mod integrations;
pub mod network;
pub mod operations;
mod postgres;
pub mod source;
pub mod sources;
pub mod telemetry;
pub mod trajectory;
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
    pub readiness: crate::app::Readiness,
    pub alerts: crate::alerts::store::Alerts,
    pub network: crate::network::catalog::CatalogReader,
    pub operations: crate::operations::service::Operations,
    pub trajectories: crate::trajectory::TrajectoryService,
}

pub fn mta_oba_api_key() -> &'static str {
    static API_KEY: OnceLock<String> = OnceLock::new();
    API_KEY.get_or_init(|| var("MTA_OBA_API_KEY").expect("MTA_OBA_API_KEY must be set"))
}

pub fn valhalla_tile_extract() -> &'static str {
    static VALHALLA_TILE_EXTRACT: OnceLock<String> = OnceLock::new();
    VALHALLA_TILE_EXTRACT.get_or_init(|| {
        var("VALHALLA_TILE_EXTRACT").unwrap_or_else(|_| "/data/valhalla_tiles.tar".into())
    })
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

fn otel_signal_enabled(exporter_key: &str, endpoint_key: &str) -> bool {
    if var("OTEL_SDK_DISABLED").as_deref() == Ok("true")
        || var(exporter_key).as_deref() == Ok("none")
    {
        return false;
    }
    var("OTEL_EXPORTER_OTLP_ENDPOINT").is_ok() || var(endpoint_key).is_ok()
}

pub fn otel_traces_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        otel_signal_enabled("OTEL_TRACES_EXPORTER", "OTEL_EXPORTER_OTLP_TRACES_ENDPOINT")
    })
}

pub fn otel_metrics_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        otel_signal_enabled(
            "OTEL_METRICS_EXPORTER",
            "OTEL_EXPORTER_OTLP_METRICS_ENDPOINT",
        )
    })
}

pub fn otel_logs_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        otel_signal_enabled("OTEL_LOGS_EXPORTER", "OTEL_EXPORTER_OTLP_LOGS_ENDPOINT")
    })
}

pub fn otel_service_name() -> Option<&'static str> {
    static SERVICE_NAME: OnceLock<Option<String>> = OnceLock::new();
    SERVICE_NAME
        .get_or_init(|| var("OTEL_SERVICE_NAME").ok())
        .as_deref()
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
