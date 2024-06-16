use serde::{Deserialize, Deserializer};

pub mod routes;
pub mod stops;
pub mod trips;

pub fn parse_list<'de, D>(deserializer: D) -> Result<Vec<i32>, D::Error>
where
    D: Deserializer<'de>,
{
    let str_sequence = String::deserialize(deserializer)?;

    Ok(str_sequence
        .split(',')
        .map(|item| item.parse::<i32>().map_err(serde::de::Error::custom))
        .collect::<Result<Vec<i32>, _>>()?)
}
