use geo::{LineString, Point};
use proj4rs::Proj;
use std::sync::{Arc, OnceLock};

use crate::trajectory::geometry::{project_linestring_wgs84_to_epsg, project_point_wgs84_to_epsg};

static PROJECTORS_6538: OnceLock<Option<Arc<(Proj, Proj)>>> = OnceLock::new();

fn projectors_6538() -> Option<&'static (Proj, Proj)> {
    let cached = PROJECTORS_6538.get_or_init(|| {
        let wgs84 = Proj::from_epsg_code(4326).ok()?;
        let target = Proj::from_epsg_code(6538).ok()?;
        Some(Arc::new((wgs84, target)))
    });
    cached.as_ref().map(|arc| arc.as_ref())
}

pub fn project_stop(point: &Point<f64>) -> Option<Point<f64>> {
    let (from, to) = projectors_6538()?;
    project_point_wgs84_to_epsg(point, from, to)
}

pub fn project_line(line: &LineString<f64>) -> Option<LineString<f64>> {
    let (from, to) = projectors_6538()?;
    project_linestring_wgs84_to_epsg(line, from, to)
}
