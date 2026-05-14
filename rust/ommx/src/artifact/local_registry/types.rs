pub const SQLITE_INDEX_FILE_NAME: &str = "index.sqlite3";
pub const FILE_BLOB_STORE_DIR_NAME: &str = "blobs";
pub const OCI_IMAGE_REF_NAME_ANNOTATION: &str = "org.opencontainers.image.ref.name";

use oci_spec::image::Descriptor;

#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefRecord {
    pub name: String,
    pub reference: String,
    pub descriptor: Descriptor,
    pub updated_at: String,
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
