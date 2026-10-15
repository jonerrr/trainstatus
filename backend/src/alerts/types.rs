use crate::source::Source;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

/// Provider-independent alert content, including every owned collection.
#[derive(Debug, Clone)]
pub struct AlertDetails {
    pub id: Uuid,
    pub original_id: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub recorded_at: DateTime<Utc>,
    pub translations: Vec<AlertTranslation>,
    pub active_periods: Vec<ActivePeriod>,
    pub entities: Vec<AffectedEntity>,
}

#[derive(Debug, Clone)]
pub struct MtaAlert {
    pub details: AlertDetails,
    pub data: MtaAlertData,
}

/// A source-discriminated collection ties every payload to its source, even when
/// empty. Both MTA sources share a payload; NJT has no MTA attributes.
///
/// ```compile_fail
/// use backend::alerts::types::{AlertCollection, MtaAlert};
/// fn wrong_source(alert: MtaAlert) {
///     let collection = AlertCollection::NjtBus(vec![alert]);
/// }
/// ```
#[derive(Debug, Clone)]
pub enum AlertCollection {
    MtaSubway(Vec<MtaAlert>),
    MtaBus(Vec<MtaAlert>),
    NjtBus(Vec<AlertDetails>),
}

/// An owner read pairs the common content with its source-discriminated payload.
#[derive(Debug, Clone)]
pub struct Alert {
    pub details: AlertDetails,
    pub data: AlertData,
}

impl std::ops::Deref for Alert {
    type Target = AlertDetails;
    fn deref(&self) -> &AlertDetails {
        &self.details
    }
}

impl AlertCollection {
    pub(crate) fn into_alerts(self) -> (Source, Vec<Alert>) {
        match self {
            Self::MtaSubway(alerts) => (
                Source::MtaSubway,
                alerts
                    .into_iter()
                    .map(|a| Alert {
                        details: a.details,
                        data: AlertData::MtaSubway(a.data),
                    })
                    .collect(),
            ),
            Self::MtaBus(alerts) => (
                Source::MtaBus,
                alerts
                    .into_iter()
                    .map(|a| Alert {
                        details: a.details,
                        data: AlertData::MtaBus(a.data),
                    })
                    .collect(),
            ),
            Self::NjtBus(alerts) => (
                Source::NjtBus,
                alerts
                    .into_iter()
                    .map(|a| Alert {
                        details: a,
                        data: AlertData::NjtBus,
                    })
                    .collect(),
            ),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum AlertSection {
    Header,
    Description,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum AlertFormat {
    Plain,
    Html,
}

/// Alert translations for different content types
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AlertTranslation {
    pub section: AlertSection,
    pub format: AlertFormat,
    pub language: String,
    pub text: String,
}

// used for mta subway and bus
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct MtaAlertData {
    pub display_before_active: i32,
    pub alert_type: String,
    /// The id of the planned work this alert was cloned from
    pub clone_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
#[serde(tag = "source", rename_all = "snake_case")]
pub enum AlertData {
    MtaSubway(MtaAlertData),
    MtaBus(MtaAlertData),
    NjtBus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivePeriod {
    pub start_time: DateTime<Utc>,
    pub end_time: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AffectedEntity {
    pub route_id: Option<String>,
    pub stop_id: Option<String>,
    pub sort_order: i32,
}
