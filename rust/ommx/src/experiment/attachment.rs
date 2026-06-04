//! Experiment and run scoped Attachment descriptor helpers.

use super::{ANN_ATTACHMENT_NAME, ANN_RUN_ID, ANN_SPACE};
use crate::artifact::local_registry::{LocalRegistry, StoredDescriptor};
use anyhow::Result;
use oci_spec::image::MediaType;
use std::collections::HashMap;

/// The storage space an Attachment descriptor belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttachmentSpace {
    /// Shared by the whole experiment (dataset, source problem, ...).
    Experiment,
    /// Owned by a single run.
    Run(u64),
}

impl AttachmentSpace {
    fn as_str(self) -> &'static str {
        match self {
            AttachmentSpace::Experiment => "experiment",
            AttachmentSpace::Run(_) => "run",
        }
    }

    fn run_id(self) -> Option<u64> {
        match self {
            AttachmentSpace::Experiment => None,
            AttachmentSpace::Run(run_id) => Some(run_id),
        }
    }
}

/// OCI layer media type for JSON attachment payloads.
const JSON_MEDIA_TYPE: &str = "application/json";

/// Write `bytes` to the registry and build the in-memory Attachment descriptor.
pub fn store_attachment_descriptor<'reg>(
    registry: &'reg LocalRegistry,
    space: AttachmentSpace,
    name: &str,
    media_type: MediaType,
    bytes: &[u8],
) -> Result<StoredDescriptor<'reg>> {
    let mut annotations = HashMap::new();
    annotations.insert(ANN_SPACE.to_string(), space.as_str().to_string());
    if let Some(run_id) = space.run_id() {
        annotations.insert(ANN_RUN_ID.to_string(), run_id.to_string());
    }
    annotations.insert(ANN_ATTACHMENT_NAME.to_string(), name.to_string());

    registry.store_layer_blob(media_type, bytes, annotations)
}

pub fn json_media_type() -> MediaType {
    MediaType::from(JSON_MEDIA_TYPE)
}

pub fn encode_json(name: &str, value: impl serde::Serialize) -> Result<Vec<u8>> {
    crate::artifact::stable_json_bytes(&value)
        .map_err(|e| crate::error!("Failed to encode JSON attachment `{name}`: {e}"))
}

pub fn attachment_name(descriptor: &oci_spec::image::Descriptor) -> Option<&str> {
    descriptor
        .annotations()
        .as_ref()
        .and_then(|annotations| annotations.get(ANN_ATTACHMENT_NAME))
        .map(String::as_str)
}
