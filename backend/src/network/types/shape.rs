use crate::geometry::Geom;
use crate::source::Source;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Shape {
    pub id: String,
    pub source: Source,
    pub geom: Geom,
}
