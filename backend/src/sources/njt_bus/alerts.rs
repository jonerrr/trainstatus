use super::{NJT_ALERTS_URL, get_token, njt_post_future};
use crate::alerts::{
    gtfs::{GtfsAlertSource, active_periods, run_pipeline},
    store::Alerts,
    types::{
        AffectedEntity, AlertCollection, AlertDetails, AlertFormat, AlertSection, AlertTranslation,
    },
};
use crate::feed::{Alert, FeedMessage};
use crate::integrations::gtfs_realtime;
use crate::source::Source;
use crate::sources::AlertsAdapter;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
#[cfg(feature = "fixture-capture")]
use std::collections::BTreeMap;
use uuid::Uuid;

pub struct NjtBusAlerts;

#[cfg(feature = "fixture-capture")]
pub async fn capture_fixtures() -> anyhow::Result<BTreeMap<String, Vec<u8>>> {
    let token = get_token().await?;
    let bytes = njt_post_future(NJT_ALERTS_URL, token).await?;

    let mut fixtures = BTreeMap::new();
    fixtures.insert("alerts.pb".to_string(), bytes.to_vec());
    Ok(fixtures)
}

#[async_trait]
impl GtfsAlertSource for NjtBusAlerts {
    type Alert = AlertDetails;
    fn source(&self) -> Source {
        Source::NjtBus
    }
    fn collection(&self, alerts: Vec<Self::Alert>) -> AlertCollection {
        AlertCollection::NjtBus(alerts)
    }

    async fn fetch_feeds(&self) -> anyhow::Result<Vec<FeedMessage>> {
        let token = get_token().await?;

        gtfs_realtime::fetch_feeds(vec![(
            "njt_bus_alerts".into(),
            njt_post_future(NJT_ALERTS_URL, token),
        )])
        .await
    }

    fn process_alert(&self, entity_id: String, alert: Alert) -> Option<Self::Alert> {
        // Derive a stable created_at from the first active period start time so repeated
        // runs upsert to the same row on (created_at, original_id, source).
        let first_period_start = alert
            .active_period
            .first()
            .and_then(|ap| ap.start)
            .and_then(|s| DateTime::from_timestamp(s as i64, 0));

        let created_at = first_period_start?;

        let alert_id = Uuid::now_v7();

        let mut translations = Vec::new();

        // NJT headers contain only the route ID; display the description.
        if let Some(description_text) = &alert.description_text {
            for translation in &description_text.translation {
                translations.push(AlertTranslation {
                    section: AlertSection::Description,
                    format: AlertFormat::Plain,
                    // seems to always be None, but default to "en" just in case
                    language: translation
                        .language
                        .clone()
                        .unwrap_or_else(|| "en".to_string()),
                    text: translation.text.clone(),
                });
            }
        }

        // Only proceed if we have some text to show
        if translations.is_empty() {
            tracing::warn!(
                trainstatus.source = Source::NjtBus.as_str(),
                "NJT Bus alert has no description text translations, skipping"
            );
            return None;
        }

        let active_periods = active_periods(&alert);

        if active_periods.is_empty() {
            return None;
        }

        // TODO: maybe also handle agency_id. from what i've seen its only "NJB"
        let affected_entities: Vec<AffectedEntity> = alert
            .informed_entity
            .iter()
            .filter_map(|entity| {
                let route_id = entity.route_id.clone();
                let stop_id = entity.stop_id.clone();

                // Skip entities with neither route nor stop
                if route_id.is_none() && stop_id.is_none() {
                    return None;
                }

                Some(AffectedEntity {
                    route_id,
                    stop_id,
                    // TODO: maybe set to something other than 0
                    sort_order: 0,
                })
            })
            .collect();

        Some(AlertDetails {
            id: alert_id,
            original_id: entity_id,
            created_at,
            updated_at: Utc::now(),
            recorded_at: Utc::now(),
            translations,
            active_periods,
            entities: affected_entities,
        })
    }
}

#[async_trait]
impl AlertsAdapter for NjtBusAlerts {
    fn source(&self) -> Source {
        Source::NjtBus
    }

    fn refresh_interval(&self) -> std::time::Duration {
        std::time::Duration::from_secs(60)
    }

    async fn run(&self, alerts: &Alerts) -> anyhow::Result<()> {
        run_pipeline(self, alerts).await
    }
}
