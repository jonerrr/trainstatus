use serde::Deserialize;
use serde_json::json;
use sqlx::{PgPool, QueryBuilder};
use zip::read::ZipFile;

pub struct Route {
    pub id: String,
    pub long_name: String,
    pub short_name: String,
    pub color: String,
    pub shuttle: bool,
    pub geom: serde_json::Value,
    pub route_type: RouteType,
}

#[derive(sqlx::Type)]
#[sqlx(type_name = "route_type", rename_all = "snake_case")]
pub enum RouteType {
    Train,
    Bus,
}

impl Route {
    pub async fn get_bus() -> Vec<Self> {
        todo!("return bus")
    }

    pub async fn get_train(routes_file: ZipFile<'_>) -> Vec<Self> {
        let mut rdr = csv::Reader::from_reader(routes_file);
        let routes = rdr
            .deserialize()
            .collect::<Result<Vec<GtfsRoute>, csv::Error>>()
            .unwrap();
        routes
            .into_iter()
            .filter_map(|r| {
                // filter out express routes
                if r.route_id.ends_with("X") {
                    None
                } else {
                    Some(r.into())
                }
            })
            .collect::<Vec<Route>>()
    }

    pub async fn insert(routes: Vec<Self>, pool: &PgPool) -> Result<(), sqlx::Error> {
        let mut query_builder = QueryBuilder::new(
            "INSERT INTO route (id, long_name, short_name, color, shuttle, geom, route_type)",
        );
        query_builder.push_values(routes, |mut b, route| {
            b.push_bind(route.id)
                .push_bind(route.long_name)
                .push_bind(route.short_name)
                .push_bind(route.color)
                .push_bind(route.shuttle)
                .push_bind(route.geom)
                .push_bind(route.route_type);
        });
        query_builder.push("ON CONFLICT (id) DO UPDATE SET long_name = EXCLUDED.long_name, short_name = EXCLUDED.short_name, color = EXCLUDED.color, shuttle = EXCLUDED.shuttle, geom = EXCLUDED.geom, route_type = EXCLUDED.route_type");
        let query = query_builder.build();
        query.execute(pool).await?;

        Ok(())
    }
}

// impl Route {
//     pub async fn bus() -> Vec<Self> {}
// }

#[derive(Deserialize)]
pub struct GtfsRoute {
    route_id: String,
    route_short_name: String,
    route_long_name: String,
    route_color: String,
}

impl From<GtfsRoute> for Route {
    fn from(value: GtfsRoute) -> Self {
        // let mut route_id = value.route_id.clone();
        // if value.route_id.ends_with('X') {
        //     route_id.pop();
        // }

        Route {
            id: value.route_id,
            long_name: value.route_long_name,
            short_name: value.route_short_name,
            color: value.route_color,
            shuttle: false,
            geom: json!({}),
            route_type: RouteType::Train,
        }
    }
}
