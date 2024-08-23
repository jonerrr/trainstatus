use sqlx::PgPool;
use std::io::Cursor;

pub mod route;
pub mod stop;

pub async fn import(pool: &PgPool) {
    let gtfs = reqwest::Client::new()
        .get("http://web.mta.info/developers/data/nyct/subway/google_transit.zip")
        .send()
        .await
        .unwrap()
        .bytes()
        .await
        .unwrap();

    let reader = Cursor::new(gtfs);
    let mut archive = zip::ZipArchive::new(reader).unwrap();

    // let transfers_file = archive.by_name("transfers.txt").unwrap();
    // let mut rdr = csv::Reader::from_reader(transfers_file);
    // let mut transfers = rdr
    //     .deserialize()
    //     .collect::<Result<Vec<Transfer>, csv::Error>>()
    //     .unwrap();

    let mut routes = route::Route::get_train(archive.by_name("routes.txt").unwrap()).await;
    let (mut stops, mut route_stops) = stop::Stop::get_train(
        routes.iter().map(|r| r.id.clone()).collect(),
        archive.by_name("transfers.txt").unwrap(),
    )
    .await;
    let (bus_routes, bus_stops, bus_route_stops) = route::Route::get_bus().await;
    routes.append(&mut bus_routes);
    stops.append(&mut bus_stops);
    route_stops.append(&mut bus_route_stops);

    route::Route::insert(routes, pool).await;
    stop::Stop::insert(stops, pool).await;
    stop::RouteStop::insert(route_stops, pool).await;

    // dbg!(train_stops.len(), train_route_stops.len());
}
