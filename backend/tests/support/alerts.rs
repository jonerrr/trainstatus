use backend::{
    feed::Alert as GtfsAlert,
    fixtures::{self, FixtureKind},
    models::{
        alert::{ActivePeriod, AffectedEntity, Alert, AlertTranslation},
        source::Source,
    },
};

pub fn fixture_alert_where<F>(source: Source, predicate: F) -> (String, GtfsAlert)
where
    F: Fn(&GtfsAlert) -> bool,
{
    let root = super::fixture_root();
    let manifest =
        fixtures::load_manifest_for(&root, source, FixtureKind::Alerts, "basic").unwrap();
    fixtures::read_gtfs_realtime_payload(&root, &manifest, "alerts")
        .unwrap()
        .entity
        .into_iter()
        .filter_map(|entity| {
            let alert = entity.alert?;
            predicate(&alert).then_some((entity.id, alert))
        })
        .next()
        .expect("fixture should contain a matching alert")
}

pub fn assert_links(
    alert: &Alert,
    translations: &[AlertTranslation],
    periods: &[ActivePeriod],
    entities: &[AffectedEntity],
) {
    assert!(!translations.is_empty());
    assert!(!periods.is_empty());
    assert!(!entities.is_empty());
    assert!(
        translations
            .iter()
            .all(|translation| translation.alert_id == alert.id)
    );
    assert!(periods.iter().all(|period| period.alert_id == alert.id));
    assert!(
        entities
            .iter()
            .all(|entity| entity.alert_id == alert.id && entity.source == alert.source)
    );
}
