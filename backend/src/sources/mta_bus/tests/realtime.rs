use super::parse_bus_origin_time;

#[test]
fn origin_centiminutes_preserve_minutes_fraction_and_overnight_offset() {
    assert_eq!(
        parse_bus_origin_time("QV_A6-Weekday-SDon-070500_Q45_552"),
        Some(42_300)
    );
    assert_eq!(
        parse_bus_origin_time("QV_A6-Weekday-SDon-070550_Q45_552"),
        Some(42_330)
    );
    assert_eq!(
        parse_bus_origin_time("QV_A6-Weekday-SDon-162000_Q45_552"),
        Some(97_200)
    );
    assert_eq!(parse_bus_origin_time("nonstandard"), None);
    assert_eq!(parse_bus_origin_time("QV_invalid_Q45_552"), None);
}
