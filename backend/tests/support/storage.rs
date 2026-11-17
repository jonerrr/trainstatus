use backend::storage::store::Storage;
use std::path::{Path, PathBuf};
use uuid::Uuid;

use super::{TestOwners, test_owners};

/// On-disk root; dropped only when the owning [`TestContext`] ends.
struct TestStorage {
    path: PathBuf,
}

impl TestStorage {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("trainstatus-it-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&path).expect("test storage directory");
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn open(&self) -> Storage {
        Storage::open(self.path()).expect("test storage should open")
    }
}

impl Drop for TestStorage {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// Keeps an isolated storage root alive for the whole test scope (parallel-safe, unique per instance).
pub struct TestContext {
    root: TestStorage,
}

impl TestContext {
    pub fn new() -> Self {
        Self {
            root: TestStorage::new(),
        }
    }

    pub fn path(&self) -> &Path {
        self.root.path()
    }

    pub fn storage(&self) -> Storage {
        self.root.open()
    }

    pub fn owners(&self) -> TestOwners {
        test_owners(self.storage())
    }
}
