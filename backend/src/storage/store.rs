use std::fs::{self, File, OpenOptions};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, bail};

use super::format::LOCK_FILE_NAME;
use super::sync::create_dir_durable;

/// Process-wide storage root. Holds an exclusive lock for the root directory lifetime.
#[derive(Clone)]
pub struct Storage {
    root: Arc<PathBuf>,
    _lock: Arc<File>,
}

impl Storage {
    /// Open or create `path`, acquire an exclusive root lock, and return a cloneable handle.
    pub fn open(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let root = path.as_ref().to_path_buf();
        create_dir_durable(&root)
            .with_context(|| format!("create storage root {}", root.display()))?;

        let lock_path = root.join(LOCK_FILE_NAME);
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lock_path)
            .with_context(|| format!("open storage lock {}", lock_path.display()))?;

        match lock.try_lock() {
            Ok(()) => {}
            Err(std::fs::TryLockError::WouldBlock) => {
                bail!(
                    "storage root {} is already locked by another process",
                    root.display()
                );
            }
            Err(std::fs::TryLockError::Error(err)) => {
                return Err(err).with_context(|| format!("lock storage root {}", root.display()));
            }
        }

        cleanup_abandoned_publications(&root)?;

        Ok(Self {
            root: Arc::new(root),
            _lock: Arc::new(lock),
        })
    }

    pub fn root(&self) -> &Path {
        self.root.as_ref()
    }

    /// Return `root/{owner}`, creating the owner directory durably if missing.
    pub fn directory(&self, owner: &str) -> anyhow::Result<PathBuf> {
        validate_owner(owner)?;
        let dir = self.root.join(owner);
        create_dir_durable(&dir)
            .with_context(|| format!("create owner directory {}", dir.display()))?;
        Ok(dir)
    }
}

// Only the application's owner trees and the filenames its publishers generate are eligible.
fn cleanup_abandoned_publications(root: &Path) -> anyhow::Result<()> {
    for owner in ["network", "operations", "alerts"] {
        let path = root.join(owner);
        match fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.file_type().is_dir() => sweep_directory(root, &path)?,
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error).with_context(|| format!("inspect {}", path.display())),
        }
    }
    Ok(())
}

fn sweep_directory(root: &Path, directory: &Path) -> anyhow::Result<()> {
    let mut removed = false;
    let result = (|| -> anyhow::Result<()> {
        for entry in fs::read_dir(directory)
            .with_context(|| format!("read storage directory {}", directory.display()))?
        {
            let entry = entry?;
            let kind = entry.file_type()?;
            let path = entry.path();
            if kind.is_dir() {
                sweep_directory(root, &path)?;
            } else if kind.is_file()
                && entry
                    .file_name()
                    .to_str()
                    .is_some_and(|name| recognized_temporary(root, directory, name))
            {
                fs::remove_file(&path)
                    .with_context(|| format!("remove abandoned publication {}", path.display()))?;
                removed = true;
            }
        }
        Ok(())
    })();
    if removed {
        File::open(directory)
            .and_then(|dir| dir.sync_all())
            .with_context(|| format!("sync storage directory {}", directory.display()))?;
    }
    result
}

fn recognized_temporary(root: &Path, directory: &Path, name: &str) -> bool {
    let Some(stem) = name.strip_prefix('.').and_then(|n| n.strip_suffix(".part")) else {
        return false;
    };
    let Some((target, unique)) = stem.rsplit_once('.') else {
        return false;
    };
    let Some((pid, sequence)) = unique.split_once('-') else {
        return false;
    };
    if !decimal(pid) || !decimal(sequence) {
        return false;
    }
    let Ok(relative) = directory.strip_prefix(root) else {
        return false;
    };
    let mut parts = [""; 5];
    let mut len = 0;
    for part in relative.iter() {
        let Some(part) = part.to_str() else {
            return false;
        };
        if len == parts.len() {
            return false;
        }
        parts[len] = part;
        len += 1;
    }
    match &parts[..len] {
        ["operations", source, "blocks"] if source_name(source) => hex_digest(target),
        ["alerts", "records"] => hex_digest(target),
        ["operations", source] if source_name(source) => target == "journal",
        ["operations", source, "checkpoint" | "archive"] if source_name(source) => {
            target == "manifest.trst"
        }
        [
            "operations",
            source,
            "checkpoint",
            "generations",
            generation,
        ] if source_name(source) && decimal(generation) => target == "global.trst",
        ["network"] => target == "current.trst",
        ["network", "blobs"] | ["network", "shapes", _] => {
            target.strip_suffix(".trst").is_some_and(hex_digest)
        }
        ["network", "publications", _] => target == "publication.trst",
        ["network", "publications", _, "sources", source] if source_name(source) => {
            target == "manifest.trst"
        }
        ["alerts"] => matches!(target, "journal" | "manifest.cbor.zst"),
        ["alerts", "sources", source] if source_name(source) => target
            .strip_prefix("index-g")
            .and_then(|n| n.strip_suffix(".cbor.zst"))
            .is_some_and(decimal),
        ["alerts", "sources", source, "pages"] if source_name(source) => alert_page(target),
        _ => false,
    }
}

fn decimal(value: &str) -> bool {
    !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit())
}

fn hex_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn source_name(value: &str) -> bool {
    matches!(value, "mta_bus" | "mta_subway" | "njt_bus")
}

fn alert_page(value: &str) -> bool {
    let Some(stem) = value.strip_suffix(".cbor.zst") else {
        return false;
    };
    let Some((bucket, rest)) = stem.split_once("-g") else {
        return false;
    };
    let Some((generation, number)) = rest.split_once('-') else {
        return false;
    };
    bucket.len() == 2
        && bucket
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        && decimal(generation)
        && decimal(number)
}

fn validate_owner(owner: &str) -> anyhow::Result<()> {
    if owner.is_empty() {
        bail!("storage owner name must not be empty");
    }
    if owner == "." || owner == ".." {
        bail!("storage owner name must not be . or ..");
    }
    if owner.contains('/') || owner.contains('\\') {
        bail!("storage owner name must not contain path separators");
    }
    if owner.contains('\0') {
        bail!("storage owner name must not contain NUL");
    }
    if owner == LOCK_FILE_NAME {
        bail!("storage owner name must not be the root lock file");
    }
    Ok(())
}
