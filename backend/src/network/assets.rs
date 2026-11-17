use std::{path::PathBuf, sync::Arc};

use anyhow::{Context, ensure};

use crate::geometry::Geom;
use crate::source::Source;
use crate::storage::content::ContentId;
use crate::storage::store::Storage;

/// Immutable, source-scoped geometry assets shared by catalogs and accepted operations.
#[derive(Clone, Debug)]
pub struct ShapeAssets {
    directory: Arc<PathBuf>,
}

impl ShapeAssets {
    pub fn open(storage: &Storage, source: Source) -> anyhow::Result<Self> {
        Self::open_root(storage.root(), source)
    }

    pub fn open_root(root: &std::path::Path, source: Source) -> anyhow::Result<Self> {
        let directory = root.join("network").join("shapes").join(source.as_str());
        crate::storage::sync::create_dir_durable(&directory)?;
        Ok(Self {
            directory: Arc::new(directory),
        })
    }

    pub fn put(&self, geometry: &Geom) -> anyhow::Result<ContentId> {
        // The codec envelope includes the format epoch, making the identity versioned.
        let bytes = crate::storage::codec::encode_to_file_bytes(geometry)?;
        let id = ContentId::hash(&bytes);
        let path = self.directory.join(format!("{id}.trst"));
        if !path.is_file() {
            crate::storage::codec::write_encoded_atomic(&path, &bytes)?;
        } else {
            ensure!(
                std::fs::metadata(&path)?.len() == bytes.len() as u64,
                "existing shape asset size mismatch: {}",
                path.display()
            );
            ensure!(
                std::fs::read(&path)? == bytes,
                "existing shape asset identity mismatch: {}",
                path.display()
            );
            crate::storage::sync::fsync_file(&path)?;
            crate::storage::sync::fsync_parent(&path)?;
        }
        Ok(id)
    }

    pub fn get(&self, id: ContentId) -> anyhow::Result<Geom> {
        Ok(self.get_weighted(id)?.0)
    }

    pub fn get_weighted(&self, id: ContentId) -> anyhow::Result<(Geom, usize)> {
        let path = self.directory.join(format!("{id}.trst"));
        let max = 16u64 + crate::storage::format::MAX_CODEC_UNCOMPRESSED as u64 + 16 * 1024 * 1024;
        ensure!(
            std::fs::metadata(&path)?.len() <= max,
            "shape asset exceeds codec size bound"
        );
        let bytes =
            std::fs::read(&path).with_context(|| format!("read shape asset {}", path.display()))?;
        ensure!(
            ContentId::hash(&bytes) == id,
            "shape asset identity mismatch: {}",
            path.display()
        );
        let geom: Geom = crate::storage::codec::decode_file_bytes(&bytes)?;
        let weight = geom.allocated_bytes().saturating_add(256);
        Ok((geom, weight))
    }
}
