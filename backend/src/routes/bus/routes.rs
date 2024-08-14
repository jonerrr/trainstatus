use crate::routes::errors::ServerError;
use axum::{
    extract::{Query, State},
    response::IntoResponse,
    Json,
};
use http::HeaderMap;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

#[derive(Serialize)]
pub struct Route {
    pub id: String,
    pub long_name: String,
    pub short_name: String,
    pub color: String,
    pub shuttle: bool,
    pub wkt: Option<String>,
}

fn no_wkt() -> bool {
    false
}
#[derive(Deserialize)]
pub struct Parameters {
    #[serde(default = "no_wkt")]
    wkt: bool,
}

pub async fn get(
    State(pool): State<PgPool>,
    params: Query<Parameters>,
) -> Result<impl IntoResponse, ServerError> {
    let routes = match params.wkt {
        true => {
            sqlx::query_as!(
                Route,
                r#"
                    SELECT
                        *
                    FROM
                        bus_routes;"#
            )
            .fetch_all(&pool)
            .await?
        }
        false => {
            sqlx::query_as!(
                Route,
                r#"
                    SELECT
                        br.id,
                        br.long_name,
                        br.short_name,
                        br.color,
                        br.shuttle,
                        NULL as "wkt: String"
                    FROM
                        bus_routes br
                "#
            )
            .fetch_all(&pool)
            .await?
        }
    };

    let mut headers = HeaderMap::new();
    // cache for a week
    headers.insert("cache-control", "public, max-age=604800".parse().unwrap());

    Ok((headers, Json(routes)))
}
