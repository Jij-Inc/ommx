pub const SQLITE_INDEX_FILE_NAME: &str = "index.sqlite3";
pub const FILE_BLOB_STORE_DIR_NAME: &str = "blobs";
pub const OCI_IMAGE_REF_NAME_ANNOTATION: &str = "org.opencontainers.image.ref.name";

pub const BLOB_KIND_BLOB: &str = "blob";
pub const BLOB_KIND_CONFIG: &str = "config";
pub const BLOB_KIND_LAYER: &str = "layer";
pub const BLOB_KIND_MANIFEST: &str = "manifest";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlobRecord {
    pub digest: String,
    pub size: u64,
    pub media_type: Option<String>,
    pub storage_uri: String,
    pub kind: String,
    pub last_verified_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestRecord {
    pub digest: String,
    pub media_type: String,
    pub size: u64,
    pub subject_digest: Option<String>,
    pub annotations_json: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefRecord {
    pub name: String,
    pub reference: String,
    pub manifest_digest: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayerRecord {
    pub manifest_digest: String,
    pub position: u32,
    pub digest: String,
    pub media_type: String,
    pub size: u64,
    pub annotations_json: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefConflictPolicy {
    KeepExisting,
    Replace,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefUpdate {
    Inserted,
    Unchanged,
    Replaced {
        previous_manifest_digest: String,
    },
    Conflicted {
        existing_manifest_digest: String,
        incoming_manifest_digest: String,
    },
}
