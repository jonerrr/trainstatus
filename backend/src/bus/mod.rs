use std::{env::var, sync::OnceLock};

pub mod positions;
pub mod static_data;
pub mod trips;

// https://stackoverflow.com/a/77249700
pub fn api_key() -> &'static str {
    // you need bustime api key to run this
    static API_KEY: OnceLock<String> = OnceLock::new();
    API_KEY.get_or_init(|| var("API_KEY").unwrap())
}
