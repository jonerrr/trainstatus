use std::fs::{File, OpenOptions};
use std::io::{Cursor, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use super::format::{
    FRAME_HEADER_LEN, FRAME_MAGIC, JOURNAL_FORMAT_VERSION, JOURNAL_HEADER_LEN, JOURNAL_MAGIC,
    MAX_FRAME_PAYLOAD, frame_body_checksum, frame_header_seal,
};
use super::sync::{create_dir_durable, replace_file, truncate_durable};

/// Checkpoint-derived replay boundary (`generation`, `sequence`).
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub struct JournalWatermark {
    pub generation: u64,
    pub sequence: u64,
}

impl From<JournalRecordId> for JournalWatermark {
    fn from(id: JournalRecordId) -> Self {
        Self {
            generation: id.generation,
            sequence: id.sequence,
        }
    }
}

impl From<JournalWatermark> for JournalRecordId {
    fn from(w: JournalWatermark) -> Self {
        Self {
            generation: w.generation,
            sequence: w.sequence,
        }
    }
}

/// Monotonic journal record identity. Survives `reset()` via `generation`; `sequence` restarts per generation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct JournalRecordId {
    pub generation: u64,
    pub sequence: u64,
}

/// Result of a bounded streaming scan (no full-file buffering of payloads).
#[derive(Clone, Copy, Debug)]
pub struct JournalReplaySummary {
    pub generation: u64,
    pub next_sequence: u64,
    pub durable_end: u64,
    pub frames_replayed: u64,
}

/// Options for [`Journal::open_with_watermark`] when a checkpoint already exists.
#[derive(Clone, Copy, Debug, Default)]
pub struct JournalOpenOptions {
    /// Last record durably reflected in a checkpoint, absent for a fresh store.
    pub watermark: Option<JournalWatermark>,
    /// Explicit generation for initializing a missing journal.
    pub initial_generation: Option<u64>,
}

impl JournalOpenOptions {
    pub fn after_checkpoint(watermark: JournalWatermark) -> Self {
        Self {
            watermark: Some(watermark),
            initial_generation: None,
        }
    }

    pub fn with_initial_generation(mut self, generation: u64) -> Self {
        self.initial_generation = Some(generation);
        self
    }
}

/// Append-only typed journal with generation-scoped sequences, durable tail recovery, and poisoned state.
///
/// **Operations / Alerts:** persist `JournalWatermark` in checkpoints; on startup call
/// [`Journal::open_with_watermark`] (recovery may truncate tails) or
/// [`Journal::replay_records_with_watermark`] (read-only audit); skip frames with `id <= watermark`.
#[derive(Debug)]
pub struct Journal<T> {
    path: PathBuf,
    file: File,
    generation: u64,
    next_seq: u64,
    durable_end: u64,
    poisoned: bool,
    _marker: std::marker::PhantomData<T>,
}

/// Journal rejected an operation because a prior append may have left ambiguous on-disk state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JournalPoisoned {
    pub generation: u64,
    pub last_confirmed_seq: u64,
}

impl std::fmt::Display for JournalPoisoned {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "journal is poisoned after uncertain write (generation {}, last confirmed sequence {})",
            self.generation, self.last_confirmed_seq
        )
    }
}

impl std::error::Error for JournalPoisoned {}

#[derive(Clone, Copy, Debug)]
enum ScanMode {
    Recover,
    ReadOnly,
}

