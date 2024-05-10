//! Manage messages as container
//!

mod media_type;
pub use media_type::*;

use ocipkg::image::{Image, OciArtifact};
use std::ops::{Deref, DerefMut};

/// OCI Artifact of artifact type [`application/vnd.ommx.v1.artifact`][v1_artifact]
pub struct Artifact<Base: Image> {
    base: OciArtifact<Base>,
}

impl<Base: Image> Deref for Artifact<Base> {
    type Target = OciArtifact<Base>;
    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl<Base: Image> DerefMut for Artifact<Base> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}
