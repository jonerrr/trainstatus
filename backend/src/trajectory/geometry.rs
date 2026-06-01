use geo::{Coord, Distance, Euclidean, LineString, Point};
use proj4rs::{Proj, transform::transform};

use crate::models::geom::Geom;

/// Cached projected geometry for a trip shape.
#[derive(Clone)]
pub struct ShapeGeometry {
    pub wgs84_line: LineString<f64>,
    pub projected_line: LineString<f64>,
    pub cum_dist: Vec<f64>,
    pub length_m: f64,
}

pub fn shape_key_from_line(line: &LineString<f64>) -> String {
    let mut hasher = blake3::Hasher::new();
    for c in &line.0 {
        hasher.update(&c.x.to_bits().to_le_bytes());
        hasher.update(&c.y.to_bits().to_le_bytes());
    }
    hasher.finalize().to_hex().to_string()
}

pub fn route_length_m(line: &LineString<f64>, epsg_code: u16) -> Option<f64> {
    let geom = build_shape_geometry(line, epsg_code)?;
    Some(geom.length_m)
}

pub fn build_shape_geometry(line: &LineString<f64>, epsg_code: u16) -> Option<ShapeGeometry> {
    let proj_wgs84 = Proj::from_epsg_code(4326).ok()?;
    let proj_target = Proj::from_epsg_code(epsg_code).ok()?;
    let projected_line = project_linestring_wgs84_to_epsg(line, &proj_wgs84, &proj_target)?;
    let cum_dist = cumulative_distances(&projected_line);
    let length_m = *cum_dist.last().unwrap_or(&0.0);
    if length_m <= 0.0 {
        return None;
    }
    Some(ShapeGeometry {
        wgs84_line: line.clone(),
        projected_line,
        cum_dist,
        length_m,
    })
}

pub fn cumulative_distances(line: &LineString) -> Vec<f64> {
    let coords = &line.0;
    let mut cum = Vec::with_capacity(coords.len());
    cum.push(0.0);
    for i in 1..coords.len() {
        let p1 = Point::new(coords[i - 1].x, coords[i - 1].y);
        let p2 = Point::new(coords[i].x, coords[i].y);
        let dist = Euclidean.distance(&p1, &p2);
        cum.push(cum[i - 1] + dist);
    }
    cum
}

pub fn distance_to_coord(
    distance_m: f64,
    line: &LineString,
    cum_dist: &[f64],
) -> Option<Coord<f64>> {
    let coords = &line.0;
    if coords.is_empty() || cum_dist.is_empty() {
        return None;
    }
    let total = *cum_dist.last().unwrap();
    if distance_m <= 0.0 {
        return Some(coords[0]);
    }
    if distance_m >= total {
        return Some(*coords.last().unwrap());
    }
    let seg = match cum_dist.binary_search_by(|d| d.partial_cmp(&distance_m).unwrap()) {
        Ok(i) => return Some(coords[i]),
        Err(i) => (i - 1).min(coords.len() - 2),
    };
    let seg_start = cum_dist[seg];
    let seg_end = cum_dist[seg + 1];
    let seg_len = seg_end - seg_start;
    if seg_len < 1e-12 {
        return Some(coords[seg]);
    }
    let frac = (distance_m - seg_start) / seg_len;
    let c0 = coords[seg];
    let c1 = coords[seg + 1];
    Some(Coord {
        x: c0.x + frac * (c1.x - c0.x),
        y: c0.y + frac * (c1.y - c0.y),
    })
}

pub fn bearing_at_distance(distance_m: f64, line: &LineString, cum_dist: &[f64]) -> Option<f32> {
    let total = *cum_dist.last()?;
    if total <= 0.0 {
        return None;
    }

    let delta = 2.0_f64.min(total / 4.0).max(0.25);
    let start = (distance_m - delta).clamp(0.0, total);
    let end = (distance_m + delta).clamp(0.0, total);
    if (end - start).abs() < 1e-9 {
        return None;
    }

    let from = distance_to_coord(start, line, cum_dist)?;
    let to = distance_to_coord(end, line, cum_dist)?;
    compass_bearing_deg(from, to).map(|bearing| bearing as f32)
}

pub fn project_point_onto_line(
    point: &Point<f64>,
    line: &LineString,
    cum_dist: &[f64],
) -> Option<f64> {
    let coords = &line.0;
    if coords.len() < 2 {
        return None;
    }
    let mut best_dist = f64::INFINITY;
    let mut best_cum_dist = 0.0;
    for i in 0..coords.len() - 1 {
        let a = coords[i];
        let b = coords[i + 1];
        let ax = point.x() - a.x;
        let ay = point.y() - a.y;
        let bx = b.x - a.x;
        let by = b.y - a.y;
        let dot = ax * bx + ay * by;
        let len_sq = bx * bx + by * by;
        let t = if len_sq < 1e-12 {
            0.0
        } else {
            (dot / len_sq).clamp(0.0, 1.0)
        };
        let proj = Coord {
            x: a.x + t * bx,
            y: a.y + t * by,
        };
        let dist = Euclidean.distance(point, &Point::new(proj.x, proj.y));
        if dist < best_dist {
            best_dist = dist;
            let seg_len = cum_dist[i + 1] - cum_dist[i];
            best_cum_dist = cum_dist[i] + t * seg_len;
        }
    }
    if best_dist.is_finite() {
        Some(best_cum_dist)
    } else {
        None
    }
}

pub fn parse_color(hex: &str) -> [u8; 3] {
    let hex = hex.trim_start_matches('#');
    if hex.len() >= 6 {
        let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(128);
        let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(128);
        let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(128);
        [r, g, b]
    } else {
        [128, 128, 128]
    }
}

fn compass_bearing_deg(from: Coord<f64>, to: Coord<f64>) -> Option<f64> {
    let lat1 = from.y.to_radians();
    let lat2 = to.y.to_radians();
    let dlon = (to.x - from.x).to_radians();

    let y = dlon.sin() * lat2.cos();
    let x = lat1.cos() * lat2.sin() - lat1.sin() * lat2.cos() * dlon.cos();
    if x.abs() < 1e-12 && y.abs() < 1e-12 {
        return None;
    }

    let theta = y.atan2(x).to_degrees();
    Some(theta.rem_euclid(360.0))
}

pub fn project_wgs84_point_to_epsg(point: &Point<f64>, epsg_code: u16) -> Option<Point<f64>> {
    let proj_wgs84 = Proj::from_epsg_code(4326).ok()?;
    let proj_target = Proj::from_epsg_code(epsg_code).ok()?;
    project_point_wgs84_to_epsg(point, &proj_wgs84, &proj_target)
}

fn project_point_wgs84_to_epsg(point: &Point<f64>, from: &Proj, to: &Proj) -> Option<Point<f64>> {
    let mut projected = Point::new(point.x().to_radians(), point.y().to_radians());
    transform(from, to, &mut projected).ok()?;
    Some(projected)
}

fn project_linestring_wgs84_to_epsg(
    line: &LineString,
    from: &Proj,
    to: &Proj,
) -> Option<LineString> {
    let mut coords = Vec::with_capacity(line.0.len());
    for coord in &line.0 {
        let point = Point::new(coord.x, coord.y);
        let projected = project_point_wgs84_to_epsg(&point, from, to)?;
        coords.push(Coord {
            x: projected.x(),
            y: projected.y(),
        });
    }
    Some(LineString::new(coords))
}

pub fn geom_to_linestring(geom: &Geom) -> Option<LineString<f64>> {
    match &geom.0 {
        geo::Geometry::LineString(ls) => Some(ls.clone()),
        _ => None,
    }
}
