use crate::support::fixture_root;
use backend::fixtures;

#[test]
fn manifests_reference_existing_parseable_payloads() {
    let root = fixture_root();
    let manifests = fixtures::discover_manifests(&root).expect("fixture discovery");
    assert!(
        !manifests.is_empty(),
        "expected committed fixture manifests"
    );

    for (_path, manifest) in manifests {
        assert!(
            !manifest.payloads.is_empty(),
            "fixture {}/{}/{} must declare payloads",
            manifest.source,
            manifest.kind,
            manifest.scenario
        );
        fixtures::verify_manifest_payloads(&root, &manifest).expect("fixture payloads parse");
    }
}
