use crate::geometry::Geom;
use crate::network::types::stop::StopData;
use crate::source::Source;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RetiredRoute {
    pub color: String,
    pub long_name: String,
    pub short_name: String,
    pub shape_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetiredStop {
    pub name: String,
    pub data: StopData,
    pub geom: Geom,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetiredShape {
    pub asset_id: crate::storage::content::ContentId,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RetiredNetworkIndex {
    pub routes: HashMap<Source, HashMap<String, RetiredRoute>>,
    pub stops: HashMap<Source, HashMap<String, RetiredStop>>,
    pub shapes: HashMap<Source, HashMap<String, RetiredShape>>,
}

/// Records newly retired entities this import; used to append blob logs without re-reading history.
#[derive(Debug, Clone, Default)]
pub struct RetiredImportDelta {
    pub stop_logs: Vec<crate::network::chunks::RetiredStopRecord>,
    pub shape_logs: Vec<crate::network::chunks::RetiredShapeRecord>,
    pub compact_stop_shapes: bool,
}
