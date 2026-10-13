use crate::support::alerts::fixture_alert_where;
use backend::alerts::gtfs::GtfsAlertSource;
use backend::alerts::types::AlertFormat;
use backend::alerts::types::AlertSection;
use backend::source::Source;
use backend::sources::njt_bus::alerts::NjtBusAlerts;

#[test]
fn detour_alert_uses_period_identity_and_description_with_default_language() {
    let (entity_id, input) = fixture_alert_where(Source::NjtBus, |alert| {
        alert.description_text.is_some()
            && !alert.active_period.is_empty()
            && !alert.informed_entity.is_empty()
    });
    let alert = NjtBusAlerts
        .process_alert(entity_id.clone(), input.clone())
        .unwrap();
    let translations = &alert.translations;
    let periods = &alert.active_periods;
    let entities = &alert.entities;
    assert!(!periods.is_empty());
    assert!(!entities.is_empty());
    assert_eq!(alert.original_id, entity_id.clone());
    assert_eq!(
        NjtBusAlerts
            .process_alert(entity_id, input)
            .unwrap()
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
