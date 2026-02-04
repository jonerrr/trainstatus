use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(sqlx::Type, Copy, Clone, Hash, Eq, PartialEq, Debug, Serialize, Deserialize, ToSchema)]
#[sqlx(type_name = "source_enum", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum Source {
    MtaSubway,
    MtaBus,
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
            // Source::Lirr => "lirr",
            // Source::Mnr => "mnr",
            // Source::NjtRail => "njt_rail",
        }
    }
}
