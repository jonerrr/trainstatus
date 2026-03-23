use async_trait::async_trait;
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::{
    feed::{Alert as GtfsAlert, FeedMessage},
    integrations::{
        gtfs_alert::{self, GtfsAlertSource},
        gtfs_realtime,
    },
    models::{
        alert::{
            ActivePeriod, AffectedEntity, Alert, AlertData, AlertFormat, AlertSection,
            AlertTranslation,
        },
        source::Source,
    },
    sources::AlertsAdapter,
    stores::alert::AlertStore,
};

use super::{NJT_ALERTS_URL, NjtApi, get_token, njt_post_future};

pub struct NjtBusAlerts;

#[async_trait]
impl GtfsAlertSource for NjtBusAlerts {
    fn source(&self) -> Source {
        Source::NjtBus
    }

    async fn fetch_feeds(&self) -> Vec<FeedMessage> {
        let token = match get_token(NjtApi::GtfsG2).await {
            Ok(t) => t,
            Err(e) => {
                tracing::error!("NJT auth failed for alerts: {:?}", e);
                return vec![];
            }
        };

        gtfs_realtime::fetch_feeds(vec![(
            "njt_bus_alerts".into(),
            njt_post_future(NJT_ALERTS_URL, token),
        )])
        .await
    }

    fn process_alert(
        &self,
        entity_id: String,
        alert: GtfsAlert,
    ) -> Option<(
        Alert,
        Vec<AlertTranslation>,
        Vec<ActivePeriod>,
        Vec<AffectedEntity>,
    )> {
        // Derive a stable created_at from the first active period start time so repeated
        // runs upsert to the same row on (created_at, original_id, source).
        let first_period_start = alert
            .active_period
            .first()
            .and_then(|ap| ap.start)
            .and_then(|s| DateTime::from_timestamp(s as i64, 0));

        let created_at = first_period_start?;

        let alert_id = Uuid::now_v7();

        let parsed_alert = Alert {
            id: alert_id,
            original_id: entity_id,
            source: Source::NjtBus,
            created_at,
            updated_at: Utc::now(),
            recorded_at: Utc::now(),
            data: AlertData::NjtBus,
        };

        let mut translations = Vec::new();

        // headers in NJT alerts are just the route id
        // if let Some(header_text) = &alert.header_text {
        //     for translation in &header_text.translation {
        //         translations.push(AlertTranslation {
        //             alert_id,
        //             section: AlertSection::Header,
        //             format: AlertFormat::Html,
        //             language: translation
        //                 .language
        //                 .clone()
        //                 .unwrap_or_else(|| "en".to_string()),
        //             text: translation.text.clone(),
        //         });
        //     }
        // }

        if let Some(description_text) = &alert.description_text {
            for translation in &description_text.translation {
                translations.push(AlertTranslation {
                    alert_id,
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
        // TODO: refactor api responses to support no header
        if translations.is_empty() {
            tracing::warn!("NJT Bus alert has no description text translations, skipping",);
            return None;
        }

        let active_periods: Vec<ActivePeriod> = alert
            .active_period
            .iter()
            .filter_map(|ap| {
                let start = DateTime::from_timestamp(ap.start? as i64, 0)?;
                let end = ap.end.and_then(|e| DateTime::from_timestamp(e as i64, 0));
                Some(ActivePeriod {
                    alert_id,
                    start_time: start,
                    end_time: end,
                })
            })
            .collect();

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
                    alert_id,
                    route_id,
                    source: Source::NjtBus,
                    stop_id,
                    // TODO: maybe set to something other than 0
                    sort_order: 0,
                })
            })
            .collect();

        Some((
            parsed_alert,
            translations,
            active_periods,
            affected_entities,
        ))
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

    async fn run(&self, alert_store: &AlertStore) -> anyhow::Result<()> {
        gtfs_alert::run_pipeline(self, alert_store).await
    }
}
