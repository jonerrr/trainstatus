use backend::feed::Alert as GtfsAlert;
use backend::fixtures;
use backend::fixtures::FixtureKind;
use backend::source::Source;

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
