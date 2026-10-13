//! Shared scalar conversion used only at private PostgreSQL boundaries.
use crate::{geometry::Geom, source::Source};
use geozero::{GeozeroGeometry, wkb::FromWkb};
use serde::{Serialize, de::DeserializeOwned};

pub(crate) fn attributes(value: &impl Serialize) -> anyhow::Result<serde_json::Value> {
    let mut value = serde_json::to_value(value)?;
    if let Some(fields) = value.as_object_mut() {
        fields.remove("source");
    }
    Ok(value)
}
pub(crate) fn decode<T: DeserializeOwned>(
    source: Source,
    mut value: serde_json::Value,
) -> anyhow::Result<T> {
    value
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("Attributes must be an object"))?
        .insert(
            "source".into(),
            serde_json::Value::String(source.as_str().into()),
        );
    Ok(serde_json::from_value(value)?)
}

impl sqlx::Type<sqlx::Postgres> for Geom {
    fn type_info() -> sqlx::postgres::PgTypeInfo {
        sqlx::postgres::PgTypeInfo::with_name("geometry")
    }
}

impl sqlx::postgres::PgHasArrayType for Geom {
    fn array_type_info() -> sqlx::postgres::PgTypeInfo {
        sqlx::postgres::PgTypeInfo::with_name("_geometry")
    }
}

impl<'de> sqlx::Decode<'de, sqlx::Postgres> for Geom {
    fn decode(
        value: sqlx::postgres::PgValueRef<'de>,
    ) -> std::result::Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        use sqlx::ValueRef;
        if value.is_null() {
            return Err(Box::new(sqlx::Error::Decode(
                "Cannot decode NULL value".into(),
            )));
        }
        let mut blob = <&[u8] as sqlx::Decode<sqlx::Postgres>>::decode(value)?;
        let geom = <Geom>::from_wkb(&mut blob, geozero::wkb::WkbDialect::Ewkb)
            .map_err(|e| sqlx::Error::Decode(e.to_string().into()))?;
        Ok(geom)
    }
}

impl sqlx::Encode<'_, sqlx::Postgres> for Geom {
    fn encode_by_ref(
        &self,
        buf: &mut sqlx::postgres::PgArgumentBuffer,
    ) -> std::result::Result<sqlx::encode::IsNull, Box<dyn std::error::Error + Send + Sync + 'static>>
    {
        let mut wkb_out: Vec<u8> = Vec::new();
        let mut writer = geozero::wkb::WkbWriter::with_opts(
            &mut wkb_out,
            geozero::wkb::WkbDialect::Ewkb,
            self.dims(),
            self.srid(),
            Vec::new(),
        );
        self.process_geom(&mut writer)?;
        buf.extend(&wkb_out);

        Ok(sqlx::encode::IsNull::No)
    }
}
