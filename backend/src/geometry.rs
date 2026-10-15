use geozero::{
    GeomProcessor, GeozeroGeometry,
    error::Result,
    wkb::{FromWkb, WkbDialect},
};
use serde::{Deserialize, Serialize};
use std::io::Read;

#[derive(Debug, Clone, Serialize, Deserialize)]
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

impl Geom {
    /// Coordinate serialization is deterministic and independent of provider shape IDs.
    pub(crate) fn content_id(&self) -> anyhow::Result<String> {
        Ok(blake3::hash(&serde_json::to_vec(self)?)
            .to_hex()
            .to_string())
    }
}
