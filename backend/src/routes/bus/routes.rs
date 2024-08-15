use crate::routes::errors::ServerError;
use axum::{extract::State, response::IntoResponse, Json};
use geo::MultiLineString;
use geojson::{feature::Id, Feature, FeatureCollection, Geometry, JsonObject, Value};
use http::HeaderMap;
use rayon::prelude::*;
use serde::Serialize;
use sqlx::PgPool;

#[derive(Serialize)]
pub struct Route {
    pub id: String,
    pub long_name: String,
    pub short_name: String,
    pub color: String,
    pub shuttle: bool,
}

pub async fn get(State(pool): State<PgPool>) -> Result<impl IntoResponse, ServerError> {
    let routes = sqlx::query_as!(
        Route,
        r#"
        SELECT
            br.id,
            br.long_name,
            br.short_name,
            br.color,
            br.shuttle
        FROM
            bus_routes br
        ORDER BY
            id;
    "#
    )
    .fetch_all(&pool)
    .await?;

    let mut headers = HeaderMap::new();
    // cache for a week
    headers.insert("cache-control", "public, max-age=604800".parse().unwrap());

    Ok((headers, Json(routes)))
}

pub async fn geojson(State(pool): State<PgPool>) -> Result<impl IntoResponse, ServerError> {
    let routes = sqlx::query!(
        r#"
            SELECT
                *
            FROM
                bus_routes
            ORDER BY 
                id;"#
    )
    .fetch_all(&pool)
    .await?;

    let features = routes
        .into_par_iter()
        .map(|r| {
            let mut properties = JsonObject::new();
            properties.insert("id".to_string(), r.id.clone().into());
            properties.insert("long_name".to_string(), r.long_name.into());
            properties.insert("short_name".to_string(), r.short_name.into());
            properties.insert("color".to_string(), format!("#{}", r.color).into());
            properties.insert("shuttle".to_string(), r.shuttle.into());
            // not sure if i should be using unwrap here
            let value = serde_json::from_value::<MultiLineString>(r.geom).unwrap();

            Feature {
                geometry: Some(Geometry {
                    value: Value::from(&value),
                    bbox: None,
                    foreign_members: None,
                }),
                id: Some(Id::String(r.id)),
                bbox: None,
                properties: Some(properties),
                foreign_members: None,
            }
        })
        .collect::<Vec<_>>();

    let geojson = FeatureCollection {
        features,
        foreign_members: None,
        bbox: None,
    };

    Ok(Json(geojson))
}
