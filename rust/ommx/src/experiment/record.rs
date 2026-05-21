//! Experiment and run scoped Record references.

use super::{ANN_RECORD_NAME, ANN_RUN_ID, ANN_SPACE};
use crate::artifact::local_registry::{LocalRegistry, StoredDescriptor};
use anyhow::{Context, Result};
use oci_spec::image::{Descriptor, MediaType};
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

/// A named reference to a Record payload.
///
/// The descriptor is an OCI descriptor stored in an Experiment config
/// or produced immediately after writing the payload to the Local
/// Registry BlobStore. `RecordRef` keeps the record name as a field so
/// callers do not need to inspect descriptor annotations for the common
/// lookup / display path.
#[derive(Debug, Clone)]
pub struct RecordRef<'reg> {
    name: String,
    descriptor: StoredDescriptor<'reg>,
}

impl<'reg> RecordRef<'reg> {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn descriptor(&self) -> &StoredDescriptor<'reg> {
        &self.descriptor
    }

    pub fn from_descriptor(registry: &'reg LocalRegistry, descriptor: Descriptor) -> Result<Self> {
        let name = descriptor
            .annotations()
            .as_ref()
            .and_then(|annotations| annotations.get(ANN_RECORD_NAME))
            .with_context(|| format!("Record descriptor is missing `{ANN_RECORD_NAME}`"))?
            .to_string();
        let descriptor = registry.stored_descriptor(descriptor)?;
        Ok(Self { name, descriptor })
    }

    fn from_stored_descriptor(name: String, descriptor: StoredDescriptor<'reg>) -> Self {
        Self { name, descriptor }
    }

    pub fn media_type(&self) -> String {
        self.descriptor.media_type().to_string()
    }
}

/// A set of Records that belong to one fixed experiment/run space.
#[derive(Debug, Clone, Default)]
pub struct RecordSet<'reg> {
    records: Vec<RecordRef<'reg>>,
}

impl<'reg> RecordSet<'reg> {
    pub fn new() -> Self {
        Self {
            records: Vec::new(),
        }
    }

    /// A record with the same `(media_type, name)` replaces the
    /// previous one. Space and run id are fixed by the owner of this
    /// collection.
    pub fn upsert(&mut self, record_ref: RecordRef<'reg>) {
        if let Some(existing) = self.records.iter_mut().find(|r| {
            r.descriptor().media_type() == record_ref.descriptor().media_type()
                && r.name() == record_ref.name()
        }) {
            *existing = record_ref;
        } else {
            self.records.push(record_ref);
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = &RecordRef<'reg>> {
        self.records.iter()
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
    Ok(RecordRef::from_stored_descriptor(
        name.to_string(),
        descriptor,
    ))
}

pub fn json_media_type() -> MediaType {
    MediaType::Other(JSON_MEDIA_TYPE.to_string())
}

pub fn encode_json(name: &str, value: impl serde::Serialize) -> Result<Vec<u8>> {
    serde_json::to_vec(&value)
        .map_err(|e| crate::error!("Failed to encode JSON record `{name}`: {e}"))
}
