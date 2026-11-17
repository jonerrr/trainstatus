use super::normalize_subway_direction;

#[test]
fn normalizes_subway_directions_to_north_south() {
    assert_eq!(normalize_subway_direction("NORTH"), Some(1));
    assert_eq!(normalize_subway_direction("EAST"), Some(1));
    assert_eq!(normalize_subway_direction("SOUTH"), Some(3));
    assert_eq!(normalize_subway_direction("WEST"), Some(3));
    assert_eq!(normalize_subway_direction("UP"), None);
}

#[test]
fn helium_origin_remains_wall_time_on_dst_transition_dates() {
    for (observed, expected) in [
        ("2026-03-08T05:30:00Z", "2026-03-08T05:30:00Z"),
        ("2026-11-01T04:30:00Z", "2026-11-01T04:30:00Z"),
    ] {
        let at = observed.parse().unwrap();
        assert_eq!(
            super::parse_created_at_from_trip_id("1W 0030 WHL/DIT", at),
            Some(expected.parse().unwrap()),
        );
    }
}
