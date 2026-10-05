use crate::support::alerts::{assert_links, fixture_alert};
use backend::{
    integrations::gtfs_alert::GtfsAlertSource,
    models::{
        alert::{AlertFormat, AlertSection},
        source::Source,
    },
    sources::mta_bus::alerts::MtaBusAlerts,
};

#[test]
fn mercury_alert_preserves_detour_text_formats_and_routes() {
    let input = fixture_alert(Source::MtaBus, "lmm:alert:552251");
    let (alert, translations, periods, entities) = MtaBusAlerts
        .process_alert("lmm:alert:552251".into(), input)
        .unwrap();
    assert_links(&alert, &translations, &periods, &entities);
    assert_eq!(alert.source, Source::MtaBus);
    assert_eq!(alert.original_id, "lmm:alert:552251");
    assert_eq!(alert.created_at.timestamp(), 1_783_321_472);
    assert_eq!(periods[0].start_time.timestamp(), 1_783_321_472);
    assert_eq!(translations.len(), 4);
    assert!(
        translations
            .iter()
            .any(|t| t.section == AlertSection::Description
                && t.format == AlertFormat::Plain
                && t.language == "en"
                && t.text.contains("Dean St/Utica Ave"))
    );
    assert!(translations.iter().any(|t| t.format == AlertFormat::Html
        && t.language == "en"
        && t.text.contains("<strong>B15</strong>")));
    assert!(
        entities
            .iter()
            .any(|e| e.route_id.as_deref() == Some("B15") && e.sort_order == 31)
    );
    assert!(
        entities
            .iter()
            .any(|e| e.route_id.as_deref() == Some("B65") && e.sort_order == 31)
    );
}

#[test]
fn non_mercury_alert_is_skipped() {
    let mut input = fixture_alert(Source::MtaBus, "lmm:alert:552251");
    input.mercury_alert = None;
    assert!(
        MtaBusAlerts
            .process_alert("unclassified".into(), input)
            .is_none()
    );
}
