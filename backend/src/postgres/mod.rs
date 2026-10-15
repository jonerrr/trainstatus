//! Shared scalar conversion used only at private PostgreSQL boundaries.

mod geometry;

use crate::source::Source;
use serde::{Serialize, de::DeserializeOwned};

pub(crate) fn attributes(value: &impl Serialize) -> anyhow::Result<serde_json::Value> {
    let mut value = serde_json::to_value(value)?;
    if let Some(fields) = value.as_object_mut() {
        fields.remove("source");
    }
    Ok(value)
}
pub(crate) fn decode<T: DeserializeOwned>(
    source: Source,
    mut value: serde_json::Value,
) -> anyhow::Result<T> {
    value
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("Attributes must be an object"))?
        .insert(
            "source".into(),
            serde_json::Value::String(source.as_str().into()),
        );
    Ok(serde_json::from_value(value)?)
}
