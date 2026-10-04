use geozero::{
    GeomProcessor, GeozeroGeometry,
    error::Result,
    wkb::{FromWkb, WkbDialect},
};
use serde::{Deserialize, Serialize};
use std::io::Read;
use utoipa::ToSchema;

// TODO: define utoipa schema
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[schema(value_type = Object)]
#[serde(transparent)]
pub struct Geom(pub geo::Geometry<f64>);

// Implement GeozeroGeometry (delegate to inner)
impl GeozeroGeometry for Geom {
    fn process_geom<P: GeomProcessor>(&self, processor: &mut P) -> Result<()> {
        self.0.process_geom(processor)
    }
    fn dims(&self) -> geozero::CoordDimensions {
        self.0.dims()
    }
    fn srid(&self) -> Option<i32> {
        // should always be 4326 for our use case
        self.0.srid()
    }
}

// Implement FromWkb (delegate to inner)
impl FromWkb for Geom {
    fn from_wkb<R: Read>(rdr: &mut R, dialect: WkbDialect) -> Result<Self> {
        let g = geo::Geometry::from_wkb(rdr, dialect)?;
        Ok(Geom(g))
    }
}

impl<T> From<T> for Geom
where
    T: Into<geo::Geometry<f64>>,
{
    fn from(x: T) -> Self {
        Geom(x.into())
    }
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
        let mut blob =
            <&[u8] as sqlx::Decode<sqlx::Postgres>>::decode(value)?;
        let geom = <Geom>::from_wkb(&mut blob, geozero::wkb::WkbDialect::Ewkb)
            .map_err(|e| sqlx::Error::Decode(e.to_string().into()))?;
        Ok(geom)
    }
}

impl sqlx::Encode<'_, sqlx::Postgres> for Geom {
    fn encode_by_ref(
        &self,
        buf: &mut sqlx::postgres::PgArgumentBuffer,
    ) -> std::result::Result<
        sqlx::encode::IsNull,
        Box<dyn std::error::Error + Send + Sync + 'static>,
    > {
        let mut wkb_out: Vec<u8> = Vec::new();
        let mut writer = geozero::wkb::WkbWriter::with_opts(
            &mut wkb_out,
            geozero::wkb::WkbDialect::Ewkb,
            self.dims(),
            self.srid(),
            Vec::new(),
        );
        self.process_geom(&mut writer)
            .expect("Failed to encode Geometry");
        buf.extend(&wkb_out);

        Ok(sqlx::encode::IsNull::No)
    }
}