impl<T> Journal<T>
where
    T: Serialize + DeserializeOwned,
{
    /// Open with generation floor enforcement, recover tails, and return decoded records (`id > watermark` only).
    ///
    /// A short final frame is recoverable only if its remaining bytes contain no
    /// sealed frame header; otherwise opening fails without truncating the file.
    pub fn open_with_watermark(
        path: &Path,
        options: JournalOpenOptions,
    ) -> anyhow::Result<(Self, Vec<T>)> {
        let mut records = Vec::new();
        let (journal, _) = Self::open_with_watermark_replay(path, options, |_id, payload| {
            let value: T =
                ciborium::de::from_reader(payload).context("decode journal record CBOR")?;
            records.push(value);
            Ok(())
        })?;
        Ok((journal, records))
    }

    /// Recover journal tails, enforce generation floor, and stream frames with `id > watermark` to `replay`.
    pub fn open_with_watermark_replay<F>(
        path: &Path,
        options: JournalOpenOptions,
        mut replay: F,
    ) -> anyhow::Result<(Self, JournalReplaySummary)>
    where
        F: FnMut(JournalRecordId, &mut dyn Read) -> anyhow::Result<()>,
    {
        let watermark = options.watermark;
        anyhow::ensure!(
            watermark.is_none() || path.exists() || options.initial_generation.is_some(),
            "checkpoint references missing journal {}",
            path.display()
        );
        let floor = watermark.map_or(0, |mark| mark.generation);
        let initial = options.initial_generation.unwrap_or(floor);
        ensure_parent_durable(path)?;

        let summary = scan_journal(
            path,
            ScanMode::Recover,
            floor,
            initial,
            &mut |id, reader| {
                if watermark.is_some_and(|mark| id <= JournalRecordId::from(mark)) {
                    return Ok(());
                }
                replay(id, reader)
            },
        )?;

        let mut file = OpenOptions::new()
            .read(true)
            .create(true)
            .append(true)
            .open(path)
            .with_context(|| format!("open journal {}", path.display()))?;

        file.seek(SeekFrom::Start(summary.durable_end))
            .with_context(|| "seek journal to durable end")?;

        Ok((
            Self {
                path: path.to_path_buf(),
                file,
                generation: summary.generation,
                next_seq: summary.next_sequence,
                durable_end: summary.durable_end,
                poisoned: false,
                _marker: std::marker::PhantomData,
            },
            summary,
        ))
    }

    /// Open without a checkpoint watermark (fresh storage).
    pub fn open(path: &Path) -> anyhow::Result<(Self, Vec<T>)> {
        Self::open_with_watermark(path, JournalOpenOptions::default())
    }

    /// Open and replay every frame (no watermark filtering).
    pub fn open_with_replay<F>(
        path: &Path,
        replay: F,
    ) -> anyhow::Result<(Self, JournalReplaySummary)>
    where
        F: FnMut(JournalRecordId, &mut dyn Read) -> anyhow::Result<()>,
    {
        Self::open_with_watermark_replay(path, JournalOpenOptions::default(), replay)
    }

    /// Read-only replay; never creates, truncates, or mutates the journal file.
    pub fn replay_records_with_watermark<F>(
        path: &Path,
        watermark: JournalWatermark,
        mut replay: F,
    ) -> anyhow::Result<JournalReplaySummary>
    where
        F: FnMut(JournalRecordId, &mut dyn Read) -> anyhow::Result<()>,
    {
        scan_journal(
            path,
            ScanMode::ReadOnly,
            watermark.generation,
            watermark.generation,
            &mut |id, reader| {
                if id <= JournalRecordId::from(watermark) {
                    return Ok(());
                }
                replay(id, reader)
            },
        )
    }

    /// Read-only replay of all frames (no watermark filter).
    pub fn replay_records<F>(path: &Path, mut replay: F) -> anyhow::Result<JournalReplaySummary>
    where
        F: FnMut(JournalRecordId, &mut dyn Read) -> anyhow::Result<()>,
    {
        scan_journal(path, ScanMode::ReadOnly, 0, 0, &mut replay)
    }

    pub fn append(&mut self, record: &T) -> anyhow::Result<()> {
        if self.poisoned {
            return Err(anyhow::Error::new(JournalPoisoned {
                generation: self.generation,
                last_confirmed_seq: self.next_seq.saturating_sub(1),
            }));
        }

        let mut payload = Vec::new();
        ciborium::ser::into_writer(record, &mut payload)
            .context("encode journal record as CBOR")?;
        if payload.len() > MAX_FRAME_PAYLOAD {
            bail!(
                "journal record size {} exceeds bound {}",
                payload.len(),
                MAX_FRAME_PAYLOAD
            );
        }

        let seq = self.next_seq;
        let frame = encode_frame(self.generation, seq, &payload)?;
        let start = self.durable_end;

        if let Err(err) = self.write_frame(&frame) {
            self.poisoned = true;
            let _ = truncate_durable(&self.path, start);
            return Err(err).context("append journal frame");
        }

        self.next_seq = seq + 1;
        self.durable_end += frame.len() as u64;
        Ok(())
    }

    /// Start a new generation after a durable checkpoint (atomic file replace).
    pub fn reset(&mut self) -> anyhow::Result<()> {
        if self.poisoned {
            return Err(anyhow::Error::new(JournalPoisoned {
                generation: self.generation,
                last_confirmed_seq: self.next_seq.saturating_sub(1),
            }));
        }

        let next_generation = self.generation.saturating_add(1);
        let header = journal_header_bytes(next_generation);

        if let Err(err) = replace_file(&self.path, &header)
            .with_context(|| format!("reset journal {}", self.path.display()))
        {
            self.poisoned = true;
            return Err(err);
        }

        match (|| -> anyhow::Result<()> {
            self.file = OpenOptions::new()
                .read(true)
                .create(true)
                .append(true)
                .open(&self.path)
                .with_context(|| format!("reopen journal {}", self.path.display()))?;
            self.generation = next_generation;
            self.next_seq = 0;
            self.durable_end = JOURNAL_HEADER_LEN as u64;
            self.file
                .seek(SeekFrom::Start(self.durable_end))
                .context("seek reset journal")?;
            Ok(())
        })() {
            Ok(()) => Ok(()),
            Err(err) => {
                self.poisoned = true;
                Err(err)
            }
        }
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn next_sequence(&self) -> u64 {
        self.next_seq
    }

    pub fn next_record_id(&self) -> JournalRecordId {
        JournalRecordId {
            generation: self.generation,
            sequence: self.next_seq,
        }
    }

    pub fn is_poisoned(&self) -> bool {
        self.poisoned
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn write_frame(&mut self, frame: &[u8]) -> anyhow::Result<()> {
        self.file.write_all(frame).context("write journal frame")?;
        self.file.flush().context("flush journal")?;
        self.file.sync_all().context("fsync journal")?;
        Ok(())
    }
}

fn ensure_parent_durable(path: &Path) -> anyhow::Result<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        create_dir_durable(parent)
            .with_context(|| format!("create journal parent {}", parent.display()))?;
    }
    Ok(())
}

