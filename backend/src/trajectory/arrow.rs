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

    let mut shape_key = StringBuilder::new();
    let mut offset_m = Float64Builder::new();
    let field = Arc::new(Field::new("item", DataType::Float64, false));
    let mut distances = ListBuilder::new(Float64Builder::new()).with_field(field);
    let mut motion = ListBuilder::new(FixedSizeListBuilder::new(Float64Builder::new(), 6));
    let mut shapes = std::collections::HashSet::new();
    let mut trips = std::collections::HashSet::new();

    let coord_field = Arc::new(Field::new("item", DataType::Float64, true));
    let pair_field = Arc::new(Field::new(
        "item",
        DataType::FixedSizeList(coord_field.clone(), 2),
        true,
    ));
    let mut pos_list_builder =
        ListBuilder::new(FixedSizeListBuilder::new(Float64Builder::new(), 2))
            .with_field(pair_field);

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

        let plan = &unit.trajectory;
        shape_key.append_value(&plan.shape.key);
        offset_m.append_value(unit.offset_m);
        if shapes.insert(plan.shape.key.clone()) {
            for [lon, lat] in &plan.shape.positions {
                let pair = pos_list_builder.values();
                pair.values().append_value(*lon);
                pair.values().append_value(*lat);
                pair.append(true);
            }
            for distance in &plan.shape.distances_m {
                distances.values().append_value(*distance);
            }
        }
        pos_list_builder.append(true);
        distances.append(true);
        if trips.insert((unit.source, unit.trip_id.clone())) {
            for interval in &plan.motion {
                for value in [
                    interval.t0,
                    interval.t1,
                    interval.a,
                    interval.b,
                    interval.c,
                    interval.d,
                ] {
                    motion.values().values().append_value(value);
                }
                motion.values().append(true);
            }
        }
        motion.append(true);
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
            Arc::new(shape_key.finish()),
            Arc::new(offset_m.finish()),
            Arc::new(pos_list_builder.finish()),
            Arc::new(distances.finish()),
            Arc::new(motion.finish()),
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
        Field::new("shape_key", DataType::Utf8, false),
        Field::new("offset_m", DataType::Float64, false),
        Field::new(
            "shape_positions",
            DataType::List(Arc::new(Field::new(
                "item",
                DataType::FixedSizeList(coord_field.clone(), 2),
                true,
            ))),
            false,
        ),
        Field::new(
            "shape_distances",
            DataType::List(Arc::new(Field::new("item", DataType::Float64, false))),
            false,
        ),
        Field::new(
            "motion",
            DataType::List(Arc::new(Field::new(
                "item",
                DataType::FixedSizeList(coord_field, 6),
                true,
            ))),
            false,
        ),
    ]))
}
