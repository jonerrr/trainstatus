//! Source-specific deterministic identifiers for persisted runs and alerts.

use chrono::{DateTime, Datelike, NaiveDate, Utc};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use utoipa::ToSchema;

use crate::source::Source;

/// Stable identifier for one scheduled run occurrence within a source.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug, ToSchema)]
#[schema(value_type = String)]
pub struct RunId([u8; 32]);

/// Immutable archive locator carried by a provider observation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RunRef {
    pub id: RunId,
    pub partition_date: NaiveDate,
}

impl From<&super::types::trip::TripRun<RunId>> for RunRef {
    fn from(trip: &super::types::trip::TripRun<RunId>) -> Self {
        Self {
            id: trip.id,
            partition_date: trip.partition_date,
        }
    }
}

/// Stable identifier for one provider alert occurrence within a source.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug, ToSchema)]
#[schema(value_type = String)]
pub struct AlertId([u8; 32]);

impl RunId {
    pub const ZERO: Self = Self([0u8; 32]);

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn to_hex(&self) -> String {
        encode_hex(&self.0)
    }

    /// GTFS scheduled bus runs: provider trip id and resolved GTFS service date only.
    pub fn scheduled_bus(source: Source, original_id: &str, service_date: NaiveDate) -> Self {
        debug_assert!(matches!(source, Source::MtaBus | Source::NjtBus));
        Self::digest_scheduled(source, original_id, service_date)
    }

    /// Scheduled subway occurrence from normalized train line, origin time, and origin code.
    pub fn mta_subway(original_trip_id: &str, service_date: NaiveDate) -> Option<Self> {
        let (line, time, half_minute, origin) = subway_occurrence(original_trip_id)?;
        let mut hash = blake3::Hasher::new();
        hash.update(b"mta_subway");
        for part in [line, time, origin] {
            hash.update(&(part.len() as u64).to_le_bytes());
            hash.update(part.as_bytes());
        }
        hash.update(&[u8::from(half_minute)]);
        hash.update(&service_date.num_days_from_ce().to_le_bytes());
        Some(Self(*hash.finalize().as_bytes()))
    }

    #[cfg(any(test, feature = "fixture-capture"))]
    pub fn arbitrary(label: &str) -> Self {
        Self(*blake3::hash(label.as_bytes()).as_bytes())
    }

    fn digest_scheduled(source: Source, identity: &str, service_date: NaiveDate) -> Self {
        let mut hash = blake3::Hasher::new();
        hash.update(source.as_str().as_bytes());
        hash.update(&(identity.len() as u64).to_le_bytes());
        hash.update(identity.as_bytes());
        hash.update(&service_date.num_days_from_ce().to_le_bytes());
        Self(*hash.finalize().as_bytes())
    }
}

impl AlertId {
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn to_hex(&self) -> String {
        encode_hex(&self.0)
    }

    pub fn new(source: Source, original_id: &str, created_at: DateTime<Utc>) -> Self {
        let mut hash = blake3::Hasher::new();
        hash.update(source.as_str().as_bytes());
        hash.update(&(original_id.len() as u64).to_le_bytes());
        hash.update(original_id.as_bytes());
        hash.update(&created_at.timestamp_micros().to_le_bytes());
        Self(*hash.finalize().as_bytes())
    }
}

/// Service calendar date in America/New_York for a provider run start timestamp.
pub fn eastern_service_date(run_start: DateTime<Utc>) -> NaiveDate {
    run_start
        .with_timezone(&chrono_tz::America::New_York)
        .date_naive()
}

/// Stable head-index key for a run's immutable partition date.
pub fn partition_key(date: NaiveDate) -> String {
    date.format("%Y-%m-%d").to_string()
}