fn journal_header_bytes(generation: u64) -> Vec<u8> {
    let mut header = Vec::with_capacity(JOURNAL_HEADER_LEN);
    header.extend_from_slice(&JOURNAL_MAGIC);
    header.extend_from_slice(&JOURNAL_FORMAT_VERSION.to_le_bytes());
    header.extend_from_slice(&generation.to_le_bytes());
    header
}

fn scan_journal<F>(
    path: &Path,
    mode: ScanMode,
    generation_floor: u64,
    initial_generation: u64,
    replay: &mut F,
) -> anyhow::Result<JournalReplaySummary>
where
    F: FnMut(JournalRecordId, &mut dyn Read) -> anyhow::Result<()>,
{
    if !path.exists() {
        if matches!(mode, ScanMode::ReadOnly) {
            bail!("journal {} does not exist", path.display());
        }
        if initial_generation < generation_floor {
            bail!(
                "initial journal generation {initial_generation} is below checkpoint generation {generation_floor}"
            );
        }
        let header = journal_header_bytes(initial_generation);
        replace_file(path, &header).context("initialize empty journal")?;
        return Ok(JournalReplaySummary {
            generation: initial_generation,
            next_sequence: 0,
            durable_end: JOURNAL_HEADER_LEN as u64,
            frames_replayed: 0,
        });
    }

    let mut file = File::open(path).with_context(|| format!("open journal {}", path.display()))?;
    let file_len = file.metadata()?.len();

    if file_len == 0 {
        bail!("journal file is empty");
    }

    if file_len < JOURNAL_HEADER_LEN as u64 {
        bail!(
            "journal header truncated ({} bytes, need {})",
            file_len,
            JOURNAL_HEADER_LEN
        );
    }

    let generation = read_journal_header(&mut file)?;
    if generation < generation_floor {
        bail!("journal generation {generation} is below checkpoint generation {generation_floor}");
    }

    let mut offset = JOURNAL_HEADER_LEN as u64;
    let mut next_seq = 0u64;
    let mut frames_replayed = 0u64;

    while offset < file_len {
        file.seek(SeekFrom::Start(offset))?;
        let header = match read_frame_header(&mut file, file_len - offset) {
            HeaderRead::Complete(h) => h,
            HeaderRead::IncompleteTail => {
                if matches!(mode, ScanMode::ReadOnly) {
                    bail!("journal has incomplete tail at offset {offset}");
                }
                truncate_durable(path, offset).with_context(|| {
                    format!(
                        "truncate incomplete journal tail at offset {offset} in {}",
                        path.display()
                    )
                })?;
                break;
            }
            HeaderRead::Corrupt(err) => return Err(err).context("journal frame header corrupt"),
        };

        if header.generation != generation {
            bail!(
                "journal frame generation {} does not match file generation {}",
                header.generation,
                generation
            );
        }

        if header.sequence != next_seq {
            bail!(
                "journal frame sequence mismatch: expected {next_seq}, got {}",
                header.sequence
            );
        }

        if header.payload_len > MAX_FRAME_PAYLOAD {
            bail!(
                "journal frame payload length {} exceeds bound {}",
                header.payload_len,
                MAX_FRAME_PAYLOAD
            );
        }

        let payload_start = offset + FRAME_HEADER_LEN as u64;
        let payload_available = file_len.saturating_sub(payload_start);

        if header.payload_len as u64 > payload_available {
            let mut tail = vec![0u8; payload_available as usize];
            file.seek(SeekFrom::Start(payload_start))?;
            file.read_exact(&mut tail)
                .context("read journal frame tail for length validation")?;
            // The unfinished payload can precede later established frames. Checking
            // only its first bytes would mistake a corrupted length for a torn tail
            // and discard those frames during recovery. Fail closed if any sealed
            // frame header survives in the remaining bytes.
            if tail.windows(FRAME_HEADER_LEN).any(frame_header_valid) {
                bail!(
                    "corrupt journal frame payload length {} at offset {offset} ({payload_available} bytes follow)",
                    header.payload_len
                );
            }
            if matches!(mode, ScanMode::ReadOnly) {
                bail!(
                    "journal frame body short at offset {offset}: declared {} bytes, {} available",
                    header.payload_len,
                    payload_available
                );
            }
            truncate_durable(path, offset).with_context(|| {
                format!(
                    "truncate incomplete journal frame at offset {offset} in {}",
                    path.display()
                )
            })?;
            break;
        }

        let mut payload = vec![0u8; header.payload_len];
        file.read_exact(&mut payload)
            .context("read journal frame payload")?;

        let body_checksum = frame_body_checksum(
            header.generation,
            header.sequence,
            header.payload_len as u32,
            &payload,
        );
        if body_checksum != header.body_checksum {
            bail!(
                "journal frame body checksum mismatch at generation {} sequence {}",
                header.generation,
                header.sequence
            );
        }

        let id = JournalRecordId {
            generation: header.generation,
            sequence: header.sequence,
        };
        let mut cursor = Cursor::new(&payload);
        replay(id, &mut cursor)?;

        offset += FRAME_HEADER_LEN as u64 + header.payload_len as u64;
        next_seq = next_seq.saturating_add(1);
        frames_replayed += 1;
    }

    Ok(JournalReplaySummary {
        generation,
        next_sequence: next_seq,
        durable_end: offset,
        frames_replayed,
    })
}

