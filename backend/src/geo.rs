use geo::HaversineDistance;
use geo_types::Point;
use itertools::Itertools;
use rayon::prelude::*;
use sqlx::PgPool;
use std::fs::File;
use std::io::Write;

#[derive(Debug, Clone)]
pub struct StopGeometry<T, S> {
    pub id: T,
    pub name: String,
    pub point: Point<f32>,
    pub routes: Option<Vec<String>>,
    // pub stop_type: StopType,
    pub closest_stops: Option<Vec<StopGeometry<S, T>>>,
}

// #[derive(Debug)]
// pub enum ClosestStop {
//     BusStop(String),
//     TrainStop(String),
// }

// #[derive(Debug, Clone)]
// pub enum StopType {
//     Bus,
//     Train,
// }

pub async fn update_transfers(pool: &PgPool) {
    tracing::info!("Updating transfers");
    let stops = sqlx::query!(r#"SELECT lat, lon, id, name FROM stops"#)
        .fetch_all(pool)
        .await
        .unwrap();

    let stop_geos = stops
        .into_iter()
        .map(|s| StopGeometry::<String, i32> {
            id: s.id,
            routes: None,
            name: s.name.clone(),
            point: Point::new(s.lon, s.lat),
            closest_stops: None,
        })
        .collect::<Vec<_>>();

    let bus_stops = sqlx::query!(r#"SELECT bs.lat, bs.lon, bs.id, bs.name, brs.route_id FROM bus_stops bs JOIN bus_route_stops brs ON bs.id = brs.stop_id JOIN bus_routes b ON brs.route_id = b.id WHERE b.shuttle IS FALSE"#)
        .fetch_all(pool)
        .await
        .unwrap();
    let bus_stop_geos = bus_stops
        .into_iter()
        .map(|s| StopGeometry::<i32, String> {
            id: s.id,
            name: s.name.clone(),
            point: Point::new(s.lon, s.lat),
            closest_stops: None,
            routes: Some(vec![s.route_id]),
        })
        .collect::<Vec<_>>();

    let stop_geos = stop_geos
        .into_par_iter()
        .map(|sg| {
            let mut new_sg = sg.clone();

            let bus_stops = bus_stop_geos
                .iter()
                .enumerate()
                .map(|(i, p)| (i, p.point.haversine_distance(&sg.point)))
                .sorted_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
                .take(5)
                .collect::<Vec<_>>();

            new_sg.closest_stops = Some(
                bus_stops
                    .iter()
                    .map(|(i, _)| bus_stop_geos[*i].clone())
                    .collect(),
            );
            // dbg!(&new_sg);

            new_sg
        })
        .collect::<Vec<_>>();

    let stop_geos_file = "stop_geos.txt";
    let mut file = File::create(stop_geos_file).unwrap();
    for stop_geo in &stop_geos {
        writeln!(file, "{:#?}", stop_geo).unwrap();
    }
}
