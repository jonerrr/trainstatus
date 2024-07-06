use geo_types::Point;

#[derive(Debug, Clone)]
pub struct StopGeometry<T, S> {
    pub id: T,
    pub name: String,
    pub point: Point<f64>,
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
