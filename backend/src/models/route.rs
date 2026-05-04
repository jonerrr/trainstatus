use serde::{Deserialize, Serialize};
use sqlx::prelude::FromRow;
use utoipa::ToSchema;

use crate::{
    impl_discriminated_data,
    models::source::Source,
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
    #[schema(example = "#FFFFFF")]
    pub text_color: String,
    #[sqlx(flatten)]
    pub data: RouteData,
}

#[derive(Debug, Serialize, Deserialize, ToSchema, Clone)]
pub struct MtaBusRouteData {
    pub sort_key: i32,
    pub service_types: Vec<String>,
}

/// Stop data changes based on the `Source`
#[derive(Serialize, Deserialize, ToSchema)]
#[serde(tag = "source", rename_all = "snake_case")]
pub enum RouteData {
    MtaBus(MtaBusRouteData),
    MtaSubway,
    NjtBus,
}

impl_discriminated_data!(
    RouteData,
    Source,
    {
        MtaBus => MtaBusRouteData,
        MtaSubway,
        NjtBus,
    }
);
