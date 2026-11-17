use crate::geometry::Geom;
use crate::network::dataset::NetworkDataset;
use crate::network::index::NetworkIndex;
use crate::network::store::Network;
use crate::network::types::route::{Route, RouteData};
use crate::network::types::stop::CompassDirection;
use crate::network::types::stop::{MtaBusStopData, RouteStop, RouteStopData, Stop, StopData};
use crate::source::Source;
use crate::storage::store::Storage;
use geo::Point;

fn sample_dataset() -> NetworkDataset {
    let mut dataset = NetworkDataset::new(Source::MtaBus);
    dataset.routes.push(Route {
        id: "b63".into(),
        long_name: "B63".into(),
        short_name: "B63".into(),
        color: "B933AD".into(),
        text_color: "FFFFFF".into(),
        data: RouteData::MtaBus(crate::network::types::route::MtaBusRouteData {
            sort_key: 0,
            service_types: vec!["local".into()],
            borough: None,
            name_prefix: "B".into(),
            name_number: 63,
            name_suffix: None,
            directions: Vec::new(),
            shape_ids: Vec::new(),
        }),
    });
    dataset.stops.push(Stop {
        id: "1".into(),
        name: "Bay Ridge".into(),
        geom: Geom(Point::new(-74.0, 40.6).into()),
        nearby_stops: Vec::new(),
        data: StopData::MtaBus(MtaBusStopData {
            bearing: None,
            is_boardable: true,
            direction: CompassDirection::Unknown,
        }),
        routes: vec![RouteStop {
            route_id: "B63".into(),
            stop_id: "1".into(),
            stop_sequence: 1,
            data: RouteStopData::MtaBus {
                direction_ids: vec![0],
                shape_ids: vec!["shape-a".into()],
            },
        }],
    });
    dataset.route_stops = dataset.stops[0].routes.clone();
    dataset
}

