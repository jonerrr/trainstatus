use geo::{Coord, Distance, Euclidean, LineString, Point};
use proj4rs::{Proj, transform::transform};

use crate::geometry::Geom;

/// Cached projected geometry for a trip shape.
#[derive(Clone)]
pub struct ShapeGeometry {
    pub wgs84_line: LineString<f64>,
    pub projected_line: LineString<f64>,
    pub cum_dist: Vec<f64>,
    pub length_m: f64,
    pub path: std::sync::Arc<super::types::TrajectoryShape>,
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
        path: std::sync::Arc::new(super::types::TrajectoryShape {
            key: shape_key_from_line(line),
            positions: line.0.iter().map(|c| [c.x, c.y]).collect(),
            distances_m: cum_dist.clone(),
        }),
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

#[cfg(test)]
pub fn distance_to_coord(
    distance_m: f64,
    line: &LineString,
    cum_dist: &[f64],
) -> Option<Coord<f64>> {
    let coords = &line.0;
    if coords.is_empty() || cum_dist.is_empty() || distance_m.is_nan() {
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

pub fn wgs84_to_epsg_projectors(epsg_code: u16) -> Option<(Proj, Proj)> {
    let proj_wgs84 = Proj::from_epsg_code(4326).ok()?;
    let proj_target = Proj::from_epsg_code(epsg_code).ok()?;
    Some((proj_wgs84, proj_target))
}

pub fn project_wgs84_point_to_epsg(point: &Point<f64>, epsg_code: u16) -> Option<Point<f64>> {
    let (proj_wgs84, proj_target) = wgs84_to_epsg_projectors(epsg_code)?;
    project_point_wgs84_to_epsg(point, &proj_wgs84, &proj_target)
}

pub fn project_point_wgs84_to_epsg(
    point: &Point<f64>,
    from: &Proj,
    to: &Proj,
) -> Option<Point<f64>> {
    let mut projected = Point::new(point.x().to_radians(), point.y().to_radians());
    transform(from, to, &mut projected).ok()?;
    Some(projected)
}

pub fn project_linestring_wgs84_to_epsg(
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

/// All nearby directed projections, deduplicated at shared vertices. Retaining
/// alternatives is essential at loops and parallel legs.
pub fn projection_candidates(point: &Point<f64>, geometry: &ShapeGeometry) -> Vec<(f64, f64)> {
    let mut candidates = Vec::new();
    for (i, pair) in geometry.projected_line.0.windows(2).enumerate() {
        let dx = pair[1].x - pair[0].x;
        let dy = pair[1].y - pair[0].y;
        let len2 = dx * dx + dy * dy;
        if len2 <= 1e-12 {
            continue;
        }
        let u =
            (((point.x() - pair[0].x) * dx + (point.y() - pair[0].y) * dy) / len2).clamp(0.0, 1.0);
        let error = (point.x() - pair[0].x - u * dx).hypot(point.y() - pair[0].y - u * dy);
        if error <= 100.0 {
            candidates.push((
                geometry.cum_dist[i] + u * (geometry.cum_dist[i + 1] - geometry.cum_dist[i]),
                error,
                i,
            ));
        }
    }
    let best = candidates.iter().map(|c| c.1).fold(f64::INFINITY, f64::min);
    candidates.retain(|c| c.1 <= best + 5.0);
    candidates.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut branches: Vec<((f64, f64), usize, f64)> = Vec::new();
    for (distance, error, segment) in candidates {
        if let Some((best, last_segment, last_distance)) = branches.last_mut()
            && segment == *last_segment + 1
            && distance - *last_distance <= 5.0
        {
            if error < best.1 {
                *best = (distance, error);
            }
            *last_segment = segment;
            *last_distance = distance;
        } else {
            branches.push(((distance, error), segment, distance));
        }
    }
    branches.into_iter().map(|(best, _, _)| best).collect()
}

/// Resolve the entire visit chain, allowing equal distances for colocated stops.
/// A projection is published only if all feasible directed traversals agree.
pub fn ordered_stop_distances(candidates: &[Vec<(f64, f64)>], ids: &[String]) -> Vec<Option<f64>> {
    let active: Vec<_> = candidates
        .iter()
        .enumerate()
        .filter(|(_, c)| !c.is_empty())
        .map(|(i, _)| i)
        .collect();
    if active.is_empty() {
        return vec![None; candidates.len()];
    }
    let compatible =
        |i: usize, a: f64, j: usize, b: f64| b >= a && (ids[i] != ids[j] || b > a + 1.0);
    let mut forward: Vec<Vec<bool>> = active
        .iter()
        .map(|i| vec![false; candidates[*i].len()])
        .collect();
    forward[0].fill(true);
    for row in 1..active.len() {
        let i = active[row - 1];
        let j = active[row];
        for (b, cb) in candidates[j].iter().enumerate() {
            forward[row][b] = candidates[i]
                .iter()
                .enumerate()
                .any(|(a, ca)| forward[row - 1][a] && compatible(i, ca.0, j, cb.0));
        }
    }
    let mut backward: Vec<Vec<bool>> = active
        .iter()
        .map(|i| vec![false; candidates[*i].len()])
        .collect();
    backward.last_mut().unwrap().fill(true);
    for row in (0..active.len() - 1).rev() {
        let i = active[row];
        let j = active[row + 1];
        for (a, ca) in candidates[i].iter().enumerate() {
            backward[row][a] = candidates[j]
                .iter()
                .enumerate()
                .any(|(b, cb)| backward[row + 1][b] && compatible(i, ca.0, j, cb.0));
        }
    }
    let mut out = vec![None; candidates.len()];
    for (row, &index) in active.iter().enumerate() {
        let viable: Vec<_> = candidates[index]
            .iter()
            .enumerate()
            .filter(|(i, _)| forward[row][*i] && backward[row][*i])
            .map(|(_, c)| c.0)
            .collect();
        if let (Some(first), Some(last)) = (viable.first(), viable.last())
            && last - first < 1.0
        {
            out[index] = Some(*first);
        }
    }
    out
}
