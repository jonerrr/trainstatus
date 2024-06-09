use rayon::prelude::*;
use serde::{Deserialize, Deserializer};
use sqlx::{PgPool, QueryBuilder};
use std::{env::var, sync::OnceLock};

// https://stackoverflow.com/a/77249700
fn api_key() -> &'static str {
    // you need bustime api key to run this
    static API_KEY: OnceLock<String> = OnceLock::new();
    API_KEY.get_or_init(|| var("API_KEY").unwrap())
}

#[derive(Deserialize, Clone, Debug)]
pub struct AgencyRoute {
    pub color: String,
    pub description: String,
    pub id: String,
    #[serde(rename = "longName")]
    pub long_name: String,
    #[serde(rename = "shortName")]
    pub short_name: String,
    #[serde(rename = "type")]
    // 3 = bus, 711 = shuttle
    pub route_type: i16,
}

impl AgencyRoute {
    pub async fn get_all() -> Vec<Self> {
        // get all of the bus routes
        let nyct_routes: serde_json::Value = reqwest::Client::new()
            .get("http://bustime.mta.info/api/where/routes-for-agency/MTA%20NYCT.json")
            .query(&[("key", api_key())])
            .send()
            .await
            .unwrap()
            .json::<serde_json::Value>()
            .await
            .unwrap();
        let bc_routes: serde_json::Value = reqwest::Client::new()
            .get("http://bustime.mta.info/api/where/routes-for-agency/MTABC.json")
            .query(&[("key", api_key())])
            .send()
            .await
            .unwrap()
            .json::<serde_json::Value>()
            .await
            .unwrap();

        // combine the two lists
        let mut routes: Vec<Self> =
            serde_json::from_value(nyct_routes["data"]["list"].to_owned()).unwrap();
        routes.extend(
            serde_json::from_value::<Vec<Self>>(bc_routes["data"]["list"].to_owned()).unwrap(),
        );

        routes
    }

    pub async fn route_stops(&self) -> Vec<RouteStop> {
        todo!("bus stuff")
    }
}

#[derive(Deserialize, Clone, Debug)]
pub struct RouteStop {}

pub async fn stops_and_routes(pool: &PgPool) {
    let route_stops = AgencyRoute::get_all().await;

    dbg!(route_stops.len());
    // todo!("bus stuff")
}
