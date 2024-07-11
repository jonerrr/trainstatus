pub trait Trip {
    fn trip_id(&self) -> &str;
    fn route_id(&self) -> &str;
    fn direction(&self) -> i16;
    fn start_date(&self) -> &str;
}
