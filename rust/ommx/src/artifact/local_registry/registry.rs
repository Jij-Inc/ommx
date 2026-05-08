use super::{
    import_legacy_local_registry_ref, migrate_legacy_local_registry,
    migrate_legacy_local_registry_with_policy, FileBlobStore, LegacyMigrationReport,
    LegacyOciDirImport, RefConflictPolicy, SqliteIndexStore,
};
use anyhow::Result;
use ocipkg::ImageName;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct LocalRegistry {
    root: PathBuf,
    index: SqliteIndexStore,
    blobs: FileBlobStore,
}

impl LocalRegistry {
    pub fn open(root: impl Into<PathBuf>) -> Result<Self> {
        let root = root.into();
        let index = SqliteIndexStore::open_in_registry_root(&root)?;
        let blobs = FileBlobStore::open_in_registry_root(&root)?;
        Ok(Self { root, index, blobs })
    }

    pub fn open_default() -> Result<Self> {
        Self::open(crate::artifact::get_local_registry_root())
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn index(&self) -> &SqliteIndexStore {
        &self.index
    }

    pub fn blobs(&self) -> &FileBlobStore {
        &self.blobs
    }

    pub fn import_legacy_ref(&self, image_name: &ImageName) -> Result<LegacyOciDirImport> {
        import_legacy_local_registry_ref(&self.index, &self.blobs, &self.root, image_name)
    }

    pub fn migrate_legacy_layout(&self) -> Result<LegacyMigrationReport> {
        migrate_legacy_local_registry(&self.index, &self.blobs, &self.root)
    }

    pub fn migrate_legacy_layout_with_policy(
        &self,
        policy: RefConflictPolicy,
    ) -> Result<LegacyMigrationReport> {
        migrate_legacy_local_registry_with_policy(&self.index, &self.blobs, &self.root, policy)
    }

    pub fn resolve_image_name(&self, image_name: &ImageName) -> Result<Option<String>> {
        self.index.resolve_image_name(image_name)
    }
}
