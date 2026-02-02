use crate::feed::Alert as GtfsAlert;
use crate::integrations::gtfs_alert::{self, GtfsAlertSource};
use crate::models::alert::{
    ActivePeriod, AffectedEntity, Alert, AlertData, AlertFormat, AlertSection, AlertTranslation,
    MtaData,
};
use crate::models::source::Source;
use crate::sources::AlertsAdapter;
use crate::stores::alert::AlertStore;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use uuid::Uuid;

// TODO: reuse parse_mta_language from mta_bus/alert.rs
/// Parses MTA's language field (e.g., "en", "en-html") into format and language.
/// "en" -> (Plain, "en")
/// "en-html" -> (Html, "en")
fn parse_mta_language(lang: Option<&str>) -> (AlertFormat, String) {
    match lang {
        Some(l) if l.ends_with("-html") => {
            let language = l.strip_suffix("-html").unwrap_or("en").to_string();
            (AlertFormat::Html, language)
        }
        Some(l) => (AlertFormat::Plain, l.to_string()),
        None => (AlertFormat::Plain, "en".to_string()),
    }
}

pub struct MtaSubwayAlerts;

impl GtfsAlertSource for MtaSubwayAlerts {
    fn source(&self) -> Source {
        Source::MtaSubway
    }

    fn feed_urls(&self) -> Vec<String> {
        vec!["https://api-endpoint.mta.info/Dataservice/mtagtfsfeeds/camsys%2Fsubway-alerts".into()]
    }

    fn parse_route_id(&self, route_id: String) -> Option<String> {
        // Convert SIR express ("SS") to "SI"
        if route_id == "SS" {
            Some("SI".to_string())
        } else {
            Some(route_id)
        }
    }

    fn parse_stop_id(&self, stop_id: String) -> Option<String> {
        // Subway stop IDs - remove trailing N/S direction indicator
        let mut stop_id = stop_id;
        if stop_id.ends_with('N') || stop_id.ends_with('S') {
            stop_id.pop();
        }

        // Filter out fake stop IDs
        if crate::models::stop::FAKE_STOP_IDS.contains(&stop_id.as_str()) {
            return None;
        }

        Some(stop_id)
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
        // Get the Mercury extension - skip if not present (likely elevator outage)
        let mercury_alert = alert.mercury_alert.as_ref()?;

        // Parse timestamps
        let created_at = DateTime::from_timestamp(mercury_alert.created_at as i64, 0)?;
        let updated_at = DateTime::from_timestamp(mercury_alert.updated_at as i64, 0)?;

        let alert_id = Uuid::now_v7();

        // Build MTA-specific data
        let mta_data = MtaData {
            display_before_active: mercury_alert.display_before_active.unwrap_or(0) as i32,
            alert_type: mercury_alert.alert_type.clone(),
            clone_id: mercury_alert.clone_id.clone(),
        };

        let parsed_alert = Alert {
            id: alert_id,
            original_id: entity_id,
            source: Source::MtaSubway,
            created_at,
            updated_at,
            recorded_at: Utc::now(),
            data: AlertData::MtaSubway(mta_data),
        };

        // Parse translations (header and description)
        let mut translations = Vec::new();

        if let Some(header_text) = &alert.header_text {
            for translation in header_text.translation.iter() {
                let (format, language) = parse_mta_language(translation.language.as_deref());
                translations.push(AlertTranslation {
                    alert_id,
                    section: AlertSection::Header,
                    format,
                    language,
                    text: translation.text.clone(),
                });
            }
        }

        if let Some(description_text) = &alert.description_text {
            for translation in description_text.translation.iter() {
                let (format, language) = parse_mta_language(translation.language.as_deref());
                translations.push(AlertTranslation {
                    alert_id,
                    section: AlertSection::Description,
                    format,
                    language,
                    text: translation.text.clone(),
                });
            }
        }

        // Parse active periods
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

        // Parse affected entities
        let affected_entities: Vec<AffectedEntity> = alert
            .informed_entity
            .iter()
            .filter_map(|entity| {
                let route_id = entity.route_id.clone().and_then(|r| self.parse_route_id(r));
                let stop_id = entity.stop_id.clone().and_then(|s| self.parse_stop_id(s));

                let sort_order = entity
                    .mercury_entity_selector
                    .as_ref()
                    .and_then(|selector| {
                        selector
                            .sort_order
                            .split(':')
                            .next_back()
                            .and_then(|s| s.parse().ok())
                    })
                    .unwrap_or(0);

                Some(AffectedEntity {
                    alert_id,
                    route_id,
                    source: Source::MtaSubway,
                    stop_id,
                    sort_order,
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
impl AlertsAdapter for MtaSubwayAlerts {
    fn source(&self) -> Source {
        Source::MtaSubway
    }

    fn refresh_interval(&self) -> std::time::Duration {
        std::time::Duration::from_secs(60)
    }

    async fn run(&self, alert_store: &AlertStore) -> anyhow::Result<()> {
        gtfs_alert::run_pipeline(self, alert_store).await
    }
}
