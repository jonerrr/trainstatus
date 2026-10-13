use crate::alerts::gtfs as gtfs_alert;
use crate::alerts::gtfs::GtfsAlertSource;
use crate::alerts::store::Alerts;
use crate::alerts::types::AffectedEntity;
use crate::alerts::types::MtaAlert;
use crate::alerts::types::MtaAlertData;
use crate::alerts::types::{AlertCollection, AlertDetails};
use crate::feed::Alert as GtfsAlert;
use crate::feed::FeedMessage;
use crate::integrations::gtfs_realtime;
use crate::source::Source;
use crate::sources::AlertsAdapter;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
#[cfg(feature = "fixture-capture")]
use std::collections::BTreeMap;
use uuid::Uuid;

pub struct MtaBusAlerts;

#[cfg(feature = "fixture-capture")]
pub async fn capture_fixtures() -> anyhow::Result<BTreeMap<String, Vec<u8>>> {
    let bytes =
        reqwest::get("https://api-endpoint.mta.info/Dataservice/mtagtfsfeeds/camsys%2Fbus-alerts")
            .await?
            .error_for_status()?
            .bytes()
            .await?;

    let mut fixtures = BTreeMap::new();
    fixtures.insert("alerts.pb".to_string(), bytes.to_vec());
    Ok(fixtures)
}

#[async_trait]
impl GtfsAlertSource for MtaBusAlerts {
    type Alert = MtaAlert;
    fn source(&self) -> Source {
        Source::MtaBus
    }
    fn collection(&self, alerts: Vec<Self::Alert>) -> AlertCollection {
        AlertCollection::MtaBus(alerts)
    }

    async fn fetch_feeds(&self) -> anyhow::Result<Vec<FeedMessage>> {
        gtfs_realtime::fetch_feeds(vec![(
            "mta_bus_alerts".into(),
            gtfs_realtime::get_bytes(
                "https://api-endpoint.mta.info/Dataservice/mtagtfsfeeds/camsys%2Fbus-alerts",
            ),
        )])
        .await
    }

    fn parse_route_id(&self, route_id: String) -> Option<String> {
        // X80 causes foreign key constraint errors
        // See: https://groups.google.com/g/mtadeveloperresources/c/ZFnhNRrTlr8/m/K8oNzFrBAgAJ
        if route_id == "X80" {
            None
        } else {
            Some(route_id)
        }
    }

    fn parse_stop_id(&self, stop_id: String) -> Option<String> {
        // Bus stops use their ID as-is
        Some(stop_id)
    }

    fn process_alert(&self, entity_id: String, alert: GtfsAlert) -> Option<Self::Alert> {
        // Get the Mercury extension - skip if not present (likely elevator outage)
        let mercury_alert = alert.mercury_alert.as_ref()?;

        // Parse timestamps
        let created_at = DateTime::from_timestamp(mercury_alert.created_at as i64, 0)?;
        let updated_at = DateTime::from_timestamp(mercury_alert.updated_at as i64, 0)?;

        let alert_id = Uuid::now_v7();

        // Build MTA-specific data
        let mta_data = MtaAlertData {
            display_before_active: mercury_alert.display_before_active.unwrap_or(0) as i32,
            alert_type: mercury_alert.alert_type.clone(),
            clone_id: mercury_alert.clone_id.clone(),
        };

        let translations = gtfs_alert::mta_translations(&alert);
        let active_periods = gtfs_alert::active_periods(&alert);

        // Parse affected entities
        let affected_entities: Vec<AffectedEntity> = alert
            .informed_entity
            .iter()
            .map(|entity| {
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

                AffectedEntity {
                    route_id,
                    stop_id,
                    sort_order,
                }
            })
            .collect();

        Some(MtaAlert {
            data: mta_data,
            details: AlertDetails {
                id: alert_id,
                original_id: entity_id,
                created_at,
                updated_at,
                recorded_at: Utc::now(),
                translations,
                active_periods,
                entities: affected_entities,
            },
        })
    }
}

#[async_trait]
impl AlertsAdapter for MtaBusAlerts {
    fn source(&self) -> Source {
        Source::MtaBus
    }

    fn refresh_interval(&self) -> std::time::Duration {
        std::time::Duration::from_secs(60)
    }

    async fn run(&self, alerts: &Alerts) -> anyhow::Result<()> {
        gtfs_alert::run_pipeline(self, alerts).await
    }
}
