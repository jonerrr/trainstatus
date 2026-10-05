use crate::support::alerts::{assert_links, fixture_alert};
use backend::{
    integrations::gtfs_alert::GtfsAlertSource,
    models::{
        alert::{AlertFormat, AlertSection},
        source::Source,
    },
    sources::njt_bus::alerts::NjtBusAlerts,
};

#[test]
fn detour_alert_uses_period_identity_and_description_with_default_language() {
    let input = fixture_alert(Source::NjtBus, "41694");
    let (alert, translations, periods, entities) = NjtBusAlerts
        .process_alert("41694".into(), input.clone())
        .unwrap();
    assert_links(&alert, &translations, &periods, &entities);
    assert_eq!(alert.source, Source::NjtBus);
    assert_eq!(alert.original_id, "41694");
    assert_eq!(alert.created_at.timestamp(), 1_783_310_400);
    assert_eq!(
        NjtBusAlerts
            .process_alert("41694".into(), input)
            .unwrap()
            .0
            .created_at,
        alert.created_at
    );
    assert_eq!(periods[0].end_time.unwrap().timestamp(), 1_783_396_800);
    assert_eq!(translations.len(), 1);
    assert_eq!(translations[0].section, AlertSection::Description);
    assert_eq!(translations[0].format, AlertFormat::Plain);
    assert_eq!(translations[0].language, "en");
    assert!(translations[0].text.contains("Harrington Park"));
    assert!(
        entities
            .iter()
            .all(|e| e.route_id.as_deref() == Some("167"))
    );
}

#[test]
fn route_only_or_undated_alert_is_skipped() {
    let input = fixture_alert(Source::NjtBus, "41694");
    let mut route_only = input.clone();
    route_only.description_text = None;
    assert!(
        NjtBusAlerts
            .process_alert("route-only".into(), route_only)
            .is_none()
    );
    let mut undated = input;
    undated.active_period.clear();
    assert!(
        NjtBusAlerts
            .process_alert("undated".into(), undated)
            .is_none()
    );
}