#[tokio::test]
async fn persist_and_restore_revision_from_storage() {
    let dir = std::env::temp_dir().join(format!("network-store-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let storage = Storage::open(&dir).unwrap();
    let index = NetworkIndex::new();
    let network = Network::new(storage.clone(), index.clone()).unwrap();
    let dataset = sample_dataset();
    network.persist(dataset).await.unwrap();
    assert!(
        storage.root().join("network/current.trst").is_file(),
        "persist must install global current pointer"
    );
    let restored_index = NetworkIndex::new();
    let restored = Network::new(storage, restored_index).unwrap();
    let revision = restored
        .network_index()
        .get(Source::MtaBus)
        .expect("restored bundle must publish");
    assert!(revision.catalog.is_some());
    assert!(revision.routes.contains_key("B63"));
    assert!(revision.stops.contains_key("1"));
    let _ = std::fs::remove_dir_all(&dir);
}
#[tokio::test]
async fn uncertain_pointer_install_fences_clones_until_reopen() {
    use std::sync::atomic::Ordering;

    let dir =
        std::env::temp_dir().join(format!("network-uncertain-pointer-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let storage = Storage::open(&dir).unwrap();
    let network = Network::new(storage.clone(), NetworkIndex::new()).unwrap();
    network.persist(sample_dataset()).await.unwrap();
    let old = network.index().get(Source::MtaBus).unwrap();
    let old_updated = network.index().source_updated_at(Source::MtaBus);
    let old_pointer = crate::network::publication::load_current_publication(storage.root())
        .unwrap()
        .unwrap()
        .id;

    let mut changed = sample_dataset();
    changed.routes[0].id = "b64".into();
    changed.routes[0].long_name = "B64".into();
    changed.routes[0].short_name = "B64".into();
    changed.route_stops[0].route_id = "B64".into();
    changed.stops[0].routes[0].route_id = "B64".into();
    network
        .fail_after_pointer_install
        .store(true, Ordering::Release);
    let error = network.persist(changed).await.unwrap_err();
    assert!(
        error
            .to_string()
            .contains("injected failure after current pointer installation")
    );
    let installed = crate::network::publication::load_current_publication(storage.root())
        .unwrap()
        .unwrap();
    assert_ne!(installed.id, old_pointer);
    assert!(
        network
            .index()
            .get(Source::MtaBus)
            .unwrap()
            .routes
            .contains_key("B63")
    );
    assert_eq!(
        network.index().source_updated_at(Source::MtaBus),
        old_updated
    );

    let mut other = sample_dataset();
    other.source = Source::NjtBus;
    other.routes[0].data = RouteData::NjtBus;
    other.stops[0].data = StopData::NjtBus(crate::network::types::stop::NjtBusStopData {
        stop_code: "10001".into(),
    });
    other.route_stops[0].data = RouteStopData::NjtBus { directions: vec![] };
    other.stops[0].routes[0].data = RouteStopData::NjtBus { directions: vec![] };
    other.trip_patterns.insert(
        "trip".into(),
        crate::network::index::TripPattern {
            route_id: "B63".into(),
            direction: 0,
            shape_id: None,
        },
    );
    let clone = network.clone();
    let error = clone.persist(other.clone()).await.unwrap_err();
    assert!(error.to_string().contains("publication is uncertain"));
    assert!(clone.restore_revision(Source::MtaBus).await.is_err());
    assert!(clone.index().source_updated_at(Source::NjtBus).is_none());
    assert_eq!(
        crate::network::publication::load_current_publication(storage.root())
            .unwrap()
            .unwrap()
            .id,
        installed.id
    );

    let reopened = Network::new(storage.clone(), NetworkIndex::new()).unwrap();
    let restored = reopened.index().get(Source::MtaBus).unwrap();
    assert!(restored.routes.contains_key("B64"));
    assert!(!restored.routes.contains_key("B63"));
    let disk_catalog =
        crate::network::artifact::NetworkBundle::load_catalog(&storage, Source::MtaBus)
            .unwrap()
            .unwrap();
    assert_eq!(restored.catalog.as_ref().unwrap().body, disk_catalog.body);
    assert_ne!(
        restored.catalog.as_ref().unwrap().etag,
        old.catalog.as_ref().unwrap().etag
    );
    reopened.persist(other).await.unwrap();
    let bus = reopened.index().get(Source::MtaBus).unwrap();
    assert!(bus.routes.contains_key("B64"));
    let catalog = crate::network::artifact::NetworkBundle::load_catalog(&storage, Source::MtaBus)
        .unwrap()
        .unwrap();
    assert_eq!(bus.catalog.as_ref().unwrap().body, catalog.body);
    assert!(reopened.index().get(Source::NjtBus).is_some());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn shared_shape_assets_are_source_scoped_and_validate_identity() {
    let dir = std::env::temp_dir().join(format!("network-shape-assets-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let storage = Storage::open(&dir).unwrap();
    let bus = crate::network::assets::ShapeAssets::open(&storage, Source::MtaBus).unwrap();
    let subway = crate::network::assets::ShapeAssets::open(&storage, Source::MtaSubway).unwrap();
    let geom = Geom(Point::new(-74.0, 40.6).into());
    let id = bus.put(&geom).unwrap();
    assert_eq!(bus.put(&geom).unwrap(), id);
    assert_eq!(bus.get(id).unwrap(), geom);
    assert!(subway.get(id).is_err());
    let path = dir
        .join("network/shapes/mta_bus")
        .join(format!("{}.trst", id.to_hex()));
    std::fs::write(path, b"corrupt").unwrap();
    assert!(bus.get(id).is_err());
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn section_reads_do_not_load_unrelated_artifacts() {
    let dir = std::env::temp_dir().join(format!("network-section-reads-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let storage = Storage::open(&dir).unwrap();
    let network = Network::new(storage.clone(), NetworkIndex::new()).unwrap();
    network.persist(sample_dataset()).await.unwrap();
    let expected_catalog = network
        .index()
        .get(Source::MtaBus)
        .unwrap()
        .catalog
        .clone()
        .unwrap();
    let publication = crate::network::publication::load_current_publication(storage.root())
        .unwrap()
        .unwrap();
    let source = &publication.sources[&Source::MtaBus];
    let manifest_path = storage
        .root()
        .join(crate::network::publication::PUBLICATIONS_DIR)
        .join(&publication.id)
        .join(&source.relative_dir)
        .join(crate::network::artifact::MANIFEST_NAME);
    let manifest: crate::network::artifact::NetworkManifest =
        crate::storage::codec::read(&manifest_path).unwrap();
    for id in &manifest.routes {
        std::fs::write(
            crate::network::chunks::blob_path(storage.root(), id),
            b"corrupt",
        )
        .unwrap();
    }
    let (stops, _) = crate::network::artifact::NetworkBundle::load_stops(&storage, Source::MtaBus)
        .unwrap()
        .unwrap();
    assert_eq!(stops[0].name, "Bay Ridge");
    assert!(
        crate::network::artifact::NetworkBundle::load_schedules(&storage, Source::MtaBus)
            .unwrap()
            .is_empty()
    );
    let catalog = crate::network::artifact::NetworkBundle::load_catalog(&storage, Source::MtaBus)
        .unwrap()
        .unwrap();
    assert_eq!(catalog.etag, manifest.catalog_etag);
    assert!(!catalog.body.is_empty());
    assert_eq!(catalog.body, expected_catalog.body);
    assert_eq!(catalog.etag, expected_catalog.etag);
    assert!(crate::network::artifact::revision_from_manifest(storage.root(), &manifest).is_err());
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn persisted_schedule_occurrences_share_template_and_keep_expiry() {
    use crate::network::schedule::{ScheduledStopTime, ScheduledTrip, TripTemplate};
    use std::sync::Arc;
    let dir =
        std::env::temp_dir().join(format!("network-schedule-template-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let storage = Storage::open(&dir).unwrap();
    let network = Network::new(storage.clone(), NetworkIndex::new()).unwrap();
    let template = Arc::new(TripTemplate {
        trip_id: "t".into(),
        route_id: "b63".into(),
        headsign: "Bay Ridge".into(),
        direction_id: 0,
        start_seconds: 90_000,
        stop_times: vec![ScheduledStopTime {
            stop_id: "1".into(),
            arrival: 90_000,
            departure: 90_120,
            stop_sequence: 1,
        }],
    });
    let mut dataset = sample_dataset();
    dataset.scheduled_trips = vec![
        ScheduledTrip {
            template: template.clone(),
            start_date: chrono::NaiveDate::from_ymd_opt(2026, 3, 8).unwrap(),
        },
        ScheduledTrip {
            template,
            start_date: chrono::NaiveDate::from_ymd_opt(2026, 3, 9).unwrap(),
        },
    ];
    network.persist(dataset).await.unwrap();
    let records =
        crate::network::artifact::NetworkBundle::load_schedules(&storage, Source::MtaBus).unwrap();
    assert_eq!(records.len(), 2);
    assert!(Arc::ptr_eq(
        &records[0].trip.template,
        &records[1].trip.template
    ));
    assert_eq!(records[0].expires_at, records[1].expires_at);
    assert_eq!(records[0].trip.template.stop_times[0].departure, 90_120);
    assert_eq!(
        records[0].trip.start_date,
        chrono::NaiveDate::from_ymd_opt(2026, 3, 8).unwrap()
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn static_and_retired_shapes_share_the_accepted_asset() {
    let dir = std::env::temp_dir().join(format!("network-shape-retirement-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let storage = Storage::open(&dir).unwrap();
    let network = Network::new(storage.clone(), NetworkIndex::new()).unwrap();
    let mut dataset = sample_dataset();
    let geom = Geom(geo::LineString::from(vec![(-74.0, 40.6), (-73.9, 40.7)]).into());
    dataset.shapes.push(crate::network::types::shape::Shape {
        id: "shape-a".into(),
        source: Source::MtaBus,
        geom: geom.clone(),
    });
    network.persist(dataset.clone()).await.unwrap();
    let assets = crate::network::assets::ShapeAssets::open(&storage, Source::MtaBus).unwrap();
    let id = assets.put(&geom).unwrap();
    dataset.shapes.clear();
    network.persist(dataset).await.unwrap();
    let restored = Network::new(storage.clone(), NetworkIndex::new()).unwrap();
    assert_eq!(
        restored
            .index()
            .shape_geom(Source::MtaBus, "shape-a")
            .unwrap()
            .as_deref(),
        Some(&geom)
    );
    assert_eq!(assets.get(id).unwrap(), geom);
    std::fs::remove_file(
        dir.join("network/shapes/mta_bus")
            .join(format!("{id}.trst")),
    )
    .unwrap();
    let lazy = Network::new(storage.clone(), NetworkIndex::new()).unwrap();
    assert!(lazy.index().shape_geom(Source::MtaBus, "shape-a").is_err());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn accepted_binding_view_shares_the_durable_shape_cache() {
    let dir = std::env::temp_dir().join(format!("network-binding-cache-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let storage = Storage::open(&dir).unwrap();
    let assets = crate::network::assets::ShapeAssets::open(&storage, Source::MtaBus).unwrap();
    let first = assets.put(&Geom(Point::new(-74.0, 40.6).into())).unwrap();
    let second = assets.put(&Geom(Point::new(-73.9, 40.7).into())).unwrap();
    let index = crate::network::shape_index::ShapeIndex::from_assets(
        assets.clone(),
        std::collections::HashMap::from([("alias".into(), second)]),
    );
    let accepted = index.with_bindings(
        std::collections::HashMap::from([("accepted".into(), first)]),
        assets.clone(),
    );
    assert_eq!(index.asset_id("alias"), Some(second));
    let geometry = accepted.get("accepted").unwrap().unwrap();
    let other_view = index.with_bindings(
        std::collections::HashMap::from([("old".into(), first)]),
        assets.clone(),
    );
    assert!(std::sync::Arc::ptr_eq(
        &geometry,
        &other_view.get("old").unwrap().unwrap()
    ));
    assert_ne!(geometry, index.get("alias").unwrap().unwrap());
    let prepared =
        crate::network::shape_index::ShapeIndex::from_prepared(std::collections::HashMap::new());
    let prepared_view = prepared.with_bindings(
        std::collections::HashMap::from([("accepted".into(), first)]),
        assets,
    );
    assert_eq!(prepared_view.get("accepted").unwrap().unwrap(), geometry);
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn importing_another_source_reuses_unrelated_sections_without_decoding_them() {
    use crate::network::schedule::{ScheduledTrip, TripTemplate};
    use crate::network::types::stop::{MtaSubwayStopData, StopType};
    use std::sync::Arc;
    let dir = std::env::temp_dir().join(format!("network-selective-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let storage = Storage::open(&dir).unwrap();
    let network = Network::new(storage.clone(), NetworkIndex::new()).unwrap();
    let mut bus = sample_dataset();
    bus.shapes.push(crate::network::types::shape::Shape {
        id: "shape-a".into(),
        source: Source::MtaBus,
        geom: Geom(geo::LineString::from(vec![(-74.0, 40.6), (-73.9, 40.7)]).into()),
    });
    bus.scheduled_trips.push(ScheduledTrip {
        template: Arc::new(TripTemplate {
            trip_id: "trip".into(),
            route_id: "b63".into(),
            headsign: "Bay Ridge".into(),
            direction_id: 0,
            start_seconds: 0,
            stop_times: vec![],
        }),
        start_date: chrono::NaiveDate::from_ymd_opt(2026, 3, 8).unwrap(),
    });
    network.persist(bus).await.unwrap();
    let before = crate::network::publication::load_current_publication(storage.root())
        .unwrap()
        .unwrap();
    let manifest =
        crate::network::publication::publication_manifest(storage.root(), &before, Source::MtaBus)
            .unwrap()
            .unwrap();
    let shape_id = network
        .index()
        .get(Source::MtaBus)
        .unwrap()
        .shapes
        .asset_id("shape-a")
        .unwrap();
    std::fs::remove_file(
        dir.join("network/shapes/mta_bus")
            .join(format!("{shape_id}.trst")),
    )
    .unwrap();
    for id in &manifest.schedule_templates {
        std::fs::write(
            crate::network::chunks::blob_path(storage.root(), id),
            b"corrupt",
        )
        .unwrap();
    }
    let mut subway = NetworkDataset::new(Source::MtaSubway);
    let mut route = sample_dataset().routes.remove(0);
    route.data = RouteData::MtaSubway;
    subway.routes.push(route);
    let mut stop = sample_dataset().stops.remove(0);
    stop.data = StopData::MtaSubway(MtaSubwayStopData {
        gtfs_stop_id: "1".into(),
        station_group_id: "1".into(),
        bubble_id: "1".into(),
        platform_edges: vec![],
        line: String::new(),
        is_major: false,
        north_headsign: String::new(),
        south_headsign: String::new(),
    });
    stop.routes[0].data = RouteStopData::MtaSubway {
        stop_type: StopType::FullTime,
    };
    subway.route_stops = stop.routes.clone();
    subway.stops.push(stop);
    network.persist(subway).await.unwrap();
    let after = crate::network::publication::load_current_publication(storage.root())
        .unwrap()
        .unwrap();
    let reused =
        crate::network::publication::publication_manifest(storage.root(), &after, Source::MtaBus)
            .unwrap()
            .unwrap();
    assert_eq!(manifest.schedule_templates, reused.schedule_templates);
    assert_eq!(manifest.schedules, reused.schedules);
    assert_eq!(manifest.shapes, reused.shapes);
    assert_eq!(manifest.trip_patterns, reused.trip_patterns);
    assert_eq!(manifest.stop_remap, reused.stop_remap);
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn njt_sections_and_retired_stops_round_trip_source_payloads() {
    use crate::network::index::TripPattern;
    use crate::network::types::stop::NjtBusStopData;
    let dir = std::env::temp_dir().join(format!("network-njt-wire-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let storage = Storage::open(&dir).unwrap();
    let network = Network::new(storage.clone(), NetworkIndex::new()).unwrap();
    let mut dataset = sample_dataset();
    dataset.source = Source::NjtBus;
    dataset.routes[0].data = RouteData::NjtBus;
    dataset.stops[0].data = StopData::NjtBus(NjtBusStopData {
        stop_code: "10001".into(),
    });
    dataset.route_stops[0].data = RouteStopData::NjtBus { directions: vec![] };
    dataset.trip_patterns.insert(
        "trip".into(),
        TripPattern {
            route_id: "B63".into(),
            direction: 0,
            shape_id: None,
        },
    );
    network.persist(dataset.clone()).await.unwrap();
    let publication = crate::network::publication::load_current_publication(storage.root())
        .unwrap()
        .unwrap();
    let manifest = crate::network::publication::publication_manifest(
        storage.root(),
        &publication,
        Source::NjtBus,
    )
    .unwrap()
    .unwrap();
    let routes = crate::network::artifact::read_routes(storage.root(), &manifest).unwrap();
    let stops = crate::network::artifact::read_stops(storage.root(), &manifest).unwrap();
    let memberships =
        crate::network::artifact::read_route_stops(storage.root(), &manifest).unwrap();
    assert!(matches!(&routes[0].data, RouteData::NjtBus));
    assert!(matches!(&stops[0].data, StopData::NjtBus(data) if data.stop_code == "10001"));
    assert!(matches!(&memberships[0].data, RouteStopData::NjtBus { .. }));
    dataset.stops[0].id = "2".into();
    dataset.route_stops[0].stop_id = "2".into();
    network.persist(dataset).await.unwrap();
    let restored = Network::new(storage.clone(), NetworkIndex::new()).unwrap();
    assert!(
        matches!(restored.index().stop(Source::NjtBus, "1").unwrap().data,
        StopData::NjtBus(data) if data.stop_code == "10001")
    );
    let _ = std::fs::remove_dir_all(&dir);
}
