use crate::geometry::Geom;
use crate::network::types::route::*;
use crate::network::types::shape::*;
use crate::network::types::stop::*;
use crate::source::Source;
pub(super) struct RouteRow {
    pub(super) source: Source,
    pub(super) id: String,
    pub(super) long_name: String,
    pub(super) short_name: String,
    pub(super) color: String,
    pub(super) text_color: Option<String>,
    pub(super) data: serde_json::Value,
}
impl TryFrom<RouteRow> for Route {
    type Error = anyhow::Error;
    fn try_from(row: RouteRow) -> anyhow::Result<Self> {
        Ok(Self {
            id: row.id,
            long_name: row.long_name,
            short_name: row.short_name,
            color: row.color,
            // Route already requires a text color. The SQL row reflects the
            // nullable database column; reject NULL when building the domain type.
            text_color: row
                .text_color
                .ok_or_else(|| anyhow::anyhow!("Route has no text color"))?,
            data: crate::persistence::decode(row.source, row.data)?,
        })
    }
}
#[derive(sqlx::FromRow)]
pub(super) struct StopRow {
    pub(super) source: Source,
    pub(super) id: String,
    pub(super) name: String,
    pub(super) geom: Geom,
    #[sqlx(json)]
    pub(super) transfers: Vec<Transfer>,
    #[sqlx(json)]
    pub(super) data: serde_json::Value,
    #[sqlx(json)]
    pub(super) routes: Vec<RouteStop>,
}
impl TryFrom<StopRow> for Stop {
    type Error = anyhow::Error;
    fn try_from(row: StopRow) -> anyhow::Result<Self> {
        Ok(Self {
            id: row.id,
            name: row.name,
            geom: row.geom,
            transfers: row.transfers,
            data: crate::persistence::decode(row.source, row.data)?,
            routes: row.routes,
        })
    }
}
pub(super) struct ShapeRow {
    pub(super) id: String,
    pub(super) source: Source,
    pub(super) geom: Geom,
    pub(super) data: serde_json::Value,
}
impl From<ShapeRow> for Shape {
    fn from(row: ShapeRow) -> Self {
        Self {
            id: row.id,
            source: row.source,
            geom: row.geom,
            data: row.data,
        }
    }
}
