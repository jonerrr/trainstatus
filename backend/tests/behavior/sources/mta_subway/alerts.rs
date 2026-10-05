use crate::support::alerts::{assert_links, fixture_alert};
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
    let input = fixture_alert(Source::MtaSubway, "lmm:alert:552384");
    let (alert, translations, periods, entities) = MtaSubwayAlerts
        .process_alert("lmm:alert:552384".into(), input)
        .unwrap();
    assert_links(&alert, &translations, &periods, &entities);
    assert_eq!(alert.source, Source::MtaSubway);
    assert_eq!(alert.original_id, "lmm:alert:552384");
    assert_eq!(alert.created_at.timestamp(), 1_783_344_587);
    assert_eq!(alert.updated_at.timestamp(), 1_783_349_444);
    assert_eq!(periods[0].start_time.timestamp(), 1_783_349_444);
    assert_eq!(periods[0].end_time, None);
    assert_eq!(translations.len(), 2);
    assert!(
        translations
            .iter()
            .any(|t| t.section == AlertSection::Header
                && t.format == AlertFormat::Plain
                && t.language == "en"
                && t.text.contains("[B][Q]"))
    );
    assert!(translations.iter().any(|t| t.format == AlertFormat::Html
        && t.language == "en"
        && t.text.contains("<strong>Parkside Av</strong>")));
    assert!(
        entities
            .iter()
            .any(|e| e.route_id.as_deref() == Some("Q") && e.sort_order == 30)
    );
    assert!(entities.iter().any(|e| e.stop_id.as_deref() == Some("D27")));
}

#[test]
fn non_mercury_alert_is_skipped() {
    let mut input = fixture_alert(Source::MtaSubway, "lmm:alert:552384");
    input.mercury_alert = None;
    assert!(
        MtaSubwayAlerts
            .process_alert("elevator".into(), input)
            .is_none()
    );
}
