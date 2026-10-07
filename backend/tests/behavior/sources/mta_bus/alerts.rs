use crate::support::alerts::{assert_links, fixture_alert_where};
use backend::{
    alerts::gtfs::GtfsAlertSource,
    models::{
        alert::{AlertFormat, AlertSection},
        source::Source,
    },
    sources::mta_bus::alerts::MtaBusAlerts,
};

#[test]
fn mercury_alert_preserves_detour_text_formats_and_routes() {
    let (entity_id, input) = fixture_alert_where(Source::MtaBus, |alert| {
        alert.mercury_alert.is_some()
            && !alert.active_period.is_empty()
            && alert.header_text.is_some()
            && alert.description_text.is_some()
            && !alert.informed_entity.is_empty()
    });
    let (alert, translations, periods, entities) = MtaBusAlerts
        .process_alert(entity_id.clone(), input)
        .unwrap();
    assert_links(&alert, &translations, &periods, &entities);
    assert_eq!(alert.source, Source::MtaBus);
    assert_eq!(alert.original_id, entity_id);
    assert_eq!(alert.created_at, periods[0].start_time);
    assert!(
        translations
            .iter()
            .any(|t| t.section == AlertSection::Description
                && t.format == AlertFormat::Plain
                && t.language == "en")
    );
    assert!(
        translations
            .iter()
            .any(|t| t.format == AlertFormat::Html && t.language == "en")
    );
    assert!(entities.iter().any(|e| e.route_id.is_some()));
}

#[test]
fn non_mercury_alert_is_skipped() {
    let (_, mut input) = fixture_alert_where(Source::MtaBus, |alert| alert.mercury_alert.is_some());
    input.mercury_alert = None;
    assert!(
        MtaBusAlerts
            .process_alert("unclassified".into(), input)
            .is_none()
    );
}
