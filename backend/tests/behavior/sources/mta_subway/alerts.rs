use crate::support::alerts::fixture_alert_where;
use backend::alerts::gtfs::GtfsAlertSource;
use backend::alerts::types::AlertFormat;
use backend::alerts::types::AlertSection;
use backend::source::Source;
use backend::sources::mta_subway::alerts::MtaSubwayAlerts;

#[test]
fn mercury_alert_preserves_formats_periods_and_affected_routes() {
    let (entity_id, input) = fixture_alert_where(Source::MtaSubway, |alert| {
        alert.mercury_alert.is_some()
            && !alert.active_period.is_empty()
            && alert.header_text.is_some()
            && alert.description_text.is_some()
            && !alert.informed_entity.is_empty()
    });
    let alert = MtaSubwayAlerts
        .process_alert(entity_id.clone(), input)
        .unwrap()
        .details;
    let translations = &alert.translations;
    let periods = &alert.active_periods;
    let entities = &alert.entities;
    assert!(!periods.is_empty());
    assert!(!entities.is_empty());
    assert_eq!(alert.original_id, entity_id);
    assert!(alert.updated_at >= alert.created_at);
    assert!(periods[0].start_time <= periods[0].end_time.unwrap_or(periods[0].start_time));
    assert!(
        translations
            .iter()
            .any(|t| t.section == AlertSection::Header
                && t.format == AlertFormat::Plain
                && t.language == "en")
    );
    assert!(
        translations
            .iter()
            .any(|t| t.format == AlertFormat::Html && t.language == "en")
    );
    assert!(
        entities
            .iter()
            .any(|e| e.route_id.is_some() || e.stop_id.is_some())
    );
}

#[test]
fn non_mercury_alert_is_skipped() {
    let (_, mut input) =
        fixture_alert_where(Source::MtaSubway, |alert| alert.mercury_alert.is_some());
    input.mercury_alert = None;
    assert!(
        MtaSubwayAlerts
            .process_alert("elevator".into(), input)
            .is_none()
    );
}
