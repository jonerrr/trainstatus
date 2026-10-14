use crate::alerts::gtfs as gtfs_alert;
use crate::alerts::gtfs::GtfsAlertSource;
use crate::alerts::store::Alerts;
use crate::alerts::types::AlertCollection;
use crate::alerts::types::MtaAlert;
use crate::feed::Alert as GtfsAlert;
use crate::feed::FeedMessage;
use crate::integrations::gtfs_realtime;
use crate::source::Source;
use crate::sources::AlertsAdapter;
use async_trait::async_trait;
#[cfg(feature = "fixture-capture")]
use std::collections::BTreeMap;

pub struct MtaBusAlerts;

#[cfg(feature = "fixture-capture")]
pub async fn capture_fixtures() -> anyhow::Result<BTreeMap<String, Vec<u8>>> {
    let bytes = crate::integrations::http::realtime()
        .get("https://api-endpoint.mta.info/Dataservice/mtagtfsfeeds/camsys%2Fbus-alerts")
        .send()
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
        gtfs_alert::mta_alert(self, entity_id, alert)
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
