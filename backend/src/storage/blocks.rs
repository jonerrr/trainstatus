//! Packed immutable files with independently compressed, directly addressable records.

use std::{
    collections::HashSet,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    os::unix::fs::FileExt,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

use anyhow::Context;
use moka::sync::Cache;
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use super::{
    codec::{decode_file_bytes, encode_to_file_bytes},
    content::ContentId,
    format::{FORMAT_EPOCH, MAX_CODEC_UNCOMPRESSED, payload_checksum},
    sync::{create_dir_durable, fsync_parent},
};

const MAGIC: &[u8; 4] = b"TSBK";
const PREFIX_LEN: usize = 12;
const ENTRY_LEN: usize = 12;
const TARGET_BLOCK_BYTES: usize = 8 * 1024 * 1024;
const MAX_RECORDS: usize = 4096;
const MAX_RECORD_BYTES: usize = MAX_CODEC_UNCOMPRESSED + 16 * 1024 * 1024 + 16;
static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct RecordRef {
    pub block: ContentId,
    pub slot: u32,
}

struct BlockReader {
    file: File,
    directory: Vec<u8>,
    count: u32,
}

#[derive(Clone)]
pub struct BlockStore {
    root: Arc<PathBuf>,
    readers: Cache<ContentId, Arc<BlockReader>>,
}

impl BlockStore {
    pub fn open(root: impl AsRef<Path>) -> anyhow::Result<Self> {
        let root = root.as_ref();
        create_dir_durable(root)?;
        Ok(Self {
            root: Arc::new(root.to_path_buf()),
            readers: Cache::builder()
                .max_capacity(8 * 1024 * 1024)
                // The minimum weight bounds open descriptors as well as directory bytes.
                .weigher(|_: &ContentId, reader: &Arc<BlockReader>| {
                    reader.directory.len().max(64 * 1024) as u32
                })
                .build(),
        })
    }

    pub fn root(&self) -> &Path {
        self.root.as_path()
    }

    /// Called only after checkpoint publication and WAL reset, with all reader roots pinned.
    pub fn collect_garbage(&self, live: &HashSet<ContentId>) -> anyhow::Result<()> {
        let mut removed = false;
        for entry in fs::read_dir(self.root())? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().is_none_or(|extension| extension != "trsb") {
                continue;
            }
            let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
                continue;
            };
            let Ok(id) = ContentId::parse_hex(stem) else {
                continue;
            };
            if !live.contains(&id) {
                self.readers.invalidate(&id);
                fs::remove_file(&path)
                    .with_context(|| format!("remove obsolete block {}", path.display()))?;
                removed = true;
            }
        }
        // Release cache-owned descriptors as well as directory entries.
        self.readers.run_pending_tasks();
        if removed {
            File::open(self.root())?.sync_all()?;
        }
        Ok(())
    }

    pub fn write<T: Serialize>(&self, records: &[T]) -> anyhow::Result<Vec<RecordRef>> {
        let mut references = Vec::with_capacity(records.len());
        let mut pending = Vec::with_capacity(records.len().min(MAX_RECORDS));
        let mut size = 0;
        for record in records {
            let bytes = encode_to_file_bytes(record)?;
            if !pending.is_empty()
                && (size + bytes.len() > TARGET_BLOCK_BYTES || pending.len() == MAX_RECORDS)
            {
                self.seal(&pending, &mut references)?;
                pending.clear();
                size = 0;
            }
            size += bytes.len();
            pending.push(bytes);
        }
        if !pending.is_empty() {
            self.seal(&pending, &mut references)?;
        }
        Ok(references)
    }

    pub fn read<T: DeserializeOwned>(&self, record: &RecordRef) -> anyhow::Result<T> {
        let reader = self
            .readers
            .try_get_with(record.block, || {
                self.open_reader(record.block).map(Arc::new)
            })
            .map_err(|error| anyhow::anyhow!("{error:#}"))?;
        anyhow::ensure!(
            record.slot < reader.count,
            "record slot {} outside block {}",
            record.slot,
            record.block
        );
        let start = PREFIX_LEN + record.slot as usize * ENTRY_LEN;
        let offset = u64::from_le_bytes(reader.directory[start..start + 8].try_into()?);
        let length =
            u32::from_le_bytes(reader.directory[start + 8..start + ENTRY_LEN].try_into()?) as usize;
        let mut bytes = vec![0; length];
        reader
            .file
            .read_exact_at(&mut bytes, offset)
            .with_context(|| format!("read record {} in block {}", record.slot, record.block))?;
        decode_file_bytes(&bytes)
            .with_context(|| format!("decode record {} in block {}", record.slot, record.block))
    }

    fn seal(&self, records: &[Vec<u8>], references: &mut Vec<RecordRef>) -> anyhow::Result<()> {
        let directory_len = PREFIX_LEN + records.len() * ENTRY_LEN + 4;
        let mut directory = Vec::with_capacity(directory_len);
        directory.extend_from_slice(MAGIC);
        directory.extend_from_slice(&FORMAT_EPOCH.to_le_bytes());
        directory.extend_from_slice(&(records.len() as u32).to_le_bytes());
        let mut offset = directory_len as u64;
        for record in records {
            directory.extend_from_slice(&offset.to_le_bytes());
            directory.extend_from_slice(&u32::try_from(record.len())?.to_le_bytes());
            offset += record.len() as u64;
        }
        directory.extend_from_slice(&payload_checksum(&directory).to_le_bytes());
        let mut hash = blake3::Hasher::new();
        hash.update(&directory);
        for bytes in records {
            hash.update(bytes);
        }
        let id = ContentId::from_bytes(*hash.finalize().as_bytes());
        let path = self.root.join(format!("{id}.trsb"));
        self.publish(&path, id, &directory, records)?;
        references.extend((0..records.len()).map(|slot| RecordRef {
            block: id,
            slot: slot as u32,
        }));
        Ok(())
    }

    fn publish(
        &self,
        path: &Path,
        id: ContentId,
        directory: &[u8],
        records: &[Vec<u8>],
    ) -> anyhow::Result<()> {
        match File::open(path) {
            Ok(mut existing) => {
                // A previous uncertain publication may have left this name behind.
                let mut hash = blake3::Hasher::new();
                let mut buffer = [0; 65_536];
                loop {
                    let count = existing.read(&mut buffer)?;
                    if count == 0 {
                        break;
                    }
                    hash.update(&buffer[..count]);
                }
                anyhow::ensure!(
                    hash.finalize().as_bytes() == id.as_bytes(),
                    "existing immutable block {} is corrupt",
                    path.display()
                );
                existing.sync_all()?;
                fsync_parent(path)?;
                return Ok(());
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        let (temporary, mut file) = loop {
            let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let temporary = self
                .root
                .join(format!(".{id}.{}-{sequence}.part", std::process::id()));
            match OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary)
            {
                Ok(file) => break (temporary, file),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error.into()),
            }
        };
        let result = (|| -> anyhow::Result<()> {
            file.write_all(directory)?;
            for bytes in records {
                file.write_all(bytes)?;
            }
            file.sync_all()?;
            fs::rename(&temporary, path)?;
            fsync_parent(path)?;
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(temporary);
        }
        result.with_context(|| format!("publish immutable block {}", path.display()))
    }

    fn open_reader(&self, id: ContentId) -> anyhow::Result<BlockReader> {
        let path = self.root.join(format!("{id}.trsb"));
        let mut file =
            File::open(&path).with_context(|| format!("open block {}", path.display()))?;
        let file_len = file.metadata()?.len();
        let mut prefix = [0; PREFIX_LEN];
        file.read_exact(&mut prefix)?;
        anyhow::ensure!(&prefix[..4] == MAGIC, "invalid block magic");
        let epoch = u32::from_le_bytes(prefix[4..8].try_into()?);
        anyhow::ensure!(epoch == FORMAT_EPOCH, "unsupported block epoch {epoch}");
        let count = u32::from_le_bytes(prefix[8..12].try_into()?);
        anyhow::ensure!(
            count > 0 && count as usize <= MAX_RECORDS,
            "invalid block record count {count}"
        );
        let directory_len = PREFIX_LEN + count as usize * ENTRY_LEN + 4;
        anyhow::ensure!(
            file_len >= directory_len as u64
                && file_len <= (MAX_RECORD_BYTES + directory_len) as u64,
            "block size outside bounds"
        );
        let mut directory = vec![0; directory_len];
        directory[..PREFIX_LEN].copy_from_slice(&prefix);
        file.read_exact(&mut directory[PREFIX_LEN..])?;
        let checksum = u32::from_le_bytes(directory[directory_len - 4..].try_into()?);
        anyhow::ensure!(
            checksum == payload_checksum(&directory[..directory_len - 4]),
            "block directory checksum mismatch"
        );
        let mut expected = directory_len as u64;
        for entry in directory[PREFIX_LEN..directory_len - 4]
            .as_chunks::<ENTRY_LEN>()
            .0
        {
            let offset = u64::from_le_bytes(entry[..8].try_into()?);
            let length = u32::from_le_bytes(entry[8..].try_into()?) as usize;
            anyhow::ensure!(
                offset == expected && (16..=MAX_RECORD_BYTES).contains(&length),
                "invalid record directory entry"
            );
            expected = expected
                .checked_add(length as u64)
                .context("block offset overflow")?;
            anyhow::ensure!(expected <= file_len, "record exceeds block length");
        }
        anyhow::ensure!(expected == file_len, "unindexed block trailing bytes");
        Ok(BlockReader {
            file,
            directory,
            count,
        })
    }
}
