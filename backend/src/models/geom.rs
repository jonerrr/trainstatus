use geozero::{
    GeomProcessor, GeozeroGeometry,
    error::Result,
    wkb::{FromWkb, WkbDialect},
};
use serde::{Deserialize, Serialize};
use std::io::Read;

// TODO: define utoipa schema
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Geom(pub geo::Geometry<f64>);

// 2. Implement GeozeroGeometry (delegate to inner)
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

// 3. Implement FromWkb (delegate to inner)
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

// These macros implement sqlx::Type, sqlx::Decode, and sqlx::Encode
geozero::impl_sqlx_postgis_type_info!(Geom);
geozero::impl_sqlx_postgis_decode!(Geom);
geozero::impl_sqlx_postgis_encode!(Geom);
