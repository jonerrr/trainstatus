use std::io::Write;

use serde::{Deserialize, Serialize};

use crate::storage::format::frame_header_seal;
use crate::storage::format::{
    FRAME_HEADER_LEN, FRAME_MAGIC, JOURNAL_FORMAT_VERSION, JOURNAL_HEADER_LEN, JOURNAL_MAGIC,
};
use crate::storage::journal::{
    Journal, JournalOpenOptions, JournalPoisoned, JournalRecordId, JournalWatermark,
};
use crate::storage::tests::support::TempDir;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Event {
    n: u64,
    text: String,
}

#[test]
fn journal_empty_open_append_replay() {
    let dir = TempDir::new();
    let path = dir.path().join("events.tsjl");

    let (mut journal, replayed) = Journal::<Event>::open(&path).unwrap();
    assert!(replayed.is_empty());
    assert_eq!(journal.generation(), 0);
    assert_eq!(journal.next_sequence(), 0);

    journal
        .append(&Event {
            n: 1,
            text: "alpha".into(),
        })
        .unwrap();
    journal
        .append(&Event {
            n: 2,
            text: "beta".into(),
        })
        .unwrap();

    let (_, replayed) = Journal::<Event>::open(&path).unwrap();
    assert_eq!(
        replayed,
        vec![
            Event {
                n: 1,
                text: "alpha".into()
            },
            Event {
                n: 2,
                text: "beta".into()
            },
        ]
    );
}

#[test]
fn journal_truncates_incomplete_tail_on_open() {
    let dir = TempDir::new();
    let path = dir.path().join("events.tsjl");

    {
        let (mut journal, _) = Journal::<Event>::open(&path).unwrap();
        journal
            .append(&Event {
                n: 10,
                text: "ok".into(),
            })
            .unwrap();
    }

    let durable_end = std::fs::metadata(&path).unwrap().len();
    let mut tail = std::fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .unwrap();
    tail.write_all(&FRAME_MAGIC).unwrap();
    tail.write_all(&0u64.to_le_bytes()).unwrap();
    tail.write_all(&0u64.to_le_bytes()).unwrap();
    tail.write_all(&4u32.to_le_bytes()).unwrap();
    let _ = tail.sync_all();

    let (_, replayed) = Journal::<Event>::open(&path).unwrap();
    assert_eq!(
        replayed,
        vec![Event {
            n: 10,
            text: "ok".into()
        }]
    );
    assert_eq!(std::fs::metadata(&path).unwrap().len(), durable_end);
}

#[test]
fn journal_rejects_corruption_in_established_frame() {
    let dir = TempDir::new();
    let path = dir.path().join("events.tsjl");

    {
        let (mut journal, _) = Journal::<Event>::open(&path).unwrap();
        journal
            .append(&Event {
                n: 1,
                text: "first".into(),
            })
            .unwrap();
        journal
            .append(&Event {
                n: 2,
                text: "second".into(),
            })
            .unwrap();
    }

    let bytes = std::fs::read(&path).unwrap();
    let second_frame_start = JOURNAL_HEADER_LEN + frame_len_at(&bytes, JOURNAL_HEADER_LEN);
    let mut bytes = bytes;
    let checksum_offset = second_frame_start + 24;
    bytes[checksum_offset] ^= 0xff;
    std::fs::write(&path, &bytes).unwrap();

    assert!(Journal::<Event>::open(&path).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
}

#[test]
fn journal_reset_increments_generation_and_clears_sequence() {
    let dir = TempDir::new();
    let path = dir.path().join("events.tsjl");

    {
        let (mut journal, _) = Journal::<Event>::open(&path).unwrap();
        assert_eq!(journal.generation(), 0);
        journal
            .append(&Event {
                n: 1,
                text: "before".into(),
            })
            .unwrap();
        assert_eq!(
            journal.next_record_id(),
            JournalRecordId {
                generation: 0,
                sequence: 1
            }
        );
        journal.reset().unwrap();
        assert_eq!(journal.generation(), 1);
        assert_eq!(journal.next_sequence(), 0);
        journal
            .append(&Event {
                n: 99,
                text: "after".into(),
            })
            .unwrap();
    }

    let (_, replayed) = Journal::<Event>::open(&path).unwrap();
    assert_eq!(
        replayed,
        vec![Event {
            n: 99,
            text: "after".into()
        }]
    );

    let header = std::fs::read(&path).unwrap();
    assert_eq!(&header[..4], &JOURNAL_MAGIC);
    assert_eq!(
        u32::from_le_bytes(header[4..8].try_into().unwrap()),
        JOURNAL_FORMAT_VERSION
    );
    assert_eq!(
        u64::from_le_bytes(header[8..16].try_into().unwrap()),
        1,
        "file generation after one reset"
    );
}

#[test]
fn journal_rejects_truncated_file_header() {
    let dir = TempDir::new();
    let path = dir.path().join("events.tsjl");
    std::fs::write(&path, &JOURNAL_MAGIC[..3]).unwrap();
    assert!(Journal::<Event>::open(&path).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), &JOURNAL_MAGIC[..3]);
}

