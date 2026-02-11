use crate::feed::Alert as GtfsAlert;
use crate::integrations::gtfs_realtime::fetch;
use crate::models::alert::{ActivePeriod, AffectedEntity, Alert, AlertTranslation};
use crate::models::source::Source;
use crate::stores::alert::AlertStore;
use tracing::{debug, info, instrument, warn};

pub trait GtfsAlertSource: Send + Sync {
    fn source(&self) -> Source;

    fn feed_urls(&self) -> Vec<String>;

    /// Process a GTFS-RT alert into our internal Alert representation.
    /// Returns None if the alert should be skipped (e.g., missing mercury extension).
    /// The entity_id is the original entity ID from the feed.
    fn process_alert(
        &self,
        entity_id: String,
        alert: GtfsAlert,
    ) -> Option<(
        Alert,
        Vec<AlertTranslation>,
        Vec<ActivePeriod>,
        Vec<AffectedEntity>,
    )>;

    /// Parse route_id for this source (e.g., convert "SS" to "SI" for subway)
    fn parse_route_id(&self, route_id: String) -> Option<String> {
        Some(route_id)
    }

    /// Parse stop_id for this source
    fn parse_stop_id(&self, stop_id: String) -> Option<String> {
        Some(stop_id)
    }
}

/// Result of processing alerts from feeds
#[derive(Default)]
pub struct ProcessedAlerts {
    pub alerts: Vec<Alert>,
    pub translations: Vec<AlertTranslation>,
    pub active_periods: Vec<ActivePeriod>,
    pub affected_entities: Vec<AffectedEntity>,
    /// Alert IDs that are in the current feed
    pub in_feed_ids: Vec<uuid::Uuid>,
    /// MTA IDs of alerts that were cloned (used to remove old versions)
    pub cloned_mta_ids: Vec<String>,
}

/// The main pipeline: Fetch -> Process -> Save
#[instrument(skip(adapter, alert_store), fields(source = ?adapter.source()))]
pub async fn run_pipeline<T: GtfsAlertSource>(
    adapter: &T,
    alert_store: &AlertStore,
) -> anyhow::Result<()> {
    let feeds = fetch(adapter.feed_urls()).await;
    if feeds.is_empty() {
        warn!("No alert feeds fetched for {:?}", adapter.source());
        return Ok(());
    }

    let mut processed = ProcessedAlerts::default();
    let mut seen_alerts = std::collections::HashSet::new();

    for feed in feeds {
        for entity in feed.entity {
            let Some(alert) = entity.alert else {
                continue;
            };

            // Process the alert using the adapter
            let Some((parsed_alert, translations, periods, entities)) =
                adapter.process_alert(entity.id, alert)
            else {
                continue;
            };

            // Check for duplicates within the feed (same created_at and original_id)
            let dedupe_key = (parsed_alert.created_at, parsed_alert.original_id.clone());
            if seen_alerts.contains(&dedupe_key) {
                debug!(
                    "Skipping duplicate alert in feed: {}",
                    parsed_alert.original_id
                );
                continue;
            }
            seen_alerts.insert(dedupe_key);

            // Track clone IDs for removing old versions
            let clone_id = match &parsed_alert.data {
                crate::models::alert::AlertData::MtaSubway(data)
                | crate::models::alert::AlertData::MtaBus(data) => data.clone_id.clone(),
            };
            if let Some(clone_id) = clone_id {
                processed.cloned_mta_ids.push(clone_id);
            }

            processed.in_feed_ids.push(parsed_alert.id);
            processed.alerts.push(parsed_alert);
            processed.translations.extend(translations);
            processed.active_periods.extend(periods);
            processed.affected_entities.extend(entities);
        }
    }

    // Remove alerts that were cloned (their old versions)
    let cloned_ids: Vec<uuid::Uuid> = processed
        .alerts
        .iter()
        .filter(|a| processed.cloned_mta_ids.contains(&a.original_id))
        .map(|a| a.id)
        .collect();

    processed.alerts.retain(|a| !cloned_ids.contains(&a.id));
    processed
        .translations
        .retain(|t| !cloned_ids.contains(&t.alert_id));
    processed
        .active_periods
        .retain(|p| !cloned_ids.contains(&p.alert_id));
    processed
        .affected_entities
        .retain(|e| !cloned_ids.contains(&e.alert_id));

    info!(
        "Processed {} alerts, {} translations, {} active_periods, {} affected_entities for {:?}",
        processed.alerts.len(),
        processed.translations.len(),
        processed.active_periods.len(),
        processed.affected_entities.len(),
        adapter.source()
    );

    // Save to database
    alert_store
        .save_all(
            adapter.source(),
            &processed.alerts,
            &processed.translations,
            &processed.active_periods,
            &processed.affected_entities,
            &processed.cloned_mta_ids,
        )
        .await?;

    Ok(())
}
