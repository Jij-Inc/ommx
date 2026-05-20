//! Experiment and run scoped Record references.

use super::{ANN_RECORD_NAME, ANN_RUN_ID, ANN_SPACE};
use crate::artifact::local_registry::{LocalRegistry, StoredDescriptor};
use anyhow::Result;
use oci_spec::image::MediaType;
use std::collections::HashMap;

/// The storage space a [`RecordRef`] belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordSpace {
    /// Shared by the whole experiment (dataset, source problem, ...).
    Experiment,
    /// Owned by a single run.
    Run,
}

impl RecordSpace {
    fn as_str(self) -> &'static str {
        match self {
            RecordSpace::Experiment => "experiment",
            RecordSpace::Run => "run",
        }
    }
}

/// OCI layer media type for JSON record payloads.
const JSON_MEDIA_TYPE: &str = "application/json";

/// A named reference to a payload that has already been written to the
/// BlobStore.
#[derive(Debug, Clone)]
pub struct RecordRef<'reg> {
    name: String,
    /// OCI layer descriptor whose payload bytes are present in the
    /// Local Registry BlobStore. Carries the payload media type and
    /// the experiment / record annotations.
    descriptor: StoredDescriptor<'reg>,
}

impl<'reg> RecordRef<'reg> {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn descriptor(&self) -> &StoredDescriptor<'reg> {
        &self.descriptor
    }
}

/// Build-phase upsert: a record with the same `(media_type, name)`
/// within a space replaces the previous one. Within one `Vec` the
/// space and `run_id` are already fixed, so `(media_type, name)` is
/// the remaining key.
pub fn upsert_record_ref<'reg>(records: &mut Vec<RecordRef<'reg>>, record_ref: RecordRef<'reg>) {
    if let Some(existing) = records.iter_mut().find(|r| {
        r.descriptor().media_type() == record_ref.descriptor().media_type()
            && r.name() == record_ref.name()
    }) {
        *existing = record_ref;
    } else {
        records.push(record_ref);
    }
}

/// Write `bytes` to the registry's BlobStore and build the in-memory
/// [`RecordRef`].
pub fn store_record_ref<'reg>(
    registry: &'reg LocalRegistry,
    space: RecordSpace,
    run_id: Option<u64>,
    name: &str,
    media_type: MediaType,
    bytes: &[u8],
) -> Result<RecordRef<'reg>> {
    let mut annotations = HashMap::new();
    annotations.insert(ANN_SPACE.to_string(), space.as_str().to_string());
    if let Some(run_id) = run_id {
        annotations.insert(ANN_RUN_ID.to_string(), run_id.to_string());
    }
    annotations.insert(ANN_RECORD_NAME.to_string(), name.to_string());

    let descriptor = registry.store_layer_blob(media_type, bytes, annotations)?;
    Ok(RecordRef {
        name: name.to_string(),
        descriptor,
    })
}

pub fn json_media_type() -> MediaType {
    MediaType::Other(JSON_MEDIA_TYPE.to_string())
}

pub fn encode_json(name: &str, value: impl serde::Serialize) -> Result<Vec<u8>> {
    serde_json::to_vec(&value)
        .map_err(|e| crate::error!("Failed to encode JSON record `{name}`: {e}"))
}
