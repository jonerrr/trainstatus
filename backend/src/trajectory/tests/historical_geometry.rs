use std::collections::HashMap;

use super::super::{
    cache::TrajectoryCache,
    snapshot::{GeometryInputs, snapshot_from_geometry},
};
use crate::{
    network::{
        index::IndexedStop,
        types::stop::{CompassDirection, MtaBusStopData, StopData},
    },
    operations::types::trip::{MtaBusData, StopTimeData, StopVisit, TripData, TripRun},
    source::Source,
};

#[tokio::test]
async fn relocated_stop_cannot_anchor_a_retained_shape() {
    let at = chrono::DateTime::from_timestamp(1_780_000_000, 0).unwrap();
    let trip = TripRun {
        id: uuid::Uuid::nil(),
        original_id: "retained-run".into(),
        vehicle_id: "vehicle".into(),
        route_id: "route".into(),
        shape_ids: vec!["asset".into()],
        direction: 0,
        created_at: at,
        updated_at: at,
        data: TripData::MtaBus(MtaBusData { deviation: None }),
    };
    let calls = vec![StopVisit {
        trip_id: trip.id,
        stop_id: "relocated".into(),
        stop_sequence: 7,
        arrival: at,
        departure: at,
        data: StopTimeData::MtaBus,
    }];
    let mut stops = HashMap::from([(
        "relocated".into(),
        IndexedStop {
            geom: geo::Point::new(-74.5, 40.5).into(),
            data: StopData::MtaBus(MtaBusStopData {
                bearing: None,
                is_boardable: true,
                direction: CompassDirection::Unknown,
            }),
        },
    )]);
    let shapes = HashMap::from([(
        "asset".into(),
        geo::LineString::from(vec![(-73.9, 40.7), (-73.9, 40.8)]).into(),
    )]);
    let cache = TrajectoryCache::new();
    let inputs = GeometryInputs {
        source: Source::MtaBus,
        historical: true,
        route_color: "#123456",
        stops: &stops,
        shapes: &shapes,
    };
    let error = snapshot_from_geometry(&trip, calls.clone(), vec![], &inputs, &cache, at)
        .await
        .err()
        .expect("a relocated stop must reject historical interpolation");
    assert!(error.to_string().contains("incompatible"));
    stops.get_mut("relocated").unwrap().geom = geo::Point::new(-73.9, 40.75).into();
    let inputs = GeometryInputs {
        source: Source::MtaBus,
        historical: true,
        route_color: "#123456",
        stops: &stops,
        shapes: &shapes,
    };
    let usable = snapshot_from_geometry(&trip, calls, vec![], &inputs, &cache, at)
        .await
        .unwrap();
    assert_eq!(usable.stops[0].stop_id, "relocated");
    assert!(usable.stops[0].stop_distance_m > 0.0);
    assert!(usable.stops[0].stop_distance_m < usable.shape_length_m);
}

#[tokio::test]
async fn live_and_history_resolve_loop_visits_by_sequence_instead_of_arrival_order() {
    let at = chrono::DateTime::from_timestamp(1_780_000_000, 0).unwrap();
    let trip = TripRun {
        id: uuid::Uuid::nil(),
        original_id: "loop".into(),
        vehicle_id: "bus".into(),
        route_id: "route".into(),
        shape_ids: vec!["loop".into()],
        direction: 0,
        created_at: at,
        updated_at: at,
        data: TripData::MtaBus(MtaBusData { deviation: None }),
    };
    let stop_data = StopData::MtaBus(MtaBusStopData {
        bearing: None,
        is_boardable: true,
        direction: CompassDirection::Unknown,
    });
    let stops = HashMap::from([
        (
            "A".into(),
            IndexedStop {
                geom: geo::Point::new(-74.0, 40.7).into(),
                data: stop_data.clone(),
            },
        ),
        (
            "B".into(),
            IndexedStop {
                geom: geo::Point::new(-73.999, 40.7).into(),
                data: stop_data.clone(),
            },
        ),
        (
            "C".into(),
            IndexedStop {
                geom: geo::Point::new(-74.001, 40.7).into(),
                data: stop_data,
            },
        ),
    ]);
    let shapes = HashMap::from([(
        "loop".into(),
        geo::LineString::from(vec![
            (-74.0, 40.7),
            (-73.999, 40.7),
            (-74.0, 40.7),
            (-74.001, 40.7),
        ])
        .into(),
    )]);
    let calls: Vec<_> = [("A", 0, 0), ("B", 1, 120), ("A", 2, 60), ("C", 3, 180)]
        .into_iter()
        .map(|(id, seq, seconds)| StopVisit {
            trip_id: trip.id,
            stop_id: id.into(),
            stop_sequence: seq,
            arrival: at + chrono::Duration::seconds(seconds),
            departure: at + chrono::Duration::seconds(seconds),
            data: StopTimeData::MtaBus,
        })
        .collect();
    let cache = TrajectoryCache::new();
    for historical in [false, true] {
        let inputs = GeometryInputs {
            source: Source::MtaBus,
            historical,
            route_color: "000000",
            stops: &stops,
            shapes: &shapes,
        };
        let snapshot = snapshot_from_geometry(&trip, calls.clone(), vec![], &inputs, &cache, at)
            .await
            .unwrap();
        assert_eq!(
            snapshot
                .stops
                .iter()
                .map(|s| s.stop_id.as_str())
                .collect::<Vec<_>>(),
            ["A", "B", "A", "C"]
        );
        assert!(
            snapshot
                .stops
                .windows(2)
                .all(|w| w[1].stop_distance_m > w[0].stop_distance_m)
        );
        assert!(snapshot.stops[2].stop_distance_m > 150.0);
    }
}