fn read_journal_header(file: &mut File) -> anyhow::Result<u64> {
    let mut buf = [0u8; JOURNAL_HEADER_LEN];
    file.read_exact(&mut buf)
        .context("read journal file header")?;

    let magic: [u8; 4] = buf[..4].try_into().unwrap();
    if magic != JOURNAL_MAGIC {
        bail!(
            "journal magic mismatch: expected {:?}, got {:?}",
            JOURNAL_MAGIC,
            magic
        );
    }

    let version = u32::from_le_bytes(buf[4..8].try_into().unwrap());
    if version != JOURNAL_FORMAT_VERSION {
        bail!("unsupported journal format version {version} (expected {JOURNAL_FORMAT_VERSION})");
    }

    Ok(u64::from_le_bytes(buf[8..16].try_into().unwrap()))
}

struct ParsedFrameHeader {
    generation: u64,
    sequence: u64,
    payload_len: usize,
    body_checksum: u32,
}

enum HeaderRead {
    Complete(ParsedFrameHeader),
    IncompleteTail,
    Corrupt(anyhow::Error),
}

fn frame_header_valid(bytes: &[u8]) -> bool {
    if bytes.len() < FRAME_HEADER_LEN {
        return false;
    }
    if bytes[..4] != FRAME_MAGIC {
        return false;
    }
    let generation = u64::from_le_bytes(bytes[4..12].try_into().expect("generation"));
    let sequence = u64::from_le_bytes(bytes[12..20].try_into().expect("sequence"));
    let payload_len = u32::from_le_bytes(bytes[20..24].try_into().expect("len"));
    let header_seal = u32::from_le_bytes(bytes[28..32].try_into().expect("seal"));
    header_seal == frame_header_seal(generation, sequence, payload_len)
        && payload_len as usize <= MAX_FRAME_PAYLOAD
}

