use super::*;
use crate::{
    models::{
        position::MtaBusPositionData,
        trip::{MtaBusData, StopTimeData},
    },
    realtime::IngestionChanges,
    static_data::index::{IndexedRoute, StaticTransitRevision},
};
use chrono::TimeZone;
use serde_json::{Value, json};

fn at(seconds: i64) -> DateTime<Utc> {
    Utc.timestamp_opt(1_780_000_000 + seconds, 0).unwrap()
}

fn trip(route_id: &str, vehicle_id: &str) -> Trip {
    Trip {
        id: Uuid::now_v7(),
        original_id: format!("{route_id}-{vehicle_id}"),
        vehicle_id: vehicle_id.into(),
        route_id: route_id.into(),
        shape_ids: vec![],
        direction: 1,
        created_at: at(0),
        updated_at: at(0),
        data: TripData::MtaBus(MtaBusData {
            deviation: Some(90),
        }),
    }
}

fn call(trip: &Trip, stop_sequence: i32, stop_id: &str, offset: i64) -> StopTime {
    StopTime {
        trip_id: trip.id,
        stop_sequence,
        stop_id: stop_id.into(),
        arrival: at(offset),
        departure: at(offset + 30),
        data: StopTimeData::MtaBus,
    }
}

fn position(trip_id: Option<Uuid>, vehicle_id: &str, stop_id: &str) -> VehiclePosition {
    VehiclePosition {
        vehicle_id: vehicle_id.into(),
        trip_id,
        stop_id: Some(stop_id.into()),
        updated_at: at(-15),
        data: PositionData::MtaBus(MtaBusPositionData {
            bearing: 0.,
            passengers: Some(12),
            capacity: Some(60),
            status: Some("approaching".into()),
            phase: None,
        }),
        geom: None,
    }
}

fn revision(route_ids: &[&str]) -> Arc<StaticTransitRevision> {
    Arc::new(StaticTransitRevision {
        source: Source::MtaBus,
        routes: route_ids
            .iter()
            .map(|id| {
                (
                    (*id).into(),
                    IndexedRoute {
                        color: String::new(),
                        shape_ids: vec![],
                    },
                )
            })
            .collect(),
        stops: HashMap::new(),
        shapes: HashMap::new(),
        route_stop_shapes: HashMap::new(),
        trip_patterns: HashMap::new(),
        stop_remap: HashMap::new(),
        scheduled_trips: HashMap::new(),
    })
}

struct Case {
    snapshot: PassengerSnapshot,
    linked: Trip,
    vehicle_only: Trip,
}

fn live_case() -> Case {
    let linked = trip("A", "bus-1");
    let vehicle_only = trip("B", "bus-2");
    let stop_times = vec![
        call(&linked, 2, "X", 600),
        call(&vehicle_only, 0, "Y", 60),
        call(&linked, 0, "X", 0),
        call(&linked, 1, "Y", 300),
    ];
    let persisted = Arc::new(PersistedSnapshot {
        source: Source::MtaBus,
        trips: vec![linked.clone(), vehicle_only.clone()],
        stop_times,
        positions: vec![
            position(Some(linked.id), "bus-1", "Y"),
            position(None, "bus-2", "Y"),
        ],
        changed_trip_ids: HashSet::new(),
        changes: IngestionChanges::default(),
        static_revision: revision(&["A", "B", "C"]),
    });
    let route_ids = persisted
        .static_revision
        .routes
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    Case {
        snapshot: PassengerSnapshot::new(Source::MtaBus, None, Inputs::Live(persisted), route_ids),
        linked,
        vehicle_only,
    }
}

fn decode(bytes: &Bytes) -> Value {
    serde_json::from_slice(bytes).unwrap()
}

