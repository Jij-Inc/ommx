//! Crate-internal streaming layer storage used by Experiment attachments.
//!
//! The parent `registry` module is crate-visible, so these public functions are
//! available to the Experiment owner without adding methods to the public
//! [`LocalRegistry`] API.

use super::{LocalRegistry, StoredDescriptor};
use anyhow::{Context, Result};
use oci_spec::image::{DescriptorBuilder, MediaType};
use std::{collections::HashMap, io::Read};

/// Stream an OCI layer into a Local Registry content-addressed blob store.
/// The descriptor digest and stored size are computed from the bytes read.
pub fn store_layer_reader<'reg>(
    registry: &'reg LocalRegistry,
    media_type: MediaType,
    reader: impl Read,
    annotations: HashMap<String, String>,
) -> Result<StoredDescriptor<'reg>> {
    let (digest, size) = registry.store_blob_reader(reader)?;
    let descriptor = DescriptorBuilder::default()
        .media_type(media_type)
        .digest(digest)
        .size(size)
        .annotations(annotations)
        .build()
        .context("Failed to build layer descriptor")?;
    Ok(StoredDescriptor {
        registry,
        descriptor,
    })
}
