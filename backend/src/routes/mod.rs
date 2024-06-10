use serde::{Deserialize, Deserializer};

pub mod alerts;
pub mod bus;
pub mod errors;
pub mod stops;
pub mod trips;

pub fn parse_list<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let str_sequence = String::deserialize(deserializer)?;

    Ok(str_sequence
        .split(',')
        .map(|item| item.to_owned())
        .collect())
}
