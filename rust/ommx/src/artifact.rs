//! Manage messages as container
//!

use crate::{error::*, v1};
use ocipkg::{oci_spec::image::DescriptorBuilder, ImageName};
use prost::Message;
use serde::*;

/// The version of OMMX schema of the message stored in the artifact
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Version {
    V1,
}

/// Kind of the message stored in the artifact
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Kind {
    Instance,
    Solution,
}

pub fn get_artifact_type(image_name: &ImageName) -> Result<(Version, Kind)> {
    dbg!(image_name);
    todo!()
}

pub trait ArtifactMessage: Sized {
    fn save(&self, image_name: &ImageName) -> Result<()>;
    fn load(image_name: &ImageName) -> Result<Self>;
}

impl ArtifactMessage for v1::Instance {
    fn save(&self, image_name: &ImageName) -> Result<()> {
        let blob = self.encode_to_vec();
        let mut artifact = ocipkg::image::LocalArtifactBuilder::new(image_name.clone())?;
        let descriptor = DescriptorBuilder::default()
            .media_type("application/vnd.ommx.v1.instance+protobuf")
            .build()?; // size and digest are set by `add_blob`
        artifact.add_blob(descriptor, &blob)?;
        artifact.finish()?;
        Ok(())
    }

    fn load(image_name: &ImageName) -> Result<Self> {
        dbg!(image_name);
        todo!()
    }
}