fn read_frame_header(file: &mut File, remaining: u64) -> HeaderRead {
    if remaining < FRAME_HEADER_LEN as u64 {
        return HeaderRead::IncompleteTail;
    }

    let mut buf = [0u8; FRAME_HEADER_LEN];
    if let Err(err) = file.read_exact(&mut buf) {
        return if err.kind() == std::io::ErrorKind::UnexpectedEof {
            HeaderRead::IncompleteTail
        } else {
            HeaderRead::Corrupt(anyhow::Error::new(err).context("read frame header"))
        };
    }

    let magic: [u8; 4] = buf[..4].try_into().unwrap();
    if magic != FRAME_MAGIC {
        return HeaderRead::Corrupt(anyhow::anyhow!("journal frame magic mismatch"));
    }

    let generation = u64::from_le_bytes(buf[4..12].try_into().unwrap());
    let sequence = u64::from_le_bytes(buf[12..20].try_into().unwrap());
    let payload_len = u32::from_le_bytes(buf[20..24].try_into().unwrap()) as usize;
    let body_checksum = u32::from_le_bytes(buf[24..28].try_into().unwrap());
    let header_seal = u32::from_le_bytes(buf[28..32].try_into().unwrap());

    let expected_seal = frame_header_seal(generation, sequence, payload_len as u32);
    if header_seal != expected_seal {
        return HeaderRead::Corrupt(anyhow::anyhow!(
            "journal frame header seal mismatch at generation {generation} sequence {sequence}"
        ));
    }

    HeaderRead::Complete(ParsedFrameHeader {
        generation,
        sequence,
        payload_len,
        body_checksum,
    })
}

fn encode_frame(generation: u64, sequence: u64, payload: &[u8]) -> anyhow::Result<Vec<u8>> {
    let payload_len = payload.len() as u32;
    let header_seal = frame_header_seal(generation, sequence, payload_len);
    let body_checksum = frame_body_checksum(generation, sequence, payload_len, payload);
    let mut frame = Vec::with_capacity(FRAME_HEADER_LEN + payload.len());
    frame.extend_from_slice(&FRAME_MAGIC);
    frame.extend_from_slice(&generation.to_le_bytes());
    frame.extend_from_slice(&sequence.to_le_bytes());
    frame.extend_from_slice(&payload_len.to_le_bytes());
    frame.extend_from_slice(&body_checksum.to_le_bytes());
    frame.extend_from_slice(&header_seal.to_le_bytes());
    frame.extend_from_slice(payload);
    Ok(frame)
}

#[cfg(test)]
#[path = "tests/journal.rs"]
mod tests;
