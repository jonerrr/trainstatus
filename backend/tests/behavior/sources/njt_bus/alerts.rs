use crate::support::alerts::{assert_links, fixture_alert_where};
use backend::{
    alerts::gtfs::GtfsAlertSource,
    models::{
        alert::{AlertFormat, AlertSection},
        source::Source,
    },
    sources::njt_bus::alerts::NjtBusAlerts,
};

#[test]
fn detour_alert_uses_period_identity_and_description_with_default_language() {
    let (entity_id, input) = fixture_alert_where(Source::NjtBus, |alert| {
        alert.description_text.is_some()
            && !alert.active_period.is_empty()
            && !alert.informed_entity.is_empty()
    });
    let (alert, translations, periods, entities) = NjtBusAlerts
        .process_alert(entity_id.clone(), input.clone())
        .unwrap();
    assert_links(&alert, &translations, &periods, &entities);
    assert_eq!(alert.source, Source::NjtBus);
    assert_eq!(alert.original_id, entity_id.clone());
    assert_eq!(
        NjtBusAlerts
            .process_alert(entity_id, input)
            .unwrap()
            .0
            .created_at,
        alert.created_at
    );
    assert_eq!(translations.len(), 1);
    assert_eq!(translations[0].section, AlertSection::Description);
    assert_eq!(translations[0].format, AlertFormat::Plain);
    assert_eq!(translations[0].language, "en");
    assert!(!translations[0].text.is_empty());
    assert!(
        entities
            .iter()
            .any(|e| e.route_id.is_some() || e.stop_id.is_some())
    );
}

#[test]
fn route_only_or_undated_alert_is_skipped() {
    let (_, input) = fixture_alert_where(Source::NjtBus, |alert| {
        alert.description_text.is_some() && !alert.active_period.is_empty()
    });
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
