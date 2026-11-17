use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// `fsync` a file by path (opens read-only if it already exists).
pub fn fsync_file(path: &Path) -> io::Result<()> {
    let file = OpenOptions::new().read(true).open(path)?;
    file.sync_all()
}

/// `fsync` the parent directory of `path` so renames and new dentries are durable.
pub fn fsync_parent(path: &Path) -> io::Result<()> {
    let Some(parent) = path.parent() else {
        return Ok(());
    };
    let parent = if parent.as_os_str().is_empty() {
        Path::new(".")
    } else {
        parent
    };
    File::open(parent)?.sync_all()
}

/// Durably create missing directories without traversing above an existing parent.
pub fn create_dir_durable(path: &Path) -> io::Result<()> {
    if path.as_os_str().is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "empty directory path",
        ));
    }
    match fs::create_dir(path) {
        Ok(()) => {
            File::open(path)?.sync_all()?;
            fsync_parent(path)
        }
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists && path.is_dir() => {
            // Another owner may have just created this directory but not synced its entry.
            fsync_parent(path)
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let parent = path
                .parent()
                .filter(|parent| !parent.as_os_str().is_empty())
                .unwrap_or_else(|| Path::new("."));
            create_dir_durable(parent)?;
            create_dir_durable(path)
        }
        Err(error) => Err(error),
    }
}

/// Write `contents` to `path` by creating a sibling `*.part` file, syncing, then renaming.
pub fn replace_file(path: &Path, contents: &[u8]) -> io::Result<()> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));

    create_dir_durable(&parent)?;

    let file_name = path
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "path has no file name"))?;
    // Different owners can publish the same immutable name concurrently.
    let (temp, mut file) = loop {
        let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let temp = parent.join(format!(
            ".{}.{}-{sequence}.part",
            file_name.to_string_lossy(),
            std::process::id()
        ));
        match OpenOptions::new().write(true).create_new(true).open(&temp) {
            Ok(file) => break (temp, file),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    };
    let result = (|| {
        file.write_all(contents)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temp, path)?;
        fsync_parent(path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(temp);
    }
    result
}

/// Truncate `path` to `len` bytes and sync the file (journal incomplete-tail recovery).
pub fn truncate_durable(path: &Path, len: u64) -> io::Result<()> {
    let file = OpenOptions::new().write(true).open(path)?;
    file.set_len(len)?;
    file.sync_all()
}
