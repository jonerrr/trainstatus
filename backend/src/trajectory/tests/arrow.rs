use crate::source::Source;
use crate::trajectory::{
    arrow::encode_render_units,
    geometry::build_shape_geometry,
    interpolation::hermite,
    types::{RenderUnit, Trajectory},
};
use arrow::array::{Array, FixedSizeListArray, Float64Array, ListArray, StringArray};
use arrow::ipc::reader::StreamReader;
use std::{io::Cursor, sync::Arc};

#[test]
fn empty_response_retains_distance_contract() {
    let bytes = encode_render_units(&[]).unwrap();
    let reader = StreamReader::try_new(Cursor::new(bytes), None).unwrap();
    for name in [
        "shape_key",
        "shape_positions",
        "shape_distances",
        "motion",
        "offset_m",
    ] {
        assert!(reader.schema().field_with_name(name).is_ok());
    }
    assert_eq!(reader.map(|b| b.unwrap().num_rows()).sum::<usize>(), 0);
}
#[test]
fn arrow_shares_geometry_and_motion_and_preserves_float64_coefficients() {
    let shape = build_shape_geometry(
        &geo::LineString::from(vec![(-74.0, 40.7), (-73.99, 40.7), (-73.99, 40.71)]),
        6538,
    )
    .unwrap()
    .path;
    let plan = Arc::new(Trajectory {
        generated_at: 100.0,
        trip_id: "trip".into(),
        route_id: "A".into(),
        color: [255, 0, 0],
        shape: shape.clone(),
        motion: vec![hermite(
            100.125,
            200.125,
            50.123456789,
            500.987654321,
            0.0,
            0.0,
        )],
        gps_fix: None,
    });
    let unit = RenderUnit {
        render_unit_id: "one".into(),
        source: Source::MtaSubway,
        trip_id: "trip".into(),
        route_id: "A".into(),
        icon_key: "rail_head".into(),
        unit_index: Some(0),
        unit_count: Some(2),
        is_head: true,
        length_m: 18.288,
        passengers: None,
        color: [255, 0, 0],
        offset_m: 9.144,
        trajectory: plan,
    };
    let mut second = unit.clone();
    second.render_unit_id = "two".into();
    second.offset_m += 18.288;
    let bytes = encode_render_units(&[unit, second]).unwrap();
    let batch = StreamReader::try_new(Cursor::new(bytes), None)
        .unwrap()
        .next()
        .unwrap()
        .unwrap();
    let lists = |name| {
        batch
            .column_by_name(name)
            .unwrap()
            .as_any()
            .downcast_ref::<ListArray>()
            .unwrap()
    };
    assert_eq!(
        lists("shape_positions").value_length(0),
        shape.positions.len() as i32
    );
    assert_eq!(lists("shape_positions").value_length(1), 0);
    assert_eq!(lists("motion").value_length(1), 0);
    let motion = lists("motion").value(0);
    let pairs = motion
        .as_any()
        .downcast_ref::<FixedSizeListArray>()
        .unwrap();
    let values = pairs
        .values()
        .as_any()
        .downcast_ref::<Float64Array>()
        .unwrap();
    assert_eq!(values.value(0), 100.125);
    assert_eq!(values.value(2), 50.123456789);
    let keys = batch
        .column_by_name("shape_key")
        .unwrap()
        .as_any()
        .downcast_ref::<StringArray>()
        .unwrap();
    assert_eq!(keys.value(0), keys.value(1));
    let offsets = batch
        .column_by_name("offset_m")
        .unwrap()
        .as_any()
        .downcast_ref::<Float64Array>()
        .unwrap();
    assert!((offsets.value(1) - offsets.value(0) - 18.288).abs() < 1e-7);
}
