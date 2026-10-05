use super::*;

#[test]
fn prefixes_bare_hex() {
    assert_eq!(normalize_hex_color("1a2b57"), "#1A2B57");
}

#[test]
fn keeps_prefixed_hex_and_uppercases() {
    assert_eq!(normalize_hex_color("#0039a6"), "#0039A6");
    assert_eq!(normalize_hex_color("#0039A6"), "#0039A6");
}

#[test]
fn trims_whitespace() {
    assert_eq!(normalize_hex_color("  ee352e  "), "#EE352E");
}

#[test]
fn falls_back_on_garbage() {
    assert_eq!(normalize_hex_color(""), FALLBACK_ROUTE_COLOR);
    assert_eq!(normalize_hex_color("red"), FALLBACK_ROUTE_COLOR);
    assert_eq!(normalize_hex_color("#12345"), FALLBACK_ROUTE_COLOR);
    assert_eq!(normalize_hex_color("gggggg"), FALLBACK_ROUTE_COLOR);
}