#[test]
fn journal_rejects_bad_file_magic() {
    let dir = TempDir::new();
    let path = dir.path().join("events.tsjl");
    let (journal, _) = Journal::<Event>::open(&path).unwrap();
    drop(journal);
    let mut bytes = std::fs::read(&path).unwrap();
    bytes[..4].copy_from_slice(b"XXXX");
    std::fs::write(&path, &bytes).unwrap();
    assert!(Journal::<Event>::open(&path).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
}

#[test]
fn journal_rejects_unsupported_format_version() {
    let dir = TempDir::new();
    let path = dir.path().join("events.tsjl");
    let mut header = Vec::with_capacity(JOURNAL_HEADER_LEN);
    header.extend_from_slice(&JOURNAL_MAGIC);
    header.extend_from_slice(&99u32.to_le_bytes());
    header.extend_from_slice(&0u64.to_le_bytes());
    std::fs::write(&path, &header).unwrap();
    assert!(Journal::<Event>::open(&path).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), header);
}

#[test]
fn journal_rejects_frame_generation_mismatch() {
    let dir = TempDir::new();
    let path = dir.path().join("events.tsjl");

    {
        let (mut journal, _) = Journal::<Event>::open(&path).unwrap();
        journal
            .append(&Event {
                n: 1,
                text: "only".into(),
            })
            .unwrap();
    }

    let mut bytes = std::fs::read(&path).unwrap();
    let frame_start = JOURNAL_HEADER_LEN;
    bytes[frame_start + 4..frame_start + 12].copy_from_slice(&99u64.to_le_bytes());
    let payload_len = u32::from_le_bytes(
        bytes[frame_start + 20..frame_start + 24]
            .try_into()
            .unwrap(),
    );
    bytes[frame_start + 28..frame_start + 32]
        .copy_from_slice(&frame_header_seal(99, 0, payload_len).to_le_bytes());
    std::fs::write(&path, &bytes).unwrap();

    assert!(Journal::<Event>::open(&path).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
}

#[test]
fn journal_rejects_oversized_declared_payload() {
    let dir = TempDir::new();
    let path = dir.path().join("events.tsjl");

    {
        let (mut journal, _) = Journal::<Event>::open(&path).unwrap();
        journal
            .append(&Event {
                n: 1,
                text: "x".into(),
            })
            .unwrap();
    }

    let mut bytes = std::fs::read(&path).unwrap();
    let frame_start = JOURNAL_HEADER_LEN;
    let payload_len = crate::storage::format::MAX_FRAME_PAYLOAD as u32 + 1;
    bytes[frame_start + 20..frame_start + 24].copy_from_slice(&payload_len.to_le_bytes());
    bytes[frame_start + 28..frame_start + 32]
        .copy_from_slice(&frame_header_seal(0, 0, payload_len).to_le_bytes());
    std::fs::write(&path, &bytes).unwrap();

    assert!(Journal::<Event>::open(&path).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
}

#[test]
fn journal_streaming_replay_matches_open() {
    let dir = TempDir::new();
    let path = dir.path().join("events.tsjl");

    {
        let (mut journal, _) = Journal::<Event>::open(&path).unwrap();
        for n in 0..5 {
            journal
                .append(&Event {
                    n,
                    text: format!("e{n}"),
                })
                .unwrap();
        }
    }

    let (_, from_open) = Journal::<Event>::open(&path).unwrap();
    let mut from_stream = Vec::new();
    Journal::<Event>::replay_records(&path, |_id, payload| {
        let event: Event = ciborium::de::from_reader(payload).unwrap();
        from_stream.push(event);
        Ok(())
    })
    .unwrap();
    assert_eq!(from_open, from_stream);
}

#[test]
fn journal_poisoned_after_append_write_failure() {
    let dir = TempDir::new();
    let path = dir.path().join("events.tsjl");

    let (mut journal, _) = Journal::<Event>::open(&path).unwrap();
    journal
        .append(&Event {
            n: 1,
            text: "durable".into(),
        })
        .unwrap();

    // Replace only this instance's descriptor with a real read-only file.
    // chmod cannot revoke write access on an already-open writable descriptor.
    journal.file = std::fs::File::open(&path).unwrap();

    assert!(
        journal
            .append(&Event {
                n: 2,
                text: "blocked".into(),
            })
            .is_err()
    );
    assert!(journal.is_poisoned());

    let poison_err = journal
        .append(&Event {
            n: 3,
            text: "nope".into(),
        })
        .unwrap_err();
    assert!(poison_err.downcast_ref::<JournalPoisoned>().is_some());
    assert!(
        journal
            .reset()
            .unwrap_err()
            .downcast_ref::<JournalPoisoned>()
            .is_some()
    );
    drop(journal);
    let (mut reopened, records) = Journal::<Event>::open(&path).unwrap();
    assert_eq!(
        records,
        vec![Event {
            n: 1,
            text: "durable".into()
        }]
    );
    reopened
        .append(&Event {
            n: 4,
            text: "recovered".into(),
        })
        .unwrap();
    let (_, records) = Journal::<Event>::open(&path).unwrap();
    assert_eq!(
        records,
        vec![
            Event {
                n: 1,
                text: "durable".into()
            },
            Event {
                n: 4,
                text: "recovered".into()
            },
        ]
    );
}

#[test]
#[cfg(unix)]
fn journal_poisoned_after_append_sync_failure() {
    use std::io::Read;
    use std::os::fd::OwnedFd;
    use std::os::unix::net::UnixStream;
    use std::time::Duration;

    let dir = TempDir::new();
    let path = dir.path().join("events.tsjl");
    let (mut journal, _) = Journal::<Event>::open(&path).unwrap();
    journal
        .append(&Event {
            n: 1,
            text: "durable".into(),
        })
        .unwrap();
    let before = std::fs::read(&path).unwrap();

    // A socket descriptor accepts the frame write but rejects File::sync_all.
    // Inject it into this private instance rather than changing production I/O
    // or relying on permissions to invalidate an already-open descriptor.
    let (writer, mut reader) = UnixStream::pair().unwrap();
    reader
        .set_read_timeout(Some(Duration::from_secs(1)))
        .unwrap();
    let descriptor: OwnedFd = writer.into();
    journal.file = descriptor.into();
    assert!(
        journal
            .append(&Event {
                n: 2,
                text: "uncertain".into()
            })
            .is_err()
    );
    let mut written_magic = [0; 4];
    reader.read_exact(&mut written_magic).unwrap();
    assert_eq!(written_magic, FRAME_MAGIC);
    assert!(journal.is_poisoned());
    assert!(
        journal
            .append(&Event {
                n: 3,
                text: "blocked".into()
            })
            .unwrap_err()
            .downcast_ref::<JournalPoisoned>()
            .is_some()
    );
    assert!(
        journal
            .reset()
            .unwrap_err()
            .downcast_ref::<JournalPoisoned>()
            .is_some()
    );
    assert_eq!(std::fs::read(&path).unwrap(), before);
    drop(journal);
    let (_, records) = Journal::<Event>::open(&path).unwrap();
    assert_eq!(
        records,
        vec![Event {
            n: 1,
            text: "durable".into()
        }]
    );
}

#[test]
fn journal_recovers_incomplete_body_without_losing_established_records() {
    let dir = TempDir::new();
    let path = dir.path().join("events.tsjl");
    let first = Event {
        n: 1,
        text: "established".into(),
    };
    let last = Event {
        n: 2,
        text: "unfinished".into(),
    };
    let (mut journal, _) = Journal::<Event>::open(&path).unwrap();
    journal.append(&first).unwrap();
    let durable_end = std::fs::metadata(&path).unwrap().len();
    journal.append(&last).unwrap();
    drop(journal);
    let file = std::fs::OpenOptions::new().write(true).open(&path).unwrap();
    file.set_len(file.metadata().unwrap().len() - 1).unwrap();
    file.sync_all().unwrap();
    drop(file);
    let torn = std::fs::read(&path).unwrap();

    assert!(Journal::<Event>::replay_records(&path, |_, _| Ok(())).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), torn);
    let (mut recovered, records) = Journal::<Event>::open(&path).unwrap();
    assert_eq!(records, vec![first.clone()]);
    assert_eq!(std::fs::metadata(&path).unwrap().len(), durable_end);
    recovered.append(&last).unwrap();
    drop(recovered);
    let (_, records) = Journal::<Event>::open(&path).unwrap();
    assert_eq!(records, vec![first, last]);
}

#[test]
fn journal_generation_zero_sequence_zero_is_replayed_unless_checkpointed() {
    let dir = TempDir::new();
    let path = dir.path().join("events.tsjl");
    let first = Event {
        n: 1,
        text: "first".into(),
    };
    let second = Event {
        n: 2,
        text: "second".into(),
    };
    let (mut journal, _) = Journal::<Event>::open(&path).unwrap();
    journal.append(&first).unwrap();
    journal.append(&second).unwrap();
    drop(journal);

    let mut records = Vec::new();
    Journal::<Event>::replay_records(&path, |id, payload| {
        records.push((id, ciborium::de::from_reader::<Event, _>(payload)?));
        Ok(())
    })
    .unwrap();
    assert_eq!(
        records,
        vec![
            (
                JournalRecordId {
                    generation: 0,
                    sequence: 0
                },
                first
            ),
            (
                JournalRecordId {
                    generation: 0,
                    sequence: 1
                },
                second.clone()
            ),
        ]
    );
    let options = JournalOpenOptions::after_checkpoint(JournalWatermark {
        generation: 0,
        sequence: 0,
    });
    let (mut journal, records) = Journal::<Event>::open_with_watermark(&path, options).unwrap();
    assert_eq!(records, vec![second.clone()]);
    let third = Event {
        n: 3,
        text: "third".into(),
    };
    journal.append(&third).unwrap();
    drop(journal);
    let (_, records) = Journal::<Event>::open_with_watermark(&path, options).unwrap();
    assert_eq!(records, vec![second, third]);
}

#[test]
fn journal_frames_use_monotonic_sequence_within_generation() {
    let dir = TempDir::new();
    let path = dir.path().join("events.tsjl");

    {
        let (mut journal, _) = Journal::<Event>::open(&path).unwrap();
        for n in 0..3 {
            journal
                .append(&Event {
                    n,
                    text: format!("e{n}"),
                })
                .unwrap();
        }
    }

    let bytes = std::fs::read(&path).unwrap();
    let file_generation = u64::from_le_bytes(bytes[8..16].try_into().unwrap());
    let mut offset = JOURNAL_HEADER_LEN;
    for expected in 0..3u64 {
        assert!(bytes[offset..].starts_with(&FRAME_MAGIC));
        let generation = u64::from_le_bytes(bytes[offset + 4..offset + 12].try_into().unwrap());
        assert_eq!(generation, file_generation);
        let seq = u64::from_le_bytes(bytes[offset + 12..offset + 20].try_into().unwrap());
        assert_eq!(seq, expected);
        offset += frame_len_at(&bytes, offset);
    }
}

fn frame_len_at(bytes: &[u8], offset: usize) -> usize {
    let payload_len =
        u32::from_le_bytes(bytes[offset + 20..offset + 24].try_into().unwrap()) as usize;
    FRAME_HEADER_LEN + payload_len
}

#[test]
fn corrupt_inflated_payload_len_does_not_truncate_valid_following_frames() {
    let dir = TempDir::new();
    let path = dir.path().join("events.tsjl");

    {
        let (mut journal, _) = Journal::<Event>::open(&path).unwrap();
        for n in 0..3u64 {
            journal
                .append(&Event {
                    n,
                    text: format!("e{n}"),
                })
                .unwrap();
        }
    }

    let len_before = std::fs::metadata(&path).unwrap().len();
    let mut bytes = std::fs::read(&path).unwrap();
    let frame2 = JOURNAL_HEADER_LEN + frame_len_at(&bytes, JOURNAL_HEADER_LEN);
    let generation = u64::from_le_bytes(bytes[frame2 + 4..frame2 + 12].try_into().unwrap());
    let sequence = u64::from_le_bytes(bytes[frame2 + 12..frame2 + 20].try_into().unwrap());
    let inflated = 5_000_000u32;
    bytes[frame2 + 20..frame2 + 24].copy_from_slice(&inflated.to_le_bytes());
    let seal = frame_header_seal(generation, sequence, inflated);
    bytes[frame2 + 28..frame2 + 32].copy_from_slice(&seal.to_le_bytes());
    std::fs::write(&path, &bytes).unwrap();

    assert!(Journal::<Event>::open(&path).is_err());
    assert_eq!(std::fs::metadata(&path).unwrap().len(), len_before);
    assert_eq!(std::fs::read(&path).unwrap(), bytes);

    assert!(Journal::<Event>::replay_records(&path, |_, _| Ok(())).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
}

#[test]
fn open_with_watermark_rejects_journal_generation_below_checkpoint() {
    let dir = TempDir::new();
    let path = dir.path().join("events.tsjl");
    let (mut journal, _) = Journal::<Event>::open(&path).unwrap();
    journal
        .append(&Event {
            n: 1,
            text: "x".into(),
        })
        .unwrap();
    journal.reset().unwrap();

    let before = std::fs::read(&path).unwrap();
    assert!(
        Journal::<Event>::open_with_watermark(
            &path,
            JournalOpenOptions::after_checkpoint(JournalWatermark {
                generation: 5,
                sequence: 0,
            }),
        )
        .is_err()
    );
    assert_eq!(std::fs::read(&path).unwrap(), before);
}

#[test]
fn reset_poisons_when_journal_path_is_a_directory() {
    let dir = TempDir::new();
    let path = dir.path().join("events.tsjl");

    let (mut journal, _) = Journal::<Event>::open(&path).unwrap();
    journal
        .append(&Event {
            n: 1,
            text: "x".into(),
        })
        .unwrap();

    std::fs::remove_file(&path).unwrap();
    std::fs::create_dir_all(&path).unwrap();

    assert!(journal.reset().is_err());
    assert!(journal.is_poisoned());
    assert!(
        journal
            .append(&Event {
                n: 2,
                text: "blocked".into()
            })
            .unwrap_err()
            .downcast_ref::<JournalPoisoned>()
            .is_some()
    );
    assert!(
        journal
            .reset()
            .unwrap_err()
            .downcast_ref::<JournalPoisoned>()
            .is_some()
    );
    assert!(path.is_dir());
}
