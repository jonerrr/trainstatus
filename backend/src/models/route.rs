use serde::{Deserialize, Serialize};
use sqlx::prelude::FromRow;
use utoipa::ToSchema;

use crate::{
    impl_discriminated_data,
    models::{geom::Geom, source::Source},
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
    #[sqlx(flatten)]
    pub data: RouteData,
    // maybe change to wkt
    #[serde(skip_serializing_if = "Option::is_none")]
    // TODO: correct schema
    // #[schema(schema_with = crate::static_data::stop::point_schema)]
    #[serde(skip_deserializing)]
    pub geom: Option<Geom>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct MtaBusData {
    pub shuttle: bool,
}

/// Stop data changes based on the `Source`
#[derive(Serialize, Deserialize, ToSchema)]
#[serde(tag = "source", rename_all = "snake_case")]
pub enum RouteData {
    MtaBus(MtaBusData),
    MtaSubway,
}

impl_discriminated_data!(
    RouteData,
    Source,
    {
        MtaBus => MtaBusData,
        MtaSubway,
    }
);
