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
#[path = "tests/color.rs"]
mod tests;
