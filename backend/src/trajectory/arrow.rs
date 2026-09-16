use std::sync::Arc;

use arrow::array::{
    ArrayRef, BooleanBuilder, FixedSizeListBuilder, Float32Builder, Float64Builder, Int16Builder,
    Int32Builder, ListBuilder, StringBuilder, UInt8Builder,
};
use arrow::datatypes::{DataType, Field, Schema};
use arrow::ipc::writer::StreamWriter;
use arrow::record_batch::RecordBatch;

use super::types::RenderUnit;

pub fn encode_render_units(render_units: &[RenderUnit]) -> anyhow::Result<Vec<u8>> {
    let schema = render_unit_schema();

    let mut render_unit_id =
        StringBuilder::with_capacity(render_units.len(), render_units.len() * 48);
    let mut source = StringBuilder::with_capacity(render_units.len(), render_units.len() * 12);
    let mut trip_id = StringBuilder::with_capacity(render_units.len(), render_units.len() * 36);
    let mut route_id = StringBuilder::with_capacity(render_units.len(), render_units.len() * 8);
    let mut icon_key = StringBuilder::with_capacity(render_units.len(), render_units.len() * 12);
    let mut unit_index = Int16Builder::with_capacity(render_units.len());
    let mut unit_count = Int16Builder::with_capacity(render_units.len());
    let mut is_head = BooleanBuilder::with_capacity(render_units.len());
    let mut length_m = Float32Builder::with_capacity(render_units.len());
    let mut passengers = Int32Builder::with_capacity(render_units.len());

    let color_field = Arc::new(Field::new("item", DataType::UInt8, false));
    let mut color_builder =
        FixedSizeListBuilder::new(UInt8Builder::with_capacity(render_units.len() * 3), 3);
    color_builder = color_builder.with_field(color_field);

    let ts_field = Arc::new(Field::new("item", DataType::Float64, false));
    let mut ts_list_builder = ListBuilder::new(Float64Builder::new()).with_field(ts_field);

    let coord_field = Arc::new(Field::new("item", DataType::Float64, true));
    let pair_field = Arc::new(Field::new(
        "item",
        DataType::FixedSizeList(coord_field.clone(), 2),
        true,
    ));
    let mut pos_list_builder =
        ListBuilder::new(FixedSizeListBuilder::new(Float64Builder::new(), 2))
            .with_field(pair_field);

    let bearing_field = Arc::new(Field::new("item", DataType::Float32, false));
    let mut bearing_list_builder =
        ListBuilder::new(Float32Builder::new()).with_field(bearing_field);

    for unit in render_units {
        render_unit_id.append_value(&unit.render_unit_id);
        source.append_value(unit.source.as_str());
        trip_id.append_value(&unit.trip_id);
        route_id.append_value(&unit.route_id);
        icon_key.append_value(&unit.icon_key);

        match unit.unit_index {
            Some(value) => unit_index.append_value(value),
            None => unit_index.append_null(),
        }
        match unit.unit_count {
            Some(value) => unit_count.append_value(value),
            None => unit_count.append_null(),
        }

        is_head.append_value(unit.is_head);
        length_m.append_value(unit.length_m);

        match unit.passengers {
            Some(value) => passengers.append_value(value),
            None => passengers.append_null(),
        }

        let color = color_builder.values();
        color.append_value(unit.color[0]);
        color.append_value(unit.color[1]);
        color.append_value(unit.color[2]);
        color_builder.append(true);

        for &ts in &unit.timestamps {
            ts_list_builder.values().append_value(ts);
        }
        ts_list_builder.append(true);

        for [lon, lat] in &unit.positions {
            let pair = pos_list_builder.values();
            pair.values().append_value(*lon);
            pair.values().append_value(*lat);
            pair.append(true);
        }
        pos_list_builder.append(true);

        for &bearing in &unit.bearings {
            bearing_list_builder.values().append_value(bearing);
        }
        bearing_list_builder.append(true);
    }

    let batch = RecordBatch::try_new(
        schema.clone(),
        vec![
            Arc::new(render_unit_id.finish()) as ArrayRef,
            Arc::new(source.finish()),
            Arc::new(trip_id.finish()),
            Arc::new(route_id.finish()),
            Arc::new(icon_key.finish()),
            Arc::new(unit_index.finish()),
            Arc::new(unit_count.finish()),
            Arc::new(is_head.finish()),
            Arc::new(length_m.finish()),
            Arc::new(passengers.finish()),
            Arc::new(color_builder.finish()),
            Arc::new(ts_list_builder.finish()),
            Arc::new(pos_list_builder.finish()),
            Arc::new(bearing_list_builder.finish()),
        ],
    )?;

    let mut buf = Vec::new();
    let mut writer = StreamWriter::try_new(&mut buf, &schema)?;
    writer.write(&batch)?;
    writer.finish()?;
    Ok(buf)
}

fn render_unit_schema() -> Arc<Schema> {
    let coord_field = Arc::new(Field::new("item", DataType::Float64, true));
    Arc::new(Schema::new(vec![
        Field::new("render_unit_id", DataType::Utf8, false),
        Field::new("source", DataType::Utf8, false),
        Field::new("trip_id", DataType::Utf8, false),
        Field::new("route_id", DataType::Utf8, false),
        Field::new("icon_key", DataType::Utf8, false),
        Field::new("unit_index", DataType::Int16, true),
        Field::new("unit_count", DataType::Int16, true),
        Field::new("is_head", DataType::Boolean, false),
        Field::new("length_m", DataType::Float32, false),
        Field::new("passengers", DataType::Int32, true),
        Field::new(
            "color",
            DataType::FixedSizeList(Arc::new(Field::new("item", DataType::UInt8, false)), 3),
            false,
        ),
        Field::new(
            "timestamps",
            DataType::List(Arc::new(Field::new("item", DataType::Float64, false))),
            false,
        ),
        Field::new(
            "positions",
            DataType::List(Arc::new(Field::new(
                "item",
                DataType::FixedSizeList(coord_field, 2),
                true,
            ))),
            false,
        ),
        Field::new(
            "bearings",
            DataType::List(Arc::new(Field::new("item", DataType::Float32, false))),
            false,
        ),
    ]))
}
