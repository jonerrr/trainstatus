use serde::{Deserialize, Serialize};

use super::support::TempDir;
use crate::storage::codec::{read, write_atomic};
use crate::storage::format::{CODEC_MAGIC, FORMAT_EPOCH, MAX_CODEC_UNCOMPRESSED, payload_checksum};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Sample {
    id: u32,
    label: String,
}

#[test]
fn codec_roundtrip_primitive_struct() {
    let dir = TempDir::new();
    let path = dir.path().join("sample.tscb");
    let value = Sample {
        id: 42,
        label: "checkpoint".into(),
    };

    write_atomic(&path, &value).unwrap();
    let decoded: Sample = read(&path).unwrap();
    assert_eq!(decoded, value);
}

#[test]
fn codec_rejects_wrong_epoch() {
    let dir = TempDir::new();
    let path = dir.path().join("bad_epoch.tscb");
    let value = Sample {
        id: 1,
        label: "x".into(),
    };
    write_atomic(&path, &value).unwrap();

    let mut bytes = std::fs::read(&path).unwrap();
    bytes[4..8].copy_from_slice(&999u32.to_le_bytes());
    std::fs::write(&path, &bytes).unwrap();

    assert!(read::<Sample>(&path).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
}

#[test]
fn codec_rejects_declared_uncompressed_above_bound() {
    let dir = TempDir::new();
    let path = dir.path().join("huge.tscb");
    write_atomic(
        &path,
        &Sample {
            id: 1,
            label: "a".into(),
        },
    )
    .unwrap();

    let mut bytes = std::fs::read(&path).unwrap();
    bytes[8..12].copy_from_slice(&((MAX_CODEC_UNCOMPRESSED as u32) + 1).to_le_bytes());
    std::fs::write(&path, &bytes).unwrap();

    assert!(read::<Sample>(&path).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
}

#[test]
fn codec_rejects_checksum_tamper() {
    let dir = TempDir::new();
    let path = dir.path().join("bad_crc.tscb");
    write_atomic(
        &path,
        &Sample {
            id: 2,
            label: "y".into(),
        },
    )
    .unwrap();

    let mut bytes = std::fs::read(&path).unwrap();
    let checksum_offset = 12;
    let expected = u32::from_le_bytes(
        bytes[checksum_offset..checksum_offset + 4]
            .try_into()
            .unwrap(),
    );
    bytes[checksum_offset..checksum_offset + 4]
        .copy_from_slice(&(expected ^ 0x00ff_00ff).to_le_bytes());
    std::fs::write(&path, &bytes).unwrap();

    assert!(read::<Sample>(&path).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
}

#[test]
fn codec_header_magic_and_checksum_match_payload() {
    let dir = TempDir::new();
    let path = dir.path().join("header.tscb");
    let value = Sample {
        id: 3,
        label: "z".into(),
    };
    write_atomic(&path, &value).unwrap();

    let bytes = std::fs::read(&path).unwrap();
    assert_eq!(&bytes[..4], &CODEC_MAGIC);
    assert_eq!(
        u32::from_le_bytes(bytes[4..8].try_into().unwrap()),
        FORMAT_EPOCH
    );

    let uncompressed_len = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
    let header_checksum = u32::from_le_bytes(bytes[12..16].try_into().unwrap());
    let cbor = zstd::decode_all(&bytes[16..]).unwrap();
    assert_eq!(cbor.len(), uncompressed_len);
    assert_eq!(header_checksum, payload_checksum(&cbor));
}

#[test]
fn concurrent_immutable_publication_never_steals_another_writers_temporary_file() {
    let dir = TempDir::new();
    let path = dir.path().join("shared.tscb");
    let value = Sample {
        id: 17,
        label: "immutable geometry".repeat(2048),
    };
    let bytes = crate::storage::codec::encode_to_file_bytes(&value).unwrap();
    let barrier = std::sync::Barrier::new(16);
    std::thread::scope(|scope| {
        let handles: Vec<_> = (0..16)
            .map(|_| {
                scope.spawn(|| {
                    barrier.wait();
                    crate::storage::codec::write_encoded_atomic(&path, &bytes)
                })
            })
            .collect();
        for handle in handles {
            handle.join().unwrap().unwrap();
        }
    });
    assert_eq!(read::<Sample>(&path).unwrap(), value);
}
