use backend::{
    sources::{
        mta_bus::static_data as mta_bus_static,
        mta_subway::static_data as mta_subway_static,
        njt_bus::{patterns::PatternFeature, static_data as njt_bus_static},
    },
    static_data::dataset::StaticDataset,
};

pub fn mta_subway_dataset() -> StaticDataset {
    let root = fixture_root();
    let manifest = backend::fixtures::load_manifest_for(
        &root,
        backend::models::source::Source::MtaSubway,
        backend::fixtures::FixtureKind::Static,
        "basic",
    )
    .expect("MTA subway static manifest should load");
    let infrastructure = backend::fixtures::read_json_payload(&root, &manifest, "infrastructure")
        .expect("MTA subway infrastructure fixture should load");
    let exit_strategy = backend::fixtures::read_json_payload(&root, &manifest, "exit_strategy")
        .expect("MTA subway exit strategy fixture should load");

    mta_subway_static::build_static_dataset_from_fixtures(infrastructure, exit_strategy)
        .expect("MTA subway static fixture should build")
}

pub fn mta_bus_dataset() -> StaticDataset {
    let root = fixture_root();
    let manifest = backend::fixtures::load_manifest_for(
        &root,
        backend::models::source::Source::MtaBus,
        backend::fixtures::FixtureKind::Static,
        "basic",
    )
    .expect("MTA bus static manifest should load");
    let infrastructure = backend::fixtures::read_json_payload(&root, &manifest, "infrastructure")
        .expect("MTA bus infrastructure fixture should load");

    mta_bus_static::build_static_dataset_from_fixture(infrastructure)
        .expect("MTA bus static fixture should build")
}

pub fn njt_bus_dataset() -> StaticDataset {
    let root = fixture_root();
    let manifest = backend::fixtures::load_manifest_for(
        &root,
        backend::models::source::Source::NjtBus,
        backend::fixtures::FixtureKind::Static,
        "basic",
    )
    .expect("NJT bus static manifest should load");
    let gtfs_path = manifest.fixture_dir(&root).join("raw/patterns_gtfs.zip");
    let gtfs =
        gtfs_structures::Gtfs::from_path(gtfs_path).expect("NJT bus GTFS fixture should load");
    let patterns: Vec<PatternFeature> = serde_json::from_value(
        backend::fixtures::read_json_payload(&root, &manifest, "operating_patterns")
            .expect("NJT bus operating-pattern fixture should load"),
    )
    .expect("NJT bus operating-pattern fixture should decode");

    njt_bus_static::build_static_dataset_from_patterns(&gtfs, patterns).dataset
}

pub fn fixture_root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

pub fn fixed_time() -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::from_timestamp(1_780_000_000, 0).unwrap()
}
