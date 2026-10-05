use crate::models::source::Source;
use crate::trajectory::arrow::encode_render_units;
use crate::trajectory::types::{RenderUnit, compute_path_bbox};
use arrow::array::{Array, Float64Array, Int16Array, ListArray, StringArray};
use arrow::datatypes::DataType;
use arrow::ipc::reader::StreamReader;
use std::io::Cursor;

#[test]
fn encode_empty_round_trip_schema() {
    let bytes = encode_render_units(&[]).expect("encode");
    let reader = StreamReader::try_new(Cursor::new(bytes), None).expect("read IPC stream");
    let schema = reader.schema();
    assert_eq!(schema.fields().len(), 14);
    assert_eq!(
        schema.field_with_name("trip_id").unwrap().data_type(),
        &DataType::Utf8
    );
    assert!(schema.field_with_name("unit_count").unwrap().is_nullable());
    assert_eq!(
        reader.map(|batch| batch.unwrap().num_rows()).sum::<usize>(),
        0
    );
}

#[test]
fn encode_single_render_unit() {
    let unit = RenderUnit {
        render_unit_id: "mta_subway:t1:0".into(),
        source: Source::MtaSubway,
        trip_id: "t1".into(),
        route_id: "A".into(),
        icon_key: "rail_head".into(),
        unit_index: Some(0),
        unit_count: Some(1),
        is_head: true,
        length_m: 18.288,
        passengers: None,
        color: [255, 0, 0],
        positions: vec![[-74.0, 40.7], [-73.9, 40.8]],
        timestamps: vec![1000.0, 1010.0],
        bearings: vec![350.0, 10.0],
        path_bbox: compute_path_bbox(&[[-74.0, 40.7], [-73.9, 40.8]]),
    };
    let bytes = encode_render_units(&[unit]).expect("encode");
    let mut reader = StreamReader::try_new(Cursor::new(bytes), None).expect("read IPC stream");
    let batch = reader.next().unwrap().unwrap();
    assert_eq!(batch.num_rows(), 1);
    let text = |name| {
        batch
            .column_by_name(name)
            .unwrap()
            .as_any()
            .downcast_ref::<StringArray>()
            .unwrap()
            .value(0)
    };
    assert_eq!(text("render_unit_id"), "mta_subway:t1:0");
    assert_eq!(text("source"), "mta_subway");
    assert_eq!(text("route_id"), "A");
    assert_eq!(text("icon_key"), "rail_head");
    let count = batch
        .column_by_name("unit_count")
        .unwrap()
        .as_any()
        .downcast_ref::<Int16Array>()
        .unwrap();
    assert_eq!(count.value(0), 1);
    assert!(batch.column_by_name("passengers").unwrap().is_null(0));
    let times = batch
        .column_by_name("timestamps")
        .unwrap()
        .as_any()
        .downcast_ref::<ListArray>()
        .unwrap()
        .value(0);
    assert_eq!(
        times
            .as_any()
            .downcast_ref::<Float64Array>()
            .unwrap()
            .values()
            .as_ref(),
        &[1000.0, 1010.0]
    );
    assert!(reader.next().is_none());
}
