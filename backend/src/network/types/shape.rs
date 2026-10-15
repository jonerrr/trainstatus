use crate::geometry::Geom;
use crate::source::Source;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct Shape {
    pub id: String,
    pub source: Source,
    pub geom: Geom,
    // TODO: use enum
    pub data: serde_json::Value,
}
