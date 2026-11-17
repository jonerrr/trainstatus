use crate::alerts::store::test_support;
use crate::geometry::Geom;
use crate::network::dataset::NetworkDataset;
use crate::network::index::{NetworkIndex, NetworkRevision};
use crate::network::types::route::MtaBusRouteData;
use crate::network::types::route::{Route, RouteData};
use crate::network::types::stop::{
    CompassDirection, MtaBusStopData, RouteStop, RouteStopData, Stop, StopData,
};
use crate::source::Source;
use crate::storage::store::Storage;
use std::path::PathBuf;
use uuid::Uuid;

pub struct TestDir(pub PathBuf);

impl TestDir {
    pub fn new() -> Self {
        let path = std::env::temp_dir().join(format!("trainstatus-alerts-test-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    pub fn storage(&self) -> Storage {
        Storage::open(&self.0).unwrap()
    }

    pub fn alerts_root(&self) -> PathBuf {
        self.0.join("alerts")
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

pub fn install_bus_route(index: &NetworkIndex, route_id: &str) {
    let mut dataset = NetworkDataset::new(Source::MtaBus);
    dataset.routes.push(Route {
        id: route_id.into(),
        long_name: String::new(),
        short_name: route_id.into(),
        color: "FFFFFF".into(),
        text_color: "000000".into(),
        data: RouteData::MtaBus(MtaBusRouteData {
            sort_key: 0,
            service_types: Vec::new(),
            borough: None,
            name_prefix: "T".into(),
            name_number: 0,
            name_suffix: None,
            directions: Vec::new(),
            shape_ids: Vec::new(),
        }),
    });
    let membership = RouteStop {
        route_id: route_id.into(),
        stop_id: "TEST_STOP".into(),
        stop_sequence: 1,
        data: RouteStopData::MtaBus {
            direction_ids: vec![0],
            shape_ids: Vec::new(),
        },
    };
    dataset.stops.push(Stop {
        id: "TEST_STOP".into(),
        name: "Test stop".into(),
        geom: Geom(geo::Point::new(-74.0, 40.6).into()),
        nearby_stops: Vec::new(),
        data: StopData::MtaBus(MtaBusStopData {
            bearing: None,
            is_boardable: true,
            direction: CompassDirection::Unknown,
        }),
        routes: vec![membership.clone()],
    });
    dataset.route_stops.push(membership);
    dataset.validate().unwrap();
    index.publish_bundle(vec![NetworkRevision::from_dataset(&dataset)]);
}

pub fn journal_watermark(dir: &TestDir) -> (u64, u64) {
    test_support::journal_watermark_parts(&dir.alerts_root()).unwrap()
}

pub fn append_stale_journal_frame(dir: &TestDir, source: Source) -> anyhow::Result<()> {
    test_support::append_stale_journal_frame(&dir.alerts_root(), source)
}

pub fn write_uncommitted_journal_only_njt(
    dir: &TestDir,
    alert: &crate::alerts::types::AlertDetails,
) -> anyhow::Result<()> {
    test_support::write_uncommitted_journal_only(
        &dir.alerts_root(),
        Source::NjtBus,
        vec![alert.clone()],
    )
}

pub fn write_manifest_without_journal(dir: &TestDir) -> anyhow::Result<()> {
    test_support::write_manifest_without_journal(&dir.alerts_root())
}

pub fn record_paths_in_bucket_pages(dir: &TestDir, source: Source) -> Vec<String> {
    test_support::record_paths_in_bucket_pages(&dir.alerts_root(), source).unwrap()
}
