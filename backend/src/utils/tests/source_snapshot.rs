use super::*;

#[test]
fn get_is_none_until_published() {
    let snap: SourceSnapshot<HashMap<String, String>> = SourceSnapshot::new();
    assert!(snap.get(Source::NjtBus).is_none());
}

#[test]
fn replace_publishes_and_is_isolated_per_source() {
    let snap: SourceSnapshot<HashMap<String, String>> = SourceSnapshot::new();
    let mut map = HashMap::new();
    map.insert("child".to_string(), "canonical".to_string());
    snap.replace(Source::NjtBus, map);

    let got = snap.get(Source::NjtBus).unwrap();
    assert_eq!(got.get("child").map(String::as_str), Some("canonical"));
    // Another source is unaffected.
    assert!(snap.get(Source::MtaBus).is_none());
}

#[test]
fn replace_swaps_the_whole_value() {
    let snap: SourceSnapshot<HashMap<String, String>> = SourceSnapshot::new();
    snap.replace(
        Source::NjtBus,
        HashMap::from([("a".to_string(), "1".to_string())]),
    );
    snap.replace(
        Source::NjtBus,
        HashMap::from([("b".to_string(), "2".to_string())]),
    );

    let got = snap.get(Source::NjtBus).unwrap();
    assert!(
        got.get("a").is_none(),
        "stale entry should be gone after swap"
    );
    assert_eq!(got.get("b").map(String::as_str), Some("2"));
}
