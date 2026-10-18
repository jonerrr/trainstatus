use super::normalize_title;

#[test]
fn transit_labels_preserve_acronyms_names_and_identifiers() {
    for (input, expected) in [
        ("US-30 at HADDON AVE", "US-30 at Haddon Ave"),
        ("US-30 AT HADDON AVE", "US-30 at Haddon Ave"),
        ("RT-40/322 100'W OF NOAHS RD", "RT-40/322 100'W of Noahs Rd"),
        ("BAYWAY AVE AT MCKINLEY ST", "Bayway Ave at McKinley St"),
        ("MCBRIDE AVE AT DECKER RD", "McBride Ave at Decker Rd"),
        ("30TH ST AT JFK BLVD", "30th St at JFK Blvd"),
        ("PATH STATION", "PATH Station"),
        ("CAMDEN WRTC", "Camden WRTC"),
        ("FAIRVIEW NJT GARAGE", "Fairview NJT Garage"),
        ("HAMILTON SQ RWJ HOSPITAL", "Hamilton Sq RWJ Hospital"),
        (
            "NEWARK PENN VIA AIRPORT IKEA & FEDEX",
            "Newark Penn via Airport IKEA & FedEx",
        ),
        (
            "119P JERSEY CITY JFK BLVD & PAMRAPO",
            "119P Jersey City JFK Blvd & Pamrapo",
        ),
        ("772G VIA I-95 & US-1", "772G via I-95 & US-1"),
        ("1ST / 2ND / 3RD / 4TH AVE", "1st / 2nd / 3rd / 4th Ave"),
        ("TERMINAL A TO TERMINALS B-C", "Terminal A to Terminals B-C"),
        ("NORTH WING GATE 12A", "North Wing Gate 12A"),
        (
            "O'CONNOR ST AT HARRAH'S CASINO",
            "O'Connor St at Harrah's Casino",
        ),
        ("  AT\tTHE HUB\nVIA MAIN ST  ", "At the Hub via Main St"),
        (
            "McKinley Ave at DeCamp Plaza",
            "McKinley Ave at DeCamp Plaza",
        ),
        ("ÉCOLE AVE", "École Ave"),
        ("", ""),
    ] {
        assert_eq!(normalize_title(input), expected, "{input}");
        assert_eq!(normalize_title(expected), expected, "idempotent: {input}");
    }
}
