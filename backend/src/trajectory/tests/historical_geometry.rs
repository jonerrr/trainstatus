use std::collections::HashMap;

use crate::{
    network::{
        index::IndexedStop,
        types::stop::{CompassDirection, MtaBusStopData, StopData},
    },
    operations::types::trip::{MtaBusData, StopTimeData, StopVisit, TripData, TripRun},
    source::Source,
    trajectory::{
        TrajectoryCache,
        snapshot::{GeometryInputs, snapshot_from_geometry},
    },
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
