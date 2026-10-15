use crate::geometry::Geom;
use geozero::{GeozeroGeometry, wkb::FromWkb};
use sqlx::ValueRef;

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
