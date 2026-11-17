use std::fs;
use std::io::{Cursor, Read};
use std::path::Path;

use anyhow::{Context, bail};
use serde::Serialize;
use serde::de::DeserializeOwned;

use super::format::{CODEC_MAGIC, FORMAT_EPOCH, MAX_CODEC_UNCOMPRESSED, payload_checksum};
use super::sync::replace_file;

const CODEC_HEADER_LEN: usize = 16;

/// Read a typed value from a codec file (`TSCB` envelope, zstd-compressed self-describing CBOR).
pub fn read<T: DeserializeOwned>(path: &Path) -> anyhow::Result<T> {
    let meta = fs::metadata(path).with_context(|| format!("stat codec file {}", path.display()))?;
    let max_on_disk =
        CODEC_HEADER_LEN as u64 + u64::from(MAX_CODEC_UNCOMPRESSED as u32) + (16 * 1024 * 1024);
    if meta.len() > max_on_disk {
        bail!(
            "codec file size {} exceeds bound {}",
            meta.len(),
            max_on_disk
        );
    }
    let bytes = fs::read(path).with_context(|| format!("read codec file {}", path.display()))?;
    decode_file_bytes(&bytes).with_context(|| format!("decode codec file {}", path.display()))
}

/// Encode a value to on-disk codec bytes (for content-addressed reuse).
pub fn encode_to_file_bytes<T: Serialize + ?Sized>(value: &T) -> anyhow::Result<Vec<u8>> {
    encode_codec_file(value)
}

/// Atomically write a typed value to `path` with magic/epoch, size bound, checksum, compression, and durable replace.
pub fn write_atomic<T: Serialize + ?Sized>(path: &Path, value: &T) -> anyhow::Result<()> {
    let file_bytes = encode_codec_file(value)?;
    write_encoded_atomic(path, &file_bytes)
}

/// Publish already encoded bytes without repeating serialization and compression.
pub fn write_encoded_atomic(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    replace_file(path, bytes).with_context(|| format!("replace encoded file {}", path.display()))
}

fn encode_codec_file<T: Serialize + ?Sized>(value: &T) -> anyhow::Result<Vec<u8>> {
    let mut cbor = Vec::new();
    ciborium::ser::into_writer(value, &mut cbor).context("encode value as CBOR")?;
    if cbor.len() > MAX_CODEC_UNCOMPRESSED {
        bail!(
            "uncompressed CBOR size {} exceeds {} byte bound; split the artifact into smaller typed codec files",
            cbor.len(),
            MAX_CODEC_UNCOMPRESSED
        );
    }

    let checksum = payload_checksum(&cbor);
    let compressed =
        zstd::encode_all(Cursor::new(&cbor), 3).context("zstd-compress codec payload")?;

    let mut out = Vec::with_capacity(CODEC_HEADER_LEN + compressed.len());
    out.extend_from_slice(&CODEC_MAGIC);
    out.extend_from_slice(&FORMAT_EPOCH.to_le_bytes());
    out.extend_from_slice(&(cbor.len() as u32).to_le_bytes());
    out.extend_from_slice(&checksum.to_le_bytes());
    out.extend_from_slice(&compressed);
    Ok(out)
}

pub fn decode_file_bytes<T: DeserializeOwned>(bytes: &[u8]) -> anyhow::Result<T> {
    if bytes.len() < CODEC_HEADER_LEN {
        bail!("codec file too short");
    }

    let magic: [u8; 4] = bytes[..4].try_into().unwrap();
    if magic != CODEC_MAGIC {
        bail!(
            "codec magic mismatch: expected {:?}, got {:?}",
            CODEC_MAGIC,
            magic
        );
    }

    let epoch = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
    if epoch != FORMAT_EPOCH {
        bail!("unsupported codec format epoch {epoch} (expected {FORMAT_EPOCH})");
    }

    let uncompressed_len = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
    if uncompressed_len > MAX_CODEC_UNCOMPRESSED {
        bail!("declared uncompressed size {uncompressed_len} exceeds bound");
    }

    let expected_checksum = u32::from_le_bytes(bytes[12..16].try_into().unwrap());
    let compressed = &bytes[16..];

    let cbor = decompress_codec_payload(compressed, uncompressed_len)
        .context("zstd-decompress codec payload")?;

    let checksum = payload_checksum(&cbor);
    if checksum != expected_checksum {
        bail!("codec payload checksum mismatch");
    }

    ciborium::de::from_reader(Cursor::new(&cbor)).context("decode CBOR value")
}

/// Stream-decompress with a hard ceiling so hostile zstd cannot allocate past the codec bound.
fn decompress_codec_payload(compressed: &[u8], expected_len: usize) -> anyhow::Result<Vec<u8>> {
    let mut decoder = zstd::Decoder::new(Cursor::new(compressed)).context("open zstd decoder")?;
    let mut out = Vec::with_capacity(expected_len.min(4096));
    let mut buf = [0u8; 65_536];

    loop {
        let read = decoder.read(&mut buf).context("read zstd codec chunk")?;
        if read == 0 {
            break;
        }
        let new_len = out
            .len()
            .checked_add(read)
            .ok_or_else(|| anyhow::anyhow!("decompressed codec payload length overflow"))?;
        if new_len > MAX_CODEC_UNCOMPRESSED {
            bail!(
                "decompressed codec payload exceeds {} byte bound",
                MAX_CODEC_UNCOMPRESSED
            );
        }
        out.extend_from_slice(&buf[..read]);
    }

    if out.len() != expected_len {
        bail!(
            "uncompressed length mismatch: header {expected_len}, actual {}",
            out.len()
        );
    }
    Ok(out)
}
