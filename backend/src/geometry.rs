use geozero::{GeomProcessor, GeozeroGeometry, error::Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

impl<T> From<T> for Geom
where
    T: Into<geo::Geometry<f64>>,
{
    fn from(x: T) -> Self {
        Geom(x.into())
    }
}

impl Geom {
    /// Cache weight follows decoded allocations, not the compressed representation.
    pub fn allocated_bytes(&self) -> usize {
        std::mem::size_of_val(self) + geometry_heap_bytes(&self.0)
    }
}

fn line_heap_bytes(line: &geo::LineString<f64>) -> usize {
    line.0.capacity() * std::mem::size_of::<geo::Coord<f64>>()
}

fn polygon_heap_bytes(polygon: &geo::Polygon<f64>) -> usize {
    line_heap_bytes(polygon.exterior())
        + polygon.interiors().len() * 2 * std::mem::size_of::<geo::LineString<f64>>()
        + polygon
            .interiors()
            .iter()
            .map(line_heap_bytes)
            .sum::<usize>()
}

fn geometry_heap_bytes(geometry: &geo::Geometry<f64>) -> usize {
    match geometry {
        geo::Geometry::LineString(line) => line_heap_bytes(line),
        geo::Geometry::Polygon(polygon) => polygon_heap_bytes(polygon),
        geo::Geometry::MultiPoint(points) => {
            points.0.capacity() * std::mem::size_of::<geo::Point<f64>>()
        }
        geo::Geometry::MultiLineString(lines) => {
            lines.0.capacity() * std::mem::size_of::<geo::LineString<f64>>()
                + lines.0.iter().map(line_heap_bytes).sum::<usize>()
        }
        geo::Geometry::MultiPolygon(polygons) => {
            polygons.0.capacity() * std::mem::size_of::<geo::Polygon<f64>>()
                + polygons.0.iter().map(polygon_heap_bytes).sum::<usize>()
        }
        geo::Geometry::GeometryCollection(collection) => {
            collection.0.capacity() * std::mem::size_of::<geo::Geometry<f64>>()
                + collection.0.iter().map(geometry_heap_bytes).sum::<usize>()
        }
        geo::Geometry::Point(_)
        | geo::Geometry::Line(_)
        | geo::Geometry::Rect(_)
        | geo::Geometry::Triangle(_) => 0,
    }
}
