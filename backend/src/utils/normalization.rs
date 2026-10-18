/// Trim and uppercase an ID for case-insensitive feed matching.
pub fn normalize_id(id: &str) -> String {
    id.trim().to_uppercase()
}

/// Trim leading/trailing whitespace and collapse internal whitespace runs.
pub fn normalize_whitespace(value: &str) -> String {
    let mut normalized = String::with_capacity(value.len());
    for token in value.split_whitespace() {
        if !normalized.is_empty() {
            normalized.push(' ');
        }
        normalized.push_str(token);
    }
    normalized
}

/// Format transit labels without expanding abbreviations or changing punctuation.
/// Casing is decided per word, so a lowercase `at` or `via` cannot preserve an
/// otherwise uppercase label. Known spellings take precedence over generic casing.
pub fn normalize_title(value: &str) -> String {
    let value = normalize_whitespace(value);
    let mut output = String::with_capacity(value.len());
    let mut start = None;
    let mut first_word = true;
    let mut apostrophe_suffix = false;
    for (index, character) in value.char_indices() {
        if character.is_alphanumeric() {
            start.get_or_insert(index);
        } else {
            if let Some(start) = start.take() {
                push_word(
                    &mut output,
                    &value[start..index],
                    first_word,
                    apostrophe_suffix,
                );
                first_word = false;
            }
            output.push(character);
            apostrophe_suffix = matches!(character, '\'' | '’')
                && value[..index]
                    .chars()
                    .next_back()
                    .is_some_and(char::is_alphabetic);
        }
    }
    if let Some(start) = start {
        push_word(&mut output, &value[start..], first_word, apostrophe_suffix);
    }
    output
}

fn push_word(output: &mut String, word: &str, first: bool, apostrophe_suffix: bool) {
    let lower = word.to_lowercase();
    if apostrophe_suffix && matches!(lower.as_str(), "s" | "t" | "d" | "ll" | "re" | "ve" | "m") {
        output.push_str(&lower);
        return;
    }
    if let Some(spelling) = known_spelling(&lower) {
        output.push_str(spelling);
    } else if word.chars().any(|character| character.is_ascii_digit()) {
        let digits = word
            .find(|character: char| !character.is_ascii_digit())
            .unwrap_or(word.len());
        if digits > 0 && matches!(&lower[digits..], "st" | "nd" | "rd" | "th") {
            output.push_str(&lower);
        } else {
            // Route branches and gates such as 119P, AC4 and 12A are identifiers.
            output.push_str(&word.to_uppercase());
        }
    } else if !first
        && matches!(
            lower.as_str(),
            "at" | "via" | "of" | "and" | "to" | "in" | "on" | "the"
        )
    {
        output.push_str(&lower);
    } else if word.chars().any(char::is_lowercase) && word.chars().skip(1).any(char::is_uppercase) {
        // Retain supplied internal casing, e.g. McCarthy or RiverLINE.
        output.push_str(word);
    } else {
        let mut characters = lower.chars();
        if let Some(first) = characters.next() {
            output.extend(first.to_uppercase());
            output.extend(characters);
        }
    }
}

/// Bounded spellings for observed transit labels, not rules for all personal names.
fn known_spelling(lower: &str) -> Option<&'static str> {
    Some(match lower {
        "path" => "PATH",
        "njt" => "NJT",
        "jfk" => "JFK",
        "wrtc" => "WRTC",
        "njsp" => "NJSP",
        "rwj" => "RWJ",
        "nlr" => "NLR",
        "gw" => "GW",
        "pabt" => "PABT",
        "ymca" => "YMCA",
        "ups" => "UPS",
        "cc" => "CC",
        "us" => "US",
        "rt" => "RT",
        "n" => "N",
        "s" => "S",
        "e" => "E",
        "w" => "W",
        "ne" => "NE",
        "nw" => "NW",
        "se" => "SE",
        "sw" => "SW",
        "ikea" => "IKEA",
        "fedex" => "FedEx",
        "mckinley" => "McKinley",
        "mcbride" => "McBride",
        _ => return None,
    })
}

#[cfg(test)]
#[path = "tests/normalization.rs"]
mod tests;
