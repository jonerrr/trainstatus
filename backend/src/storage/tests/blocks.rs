use std::fs;

use serde::{Deserialize, Serialize};

use super::support::TempDir;
use crate::storage::blocks::BlockStore;

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Passenger {
    run: String,
    arrivals: Vec<i64>,
}

#[test]
fn selected_record_does_not_load_or_decode_other_payloads() {
    let dir = TempDir::new();
    let store = BlockStore::open(dir.path()).unwrap();
    let records = [
        Passenger {
            run: "first".into(),
            arrivals: vec![100, 200],
        },
        Passenger {
            run: "second".into(),
            arrivals: vec![300, 400],
        },
    ];
    let references = store.write(&records).unwrap();
    let path = dir.path().join(format!("{}.trsb", references[0].block));
    let mut bytes = fs::read(&path).unwrap();
    let first_offset = u64::from_le_bytes(bytes[12..20].try_into().unwrap()) as usize;
    bytes[first_offset + 12] ^= 1;
    fs::write(&path, bytes).unwrap();
    assert_eq!(store.read::<Passenger>(&references[1]).unwrap(), records[1]);
    let error = store.read::<Passenger>(&references[0]).unwrap_err();
    assert!(format!("{error:#}").contains("checksum mismatch"));
    assert!(
        store.write(&records).is_err(),
        "never reuse a corrupt immutable name"
    );
}

#[test]
fn corrupt_directory_cannot_redirect_a_record() {
    let dir = TempDir::new();
    let store = BlockStore::open(dir.path()).unwrap();
    let references = store
        .write(&[Passenger {
            run: "run".into(),
            arrivals: vec![123],
        }])
        .unwrap();
    let path = dir.path().join(format!("{}.trsb", references[0].block));
    let mut bytes = fs::read(path.clone()).unwrap();
    bytes[12] ^= 1;
    fs::write(path, bytes).unwrap();
    let error = store.read::<Passenger>(&references[0]).unwrap_err();
    assert!(format!("{error:#}").contains("directory checksum mismatch"));
}

#[test]
fn corrupt_count_and_out_of_range_slot_fail_without_materialization() {
    let dir = TempDir::new();
    let store = BlockStore::open(dir.path()).unwrap();
    let mut reference = store
        .write(&[Passenger {
            run: "run".into(),
            arrivals: vec![123],
        }])
        .unwrap()
        .remove(0);
    reference.slot = u32::MAX;
    let error = store.read::<Passenger>(&reference).unwrap_err();
    assert!(format!("{error:#}").contains("outside block"));
    let path = dir.path().join(format!("{}.trsb", reference.block));
    let mut bytes = fs::read(path.clone()).unwrap();
    bytes[8..12].copy_from_slice(&u32::MAX.to_le_bytes());
    fs::write(path, bytes).unwrap();
    let reopened = BlockStore::open(dir.path()).unwrap();
    let error = reopened.read::<Passenger>(&reference).unwrap_err();
    assert!(format!("{error:#}").contains("invalid block record count"));
}
