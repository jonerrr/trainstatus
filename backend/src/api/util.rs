pub fn point_schema() -> utoipa::openapi::schema::Object {
    let point_coords = utoipa::openapi::schema::ObjectBuilder::new()
        .schema_type(utoipa::openapi::schema::Type::Object)
        .property(
            "x",
            utoipa::openapi::schema::ObjectBuilder::new()
                .schema_type(utoipa::openapi::schema::Type::Number)
                .format(Some(utoipa::openapi::SchemaFormat::KnownFormat(
                    utoipa::openapi::KnownFormat::Double,
                ))),
        )
        .property(
            "y",
            utoipa::openapi::schema::ObjectBuilder::new()
                .schema_type(utoipa::openapi::schema::Type::Number)
                .format(Some(utoipa::openapi::SchemaFormat::KnownFormat(
                    utoipa::openapi::KnownFormat::Double,
                ))),
        )
        .required("x")
        .required("y")
        .build();

    let coordinates = utoipa::openapi::schema::ObjectBuilder::new()
        .schema_type(utoipa::openapi::schema::Type::Object)
        .property("Point", point_coords)
        .required("Point")
        .build();

    utoipa::openapi::schema::ObjectBuilder::new()
        .schema_type(utoipa::openapi::schema::Type::Object)
        .property("coordinates", coordinates)
        .property(
            "type",
            utoipa::openapi::schema::ObjectBuilder::new()
                .schema_type(utoipa::openapi::schema::Type::String)
                .enum_values(Some(vec!["Point"])),
        )
        .required("coordinates")
        .required("type")
        .build()
}
