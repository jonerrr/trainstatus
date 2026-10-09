use super::*;
use crate::feed::{TimeRange, TranslatedString, translated_string::Translation};

#[test]
fn mta_translation_language_and_format_are_shared_across_sections() {
    let id = Uuid::nil();
    let alert = GtfsAlert {
        header_text: Some(TranslatedString {
            translation: vec![
                Translation {
                    text: "plain".into(),
                    language: None,
                },
                Translation {
                    text: "<b>header</b>".into(),
                    language: Some("en-html".into()),
                },
            ],
        }),
        description_text: Some(TranslatedString {
            translation: vec![Translation {
                text: "<p>description</p>".into(),
                language: Some("es-html".into()),
            }],
        }),
        ..Default::default()
    };
    let translations = mta_translations(id, &alert);
    assert_eq!(translations.len(), 3);
    let expected = [
        (AlertSection::Header, AlertFormat::Plain, "en", "plain"),
        (
            AlertSection::Header,
            AlertFormat::Html,
            "en",
            "<b>header</b>",
        ),
        (
            AlertSection::Description,
            AlertFormat::Html,
            "es",
            "<p>description</p>",
        ),
    ];
    for (translation, (section, format, language, text)) in translations.iter().zip(expected) {
        assert_eq!(translation.alert_id, id);
        assert_eq!(translation.section, section);
        assert_eq!(translation.format, format);
        assert_eq!(translation.language, language);
        assert_eq!(translation.text, text);
    }
}

#[test]
fn periods_require_a_valid_start_and_preserve_open_ends() {
    let alert = GtfsAlert {
        active_period: vec![
            TimeRange {
                start: None,
                end: Some(200),
            },
            TimeRange {
                start: Some(i64::MAX as u64),
                end: None,
            },
            TimeRange {
                start: Some(100),
                end: None,
            },
            TimeRange {
                start: Some(200),
                end: Some(300),
            },
        ],
        ..Default::default()
    };
    let periods = active_periods(Uuid::nil(), &alert);
    assert_eq!(periods.len(), 2);
    assert_eq!(periods[0].start_time.timestamp(), 100);
    assert!(periods[0].end_time.is_none());
    assert_eq!(periods[1].start_time.timestamp(), 200);
    assert_eq!(periods[1].end_time.unwrap().timestamp(), 300);
}