#[test]
fn requested_routes_contain_ordered_visits_status_and_explicit_empty_routes() {
    let case = live_case();
    let body = case
        .snapshot
        .encode(&RouteSelection::parse("C,A,A").unwrap())
        .unwrap();
    let body = decode(&body);
    assert_eq!(body["source"], "mta_bus");
    assert!(body.get("at").is_none());
    assert_eq!(
        body["routes"]
            .as_object()
            .unwrap()
            .keys()
            .collect::<Vec<_>>(),
        ["A", "C"]
    );
    assert_eq!(
        body["routes"]["C"],
        json!({"trips": [], "stops": [], "calls": {
            "trip": [], "stop": [], "sequence": [], "arrival": [], "departure": []
        }})
    );
    let route = &body["routes"]["A"];
    assert_eq!(route["trips"][0]["id"], case.linked.id.to_string());
    assert_eq!(route["trips"][0]["vehicle_id"], "bus-1");
    assert_eq!(route["trips"][0]["data"]["deviation"], 90);
    assert_eq!(
        route["trips"][0]["status"],
        json!({
            "updated_at": 1_780_000_000 - 15,
            "stop_id": "Y",
            "data": {"source": "mta_bus", "passengers": 12, "capacity": 60, "status": "approaching"}
        })
    );
    assert_eq!(route["stops"], json!(["X", "Y"]));
    assert_eq!(
        route["calls"],
        json!({
            "trip": [0, 0, 0],
            "stop": [0, 1, 0],
            "sequence": [0, 1, 2],
            "arrival": [1_780_000_000, 1_780_000_300, 1_780_000_600],
            "departure": [1_780_000_030, 1_780_000_330, 1_780_000_630],
        })
    );
}

#[test]
fn status_falls_back_to_unlinked_vehicle_report() {
    let case = live_case();
    let body = decode(
        &case
            .snapshot
            .encode(&RouteSelection::Routes(vec!["B".into()]))
            .unwrap(),
    );
    let trip = &body["routes"]["B"]["trips"][0];
    assert_eq!(trip["id"], case.vehicle_only.id.to_string());
    assert_eq!(trip["status"]["stop_id"], "Y");
}

#[test]
fn unknown_routes_are_reported_and_malformed_lists_rejected() {
    let case = live_case();
    match case
        .snapshot
        .encode(&RouteSelection::parse("A,Z,Q").unwrap())
    {
        Err(ServiceError::UnknownRoutes(unknown)) => assert_eq!(unknown, ["Q", "Z"]),
        _ => panic!("unknown routes must be rejected"),
    }
    for raw in ["", "A,", ",A", "A,,B"] {
        assert!(RouteSelection::parse(raw).is_none(), "{raw:?}");
    }
}

#[test]
fn whole_source_encoding_is_shared_within_a_generation() {
    let case = live_case();
    let first = case.snapshot.encode(&RouteSelection::All).unwrap();
    let second = case.snapshot.encode(&RouteSelection::All).unwrap();
    assert_eq!(first.as_ptr(), second.as_ptr());
    let body = decode(&first);
    assert_eq!(
        body["routes"]
            .as_object()
            .unwrap()
            .keys()
            .collect::<Vec<_>>(),
        ["A", "B", "C"]
    );
}

#[test]
fn historical_snapshot_echoes_at_and_has_no_status() {
    let retained = trip("A", "bus-1");
    let stop_times = vec![call(&retained, 0, "X", 0), call(&retained, 1, "Y", 300)];
    let snapshot = PassengerSnapshot::new(
        Source::MtaBus,
        Some(1_780_000_000),
        Inputs::Historical {
            trips: vec![retained],
            stop_times,
        },
        ["A".to_owned()],
    );
    let body = decode(&snapshot.encode(&RouteSelection::All).unwrap());
    assert_eq!(body["at"], 1_780_000_000);
    assert_eq!(body["routes"]["A"]["trips"][0]["status"], Value::Null);
    assert_eq!(body["routes"]["A"]["calls"]["sequence"], json!([0, 1]));
}
