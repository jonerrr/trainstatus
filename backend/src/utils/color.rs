/// Neutral grey used when a color can't be parsed. Mirrors the frontend
/// `FALLBACK_ROUTE_COLOR` so a normalized-away value looks the same everywhere.
pub const FALLBACK_ROUTE_COLOR: &str = "#8B95A1";

/// normalize a route color to canonical `#RRGGBB` (uppercase).
///
/// Accepts `#RRGGBB` or bare `RRGGBB` (any case). Anything else — empty,
/// wrong length, non-hex — collapses to [`FALLBACK_ROUTE_COLOR`].
pub fn normalize_hex_color(color: &str) -> String {
    let hex = color.trim().strip_prefix('#').unwrap_or(color.trim());
    if hex.len() == 6 && hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        format!("#{}", hex.to_ascii_uppercase())
    } else {
        FALLBACK_ROUTE_COLOR.to_string()
    }
}

#[cfg(test)]
mod tests {
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
}
