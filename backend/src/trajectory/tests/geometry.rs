use crate::trajectory::types::{bbox_intersects, compute_path_bbox};

#[test]
fn bbox_intersection() {
    let trip = [0.0, 0.0, 10.0, 10.0];
    let query = [5.0, 5.0, 15.0, 15.0];
    assert!(bbox_intersects(trip, query));
    let miss = [-10.0, -10.0, -1.0, -1.0];
    assert!(!bbox_intersects(trip, miss));
}

#[test]
fn path_bbox_from_coords() {
    let path = vec![[-74.0, 40.7], [-73.9, 40.8]];
    let b = compute_path_bbox(&path);
    assert!((b[0] - (-74.0)).abs() < 1e-9);
    assert!((b[2] - (-73.9)).abs() < 1e-9);
}
