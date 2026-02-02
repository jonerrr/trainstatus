#[macro_export]
macro_rules! impl_discriminated_data {
    // Entry Point: We inject the identifier `raw_json` here so it is consistent
    (
        $enum_name:ident,
        $source_type:ty,
        { $($tokens:tt)* }
    ) => {
        impl_discriminated_data!(
            @parse_arms
            ($enum_name)
            ($source_type)
            (raw_json)    // <--- Key Fix: Pass the identifier for the JSON variable
            ($($tokens)*)
            ()
        );
    };

    // Case 1: Variant => Type (With Data), followed by comma
    (
        @parse_arms
        ($enum_name:ident)
        ($source_type:ty)
        ($json:ident) // <--- Receive identifier
        ($var:ident => $data:ty, $($rest:tt)*)
        ($($arms:tt)*)
    ) => {
        impl_discriminated_data!(
            @parse_arms
            ($enum_name)
            ($source_type)
            ($json) // <--- Pass identifier along
            ($($rest)*)
            (
                $($arms)*
                <$source_type>::$var => {
                    // Use $json instead of literal raw_json
                    let payload: $data = serde_json::from_str($json.0.get())
                        .map_err(|e| sqlx::Error::ColumnDecode {
                            index: "data".to_string(),
                            source: Box::new(e),
                        })?;
                    Ok($enum_name::$var(payload))
                },
            )
        );
    };

    // Case 2: Variant (Unit/No Data), followed by comma
    (
        @parse_arms
        ($enum_name:ident)
        ($source_type:ty)
        ($json:ident)
        ($var:ident, $($rest:tt)*)
        ($($arms:tt)*)
    ) => {
        impl_discriminated_data!(
            @parse_arms
            ($enum_name)
            ($source_type)
            ($json)
            ($($rest)*)
            (
                $($arms)*
                <$source_type>::$var => Ok($enum_name::$var),
            )
        );
    };

    // Case 3: Handle (Variant => Type) at the very end (no trailing comma)
    (
        @parse_arms
        ($enum_name:ident)
        ($source_type:ty)
        ($json:ident)
        ($var:ident => $data:ty)
        ($($arms:tt)*)
    ) => {
        impl_discriminated_data!(@parse_arms ($enum_name) ($source_type) ($json) ($var => $data,) ($($arms)*));
    };

    // Case 4: Handle (Variant) at the very end (no trailing comma)
    (
        @parse_arms
        ($enum_name:ident)
        ($source_type:ty)
        ($json:ident)
        ($var:ident)
        ($($arms:tt)*)
    ) => {
        impl_discriminated_data!(@parse_arms ($enum_name) ($source_type) ($json) ($var,) ($($arms)*));
    };

    // Base Case: Emit the implementation
    (
        @parse_arms
        ($enum_name:ident)
        ($source_type:ty)
        ($json:ident)
        ()
        ($($arms:tt)*)
    ) => {
        impl<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow> for $enum_name {
            fn from_row(row: &'r sqlx::postgres::PgRow) -> Result<Self, sqlx::Error> {
                use sqlx::Row;

                let source: $source_type = row.try_get("source")?;

                // We define the variable using the passed identifier $json
                let $json: sqlx::types::Json<Box<serde_json::value::RawValue>> = row.try_get("data")?;

                match source {
                    $($arms)*
                    _ => Err(sqlx::Error::Decode("Unknown or unsupported source variant".into())),
                }
            }
        }
    };
}
