//! Experiment and run scoped Record descriptor helpers.

use super::{ANN_RECORD_NAME, ANN_RUN_ID, ANN_SPACE};
use crate::artifact::local_registry::{LocalRegistry, StoredDescriptor};
use anyhow::Result;
use oci_spec::image::MediaType;
use std::collections::HashMap;

/// The storage space a Record descriptor belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordSpace {
    /// Shared by the whole experiment (dataset, source problem, ...).
    Experiment,
    /// Owned by a single run.
    Run(u64),
}

impl RecordSpace {
    fn as_str(self) -> &'static str {
        match self {
            RecordSpace::Experiment => "experiment",
            RecordSpace::Run(_) => "run",
        }
    }

    fn run_id(self) -> Option<u64> {
        match self {
            RecordSpace::Experiment => None,
            RecordSpace::Run(run_id) => Some(run_id),
        }
    }
}

/// OCI layer media type for JSON record payloads.
const JSON_MEDIA_TYPE: &str = "application/json";

/// Write `bytes` to the registry's BlobStore and build the in-memory
/// Record descriptor.
pub fn store_record_descriptor<'reg>(
    registry: &'reg LocalRegistry,
    space: RecordSpace,
    name: &str,
    media_type: MediaType,
    bytes: &[u8],
) -> Result<StoredDescriptor<'reg>> {
    let mut annotations = HashMap::new();
    annotations.insert(ANN_SPACE.to_string(), space.as_str().to_string());
    if let Some(run_id) = space.run_id() {
        annotations.insert(ANN_RUN_ID.to_string(), run_id.to_string());
    }
    annotations.insert(ANN_RECORD_NAME.to_string(), name.to_string());

    registry.store_layer_blob(media_type, bytes, annotations)
}

pub fn json_media_type() -> MediaType {
    MediaType::Other(JSON_MEDIA_TYPE.to_string())
}

pub fn encode_json(name: &str, value: impl serde::Serialize) -> Result<Vec<u8>> {
    serde_json::to_vec(&value)
        .map_err(|e| crate::error!("Failed to encode JSON record `{name}`: {e}"))
}

pub fn record_name(descriptor: &oci_spec::image::Descriptor) -> Option<&str> {
    descriptor
        .annotations()
        .as_ref()
        .and_then(|annotations| annotations.get(ANN_RECORD_NAME))
        .map(String::as_str)
}
