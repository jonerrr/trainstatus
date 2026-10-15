use crate::alerts::store::Alerts;
use crate::alerts::types::ActivePeriod;
use crate::alerts::types::AlertCollection;
use crate::alerts::types::AlertFormat;
use crate::alerts::types::AlertSection;
use crate::alerts::types::AlertTranslation;
use crate::alerts::types::{AffectedEntity, AlertDetails, MtaAlert, MtaAlertData};
use crate::feed::Alert as GtfsAlert;
use crate::feed::FeedMessage;
use crate::source::Source;
use async_trait::async_trait;
use chrono::DateTime;
use tracing::{info, instrument, warn};

/// MTA uses a language suffix to distinguish HTML from plain translations.
pub(crate) fn mta_translations(alert: &GtfsAlert) -> Vec<AlertTranslation> {
    let mut translations = Vec::new();
    for (section, text) in [
        (AlertSection::Header, &alert.header_text),
        (AlertSection::Description, &alert.description_text),
    ] {
        if let Some(text) = text {
            for translation in &text.translation {
                let language = translation.language.as_deref().unwrap_or("en");
                let (format, language) = match language.strip_suffix("-html") {
                    Some(language) => (AlertFormat::Html, language),
                    None => (AlertFormat::Plain, language),
                };
                translations.push(AlertTranslation {
                    section,
                    format,
                    language: language.to_owned(),
                    text: translation.text.clone(),
                });
            }
        }
    }
    translations
}

pub(crate) fn active_periods(alert: &GtfsAlert) -> Vec<ActivePeriod> {
    alert
        .active_period
        .iter()
        .filter_map(|period| {
            let start = DateTime::from_timestamp(period.start? as i64, 0)?;
            let end = period
                .end
                .and_then(|end| DateTime::from_timestamp(end as i64, 0));
            Some(ActivePeriod {
                start_time: start,
                end_time: end,
            })
        })
        .collect()
}

/// Shared Mercury payload parsing; each source retains its route/stop ID policy.
pub(crate) fn mta_alert<T: GtfsAlertSource>(
    adapter: &T,
    entity_id: String,
    alert: GtfsAlert,
) -> Option<crate::alerts::types::MtaAlert> {
    let mercury = alert.mercury_alert.as_ref()?;
    let created_at = DateTime::from_timestamp(mercury.created_at as i64, 0)?;
    let updated_at = DateTime::from_timestamp(mercury.updated_at as i64, 0)?;
    Some(MtaAlert {
        data: MtaAlertData {
            display_before_active: mercury.display_before_active.unwrap_or(0) as i32,
            alert_type: mercury.alert_type.clone(),
            clone_id: mercury.clone_id.clone(),
        },
        details: AlertDetails {
            id: uuid::Uuid::now_v7(),
            original_id: entity_id,
            created_at,
            updated_at,
            recorded_at: chrono::Utc::now(),
            translations: mta_translations(&alert),
            active_periods: active_periods(&alert),
            entities: alert
                .informed_entity
                .iter()
                .map(|entity| AffectedEntity {
                    route_id: entity
                        .route_id
                        .clone()
                        .and_then(|id| adapter.parse_route_id(id)),
                    stop_id: entity
                        .stop_id
                        .clone()
                        .and_then(|id| adapter.parse_stop_id(id)),
                    sort_order: entity
                        .mercury_entity_selector
                        .as_ref()
                        .and_then(|selector| {
                            selector.sort_order.split(':').next_back()?.parse().ok()
                        })
                        .unwrap_or(0),
                })
                .collect(),
        },
    })
}

#[cfg(test)]
#[path = "tests/gtfs.rs"]
mod tests;

#[async_trait]
pub trait GtfsAlertSource: Send + Sync {
    type Alert: Send;
    fn source(&self) -> Source;
    fn collection(&self, alerts: Vec<Self::Alert>) -> AlertCollection;

    /// Fetch and decode all GTFS-RT alert feeds for this source.
    async fn fetch_feeds(&self) -> anyhow::Result<Vec<FeedMessage>>;

    /// Process a GTFS-RT alert into our internal Alert representation.
    /// Returns None if the alert should be skipped (e.g., missing mercury extension).
    /// The entity_id is the original entity ID from the feed.
    fn process_alert(&self, entity_id: String, alert: GtfsAlert) -> Option<Self::Alert>;

    /// Parse route_id for this source (e.g., convert "SS" to "SI" for subway)
    fn parse_route_id(&self, route_id: String) -> Option<String> {
        Some(route_id)
    }

    /// Parse stop_id for this source
    fn parse_stop_id(&self, stop_id: String) -> Option<String> {
        Some(stop_id)
    }
}

/// The main pipeline: Fetch -> Process -> Save
#[instrument(level = "debug", skip(adapter, alerts), fields(source = %adapter.source()))]
pub async fn run_pipeline<T: GtfsAlertSource>(adapter: &T, alerts: &Alerts) -> anyhow::Result<()> {
    let feeds = adapter.fetch_feeds().await?;
    if feeds.is_empty() {
        warn!(source = %adapter.source(), "No alert feeds fetched");
        return Ok(());
    }

    let mut processed = Vec::new();

    for feed in feeds {
        for entity in feed.entity {
            let Some(alert) = entity.alert else {
                continue;
            };

            // Process the alert using the adapter
            let Some(parsed_alert) = adapter.process_alert(entity.id, alert) else {
                continue;
            };

            processed.push(parsed_alert);
        }
    }

    info!(source = %adapter.source(), count = processed.len(), "Processed alerts");
    alerts.save_all(adapter.collection(processed)).await
}
