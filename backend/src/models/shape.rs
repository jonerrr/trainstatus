use crate::models::{geom::Geom, source::Source};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::ToSchema;

#[derive(Serialize, Deserialize, ToSchema, FromRow, Debug)]
pub struct Shape {
    pub id: String,
    pub source: Source,
    pub geom: Geom,
    // TODO: use enum
    #[sqlx(json)]
    pub data: serde_json::Value,
}
