use super::cache::PlatformMatch;
use crate::models::stop::{PlatformDirection, PlatformEdge};
pub(super) fn platform_position_for_direction(
    position_from_north_m: f64,
    platform_length_m: f64,
    direction: i16,
) -> f64 {
    match direction {
        1 => position_from_north_m,
        3 => platform_length_m - position_from_north_m,
        _ => position_from_north_m,
    }
}

pub(super) fn select_platform(
    edges: &[PlatformEdge],
    direction: i16,
    consist_length_m: f64,
) -> Option<PlatformMatch> {
    if edges.is_empty() {
        return None;
    }
    let consist_length_ft = consist_length_m / 0.3048;
    let mut candidates: Vec<(PlatformEdge, f64, f64, u8)> = Vec::new();

    for edge in edges {
        let platform_length_m = edge.length_ft as f64 * 0.3048;
        for marker in &edge.car_markers {
            let matches_direction = match direction {
                1 => marker.direction == PlatformDirection::North,
                3 => marker.direction == PlatformDirection::South,
                _ => false,
            };
            if !matches_direction {
                continue;
            }
            let position_m = platform_position_for_direction(
                marker.position_ft as f64 * 0.3048,
                platform_length_m,
                direction,
            );
            let consist_match = marker
                .consist_length_ft
                .map(|len| ((len as f64) - consist_length_ft).abs() < 0.1)
                .unwrap_or(false);
            let rank = if consist_match { 0 } else { 1 };
            candidates.push((edge.clone(), position_m, platform_length_m, rank));
        }
    }

    candidates.sort_by(|a, b| {
        if a.3 != b.3 {
            a.3.cmp(&b.3)
        } else {
            let pos_a = (a.1 - consist_length_m).abs();
            let pos_b = (b.1 - consist_length_m).abs();
            pos_a
                .partial_cmp(&pos_b)
                .unwrap_or(std::cmp::Ordering::Equal)
        }
    });

    candidates
        .first()
        .map(|(edge, position_m, platform_length_m, _)| PlatformMatch {
            platform_edge_id: edge.id.clone(),
            position_m: *position_m,
            platform_edge_length_m: *platform_length_m,
        })
}