/// The train-type character and destination are mutable operating instructions.
fn subway_occurrence(trip_id: &str) -> Option<(&str, &str, bool, &str)> {
    let mut parts = trip_id.split_whitespace();
    let train = parts.next()?;
    let (_, line) = train.split_at_checked(train.chars().next()?.len_utf8())?;
    if line.is_empty() {
        return None;
    }
    let token = parts.next()?;
    let time = token.get(..4)?;
    if !time.bytes().all(|byte| byte.is_ascii_digit())
        || time[..2].parse::<u8>().ok()? > 23
        || time[2..].parse::<u8>().ok()? > 59
    {
        return None;
    }
    let suffix = token.get(4..)?;
    let (half_minute, suffix) = match suffix.strip_prefix('+') {
        Some(rest) => (true, rest),
        None => (false, suffix),
    };
    let endpoints = if suffix.is_empty() {
        parts.next()?
    } else {
        suffix
    };
    let (origin, destination) = endpoints.split_once('/')?;
    if origin.is_empty() || destination.is_empty() || parts.next().is_some() {
        return None;
    }
    Some((line, time, half_minute, origin))
}

impl Serialize for RunId {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serialize_digest(&self.0, serializer)
    }
}

impl<'de> Deserialize<'de> for RunId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserialize_digest(deserializer).map(Self)
    }
}

impl Serialize for AlertId {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serialize_digest(&self.0, serializer)
    }
}

impl<'de> Deserialize<'de> for AlertId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserialize_digest(deserializer).map(Self)
    }
}

impl std::fmt::Display for RunId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(hex_str(&hex_ascii(&self.0)))
    }
}

impl std::fmt::Display for AlertId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(hex_str(&hex_ascii(&self.0)))
    }
}

fn hex_ascii(bytes: &[u8; 32]) -> [u8; 64] {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = [0; 64];
    for (index, byte) in bytes.iter().enumerate() {
        output[index * 2] = HEX[usize::from(byte >> 4)];
        output[index * 2 + 1] = HEX[usize::from(byte & 0xf)];
    }
    output
}

fn hex_str(bytes: &[u8; 64]) -> &str {
    std::str::from_utf8(bytes).expect("hexadecimal is ASCII")
}

fn encode_hex(bytes: &[u8; 32]) -> String {
    hex_str(&hex_ascii(bytes)).to_owned()
}

fn serialize_digest<S: Serializer>(bytes: &[u8; 32], serializer: S) -> Result<S::Ok, S::Error> {
    if serializer.is_human_readable() {
        serializer.serialize_str(hex_str(&hex_ascii(bytes)))
    } else {
        serializer.serialize_bytes(bytes)
    }
}

fn deserialize_digest<'de, D: Deserializer<'de>>(deserializer: D) -> Result<[u8; 32], D::Error> {
    struct DigestVisitor;
    impl serde::de::Visitor<'_> for DigestVisitor {
        type Value = [u8; 32];

        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("a 32-byte identifier or 64-character hexadecimal string")
        }

        fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<Self::Value, E> {
            decode_hex(value).map(|id| id.0).map_err(E::custom)
        }

        fn visit_bytes<E: serde::de::Error>(self, value: &[u8]) -> Result<Self::Value, E> {
            value
                .try_into()
                .map_err(|_| E::invalid_length(value.len(), &self))
        }
    }
    deserializer.deserialize_any(DigestVisitor)
}

fn decode_hex(hex: &str) -> Result<RunId, String> {
    if hex.len() != 64 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err("run id must be a 64-character hex string".into());
    }
    let mut out = [0u8; 32];
    for (index, chunk) in hex.as_bytes().chunks(2).enumerate() {
        let high = from_hex_digit(chunk[0])?;
        let low = from_hex_digit(chunk[1])?;
        out[index] = (high << 4) | low;
    }
    Ok(RunId(out))
}

fn from_hex_digit(byte: u8) -> Result<u8, String> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => Err("invalid hex digit".into()),
    }
}

#[cfg(test)]
#[path = "tests/identity.rs"]
mod tests;
