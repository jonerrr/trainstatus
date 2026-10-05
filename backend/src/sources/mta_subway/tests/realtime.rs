use super::normalize_subway_direction;

#[test]
fn normalizes_subway_directions_to_north_south() {
    assert_eq!(normalize_subway_direction("NORTH"), Some(1));
    assert_eq!(normalize_subway_direction("EAST"), Some(1));
    assert_eq!(normalize_subway_direction("SOUTH"), Some(3));
    assert_eq!(normalize_subway_direction("WEST"), Some(3));
    assert_eq!(normalize_subway_direction("UP"), None);
}
