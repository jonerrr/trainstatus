use super::{AlertId, RunId};
use crate::source::Source;
use chrono::{NaiveDate, TimeZone, Utc};

fn date() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 11, 13).unwrap()
}

#[test]
fn rerouting_short_turns_and_type_changes_preserve_subway_identity() {
    let original = RunId::mta_subway("07 0808 34H/MST", date()).unwrap();
    for id in ["$7 0808 34H/111", "=7 0808 34H/WPT", "/7 0808 34H/MST"] {
        assert_eq!(RunId::mta_subway(id, date()), Some(original));
    }
    for id in ["07 0808 WPT/MST", "07 0808+ 34H/MST", "01 0808 34H/MST"] {
        assert_ne!(RunId::mta_subway(id, date()), Some(original));
    }
    assert_ne!(
        RunId::mta_subway("07 0808 34H/MST", date().succ_opt().unwrap()),
        Some(original)
    );
}

#[test]
fn attached_half_minute_origin_and_whitespace_have_the_same_identity() {
    assert_eq!(
        RunId::mta_subway("0L 1941+RPY/8AV", date()),
        RunId::mta_subway(" /L   1941+  RPY/8AV ", date()),
    );
    for invalid in [
        "0L 2460 RPY/8AV",
        "0L 1941 RPY",
        "0L 1941 /8AV",
        "0L 1941 RPY/",
        "L 1941 RPY/8AV",
    ] {
        assert_eq!(RunId::mta_subway(invalid, date()), None);
    }
}

#[test]
fn scheduled_bus_identity_separates_sources_dates_and_provider_trips() {
    let id = RunId::scheduled_bus(Source::MtaBus, "trip-1", date());
    assert_ne!(id, RunId::scheduled_bus(Source::NjtBus, "trip-1", date()));
    assert_ne!(
        id,
        RunId::scheduled_bus(Source::MtaBus, "trip-1", date().succ_opt().unwrap())
    );
    assert_ne!(id, RunId::scheduled_bus(Source::MtaBus, "trip-2", date()));
    let encoded = serde_json::to_string(&id).unwrap();
    assert_eq!(serde_json::from_str::<RunId>(&encoded).unwrap(), id);
    assert!(serde_json::from_str::<RunId>("\"invalid\"").is_err());
}

#[test]
fn alert_occurrences_separate_creation_time_and_source() {
    let created = Utc.with_ymd_and_hms(2026, 11, 13, 10, 0, 0).unwrap();
    let id = AlertId::new(Source::MtaBus, "alert-1", created);
    assert_ne!(id, AlertId::new(Source::MtaSubway, "alert-1", created));
    assert_ne!(
        id,
        AlertId::new(
            Source::MtaBus,
            "alert-1",
            created + chrono::Duration::microseconds(1)
        )
    );
    let encoded = serde_json::to_string(&id).unwrap();
    assert_eq!(serde_json::from_str::<AlertId>(&encoded).unwrap(), id);
}

#[test]
fn binary_identifiers_survive_source_adts_and_reject_invalid_widths() {
    #[derive(Debug, PartialEq, serde::Serialize, serde::Deserialize)]
    #[serde(tag = "source", rename_all = "snake_case")]
    enum Record {
        MtaBus { run: RunId, alert: AlertId },
    }
    let record = Record::MtaBus {
        run: RunId::scheduled_bus(Source::MtaBus, "trip-1", date()),
        alert: AlertId::new(
            Source::MtaBus,
            "alert-1",
            Utc.with_ymd_and_hms(2026, 11, 13, 10, 0, 0).unwrap(),
        ),
    };
    let mut bytes = Vec::new();
    ciborium::ser::into_writer(&record, &mut bytes).unwrap();
    let recovered: Record = ciborium::de::from_reader(bytes.as_slice()).unwrap();
    assert_eq!(recovered, record);
    for width in [0, 31, 33] {
        let mut bytes = Vec::new();
        ciborium::ser::into_writer(serde_bytes::Bytes::new(&vec![0; width]), &mut bytes).unwrap();
        assert!(ciborium::de::from_reader::<RunId, _>(bytes.as_slice()).is_err());
    }
}
