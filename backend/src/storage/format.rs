//! On-disk magic values, format epoch, and size bounds shared by codec and journal files.

/// Bump on incompatible codec envelope or native payload-schema changes.
pub const FORMAT_EPOCH: u32 = 3;

pub const CODEC_MAGIC: [u8; 4] = *b"TSCB";
pub const JOURNAL_MAGIC: [u8; 4] = *b"TSJL";
pub const FRAME_MAGIC: [u8; 4] = *b"TSFR";

pub const LOCK_FILE_NAME: &str = ".storage.lock";

/// Bump on incompatible journal envelope or accepted-record schema changes.
pub const JOURNAL_FORMAT_VERSION: u32 = 4;

/// `TSJL` + `JOURNAL_FORMAT_VERSION` + monotonic `generation` (`u64`).
pub const JOURNAL_HEADER_LEN: usize = 16;

/// `TSFR` + `generation` + `sequence` + `payload_len` + checksum + reserved.
pub const FRAME_HEADER_LEN: usize = 32;

/// Upper bound on uncompressed typed CBOR before compression (single codec file).
///
/// Large domain bundles (for example full network schedules or geometry catalogs) must be sharded
/// into multiple codec files under this limit; do not rely on raising this cap for whole imports.
pub const MAX_CODEC_UNCOMPRESSED: usize = 64 * 1024 * 1024;

/// Upper bound on a single journal record's typed CBOR payload.
pub const MAX_FRAME_PAYLOAD: usize = 16 * 1024 * 1024;

/// Truncated BLAKE3 digest (first four bytes, little-endian) of `payload`.
pub fn payload_checksum(payload: &[u8]) -> u32 {
    let hash = blake3::hash(payload);
    u32::from_le_bytes(hash.as_bytes()[..4].try_into().expect("four bytes"))
}

/// Checksum of frame metadata (`generation`, `sequence`, `payload_len`) before the body is read.
pub fn frame_header_seal(generation: u64, sequence: u64, payload_len: u32) -> u32 {
    let mut commitment = [0u8; 20];
    commitment[0..8].copy_from_slice(&generation.to_le_bytes());
    commitment[8..16].copy_from_slice(&sequence.to_le_bytes());
    commitment[16..20].copy_from_slice(&payload_len.to_le_bytes());
    payload_checksum(&commitment)
}

/// Checksum covering frame metadata and CBOR payload (detects torn/corrupt bodies).
pub fn frame_body_checksum(
    generation: u64,
    sequence: u64,
    payload_len: u32,
    payload: &[u8],
) -> u32 {
    let mut metadata = [0u8; 20];
    metadata[0..8].copy_from_slice(&generation.to_le_bytes());
    metadata[8..16].copy_from_slice(&sequence.to_le_bytes());
    metadata[16..20].copy_from_slice(&payload_len.to_le_bytes());
    let mut hasher = blake3::Hasher::new();
    hasher.update(&metadata);
    hasher.update(payload);
    u32::from_le_bytes(
        hasher.finalize().as_bytes()[..4]
            .try_into()
            .expect("four bytes"),
    )
}
