use crate::support::alerts::{assert_links, fixture_alert_where};
use backend::{
    integrations::gtfs_alert::GtfsAlertSource,
    models::{
        alert::{AlertFormat, AlertSection},
        source::Source,
    },
    sources::mta_subway::alerts::MtaSubwayAlerts,
};

#[test]
fn mercury_alert_preserves_formats_periods_and_affected_routes() {
    let (entity_id, input) = fixture_alert_where(Source::MtaSubway, |alert| {
        alert.mercury_alert.is_some()
            && !alert.active_period.is_empty()
            && alert.header_text.is_some()
            && alert.description_text.is_some()
            && !alert.informed_entity.is_empty()
    });
    let (alert, translations, periods, entities) = MtaSubwayAlerts
        .process_alert(entity_id.clone(), input)
        .unwrap();
    assert_links(&alert, &translations, &periods, &entities);
    assert_eq!(alert.source, Source::MtaSubway);
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
