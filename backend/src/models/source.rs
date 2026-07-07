use serde::{Deserialize, Serialize};
use std::{fmt, str::FromStr};
use utoipa::ToSchema;

#[derive(sqlx::Type, Copy, Clone, Hash, Eq, PartialEq, Debug, Serialize, Deserialize, ToSchema)]
#[sqlx(type_name = "source_enum", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum Source {
    MtaSubway,
    MtaBus,
    NjtBus,
    // below not implemented yet
    // Lirr,
    // Mnr,
    // NjtRail,
}

impl Source {
    pub fn as_str(&self) -> &'static str {
        match self {
            Source::MtaSubway => "mta_subway",
            Source::MtaBus => "mta_bus",
            Source::NjtBus => "njt_bus",
            // Source::Lirr => "lirr",
            // Source::Mnr => "mnr",
            // Source::NjtRail => "njt_rail",
        }
    }
}

impl fmt::Display for Source {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Source {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "mta_subway" => Ok(Self::MtaSubway),
            "mta_bus" => Ok(Self::MtaBus),
            "njt_bus" => Ok(Self::NjtBus),
            other => anyhow::bail!("unknown source: {other}"),
        }
    }
}
