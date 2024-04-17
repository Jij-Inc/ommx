//! Manage messages as container
//!

use crate::{error::*, v1};
use ocipkg::{
    oci_spec::image::{DescriptorBuilder, MediaType},
    ImageName,
};
use prost::Message;
use std::path::Path;

pub trait ArtifactMessage: Sized {
    fn save(&self, image_name: &ImageName) -> Result<()>;
    fn save_as_archive(&self, path: &Path) -> Result<()>;
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
        artifact.set_artifact_type(MediaType::Other(
            "application/vnd.ommx.v1.artifact".to_string(),
        ))?;
        artifact.finish()?;
        Ok(())
    }

    fn save_as_archive(&self, path: &Path) -> Result<()> {
        dbg!(path);
        todo!()
    }

    fn load(image_name: &ImageName) -> Result<Self> {
        dbg!(image_name);
        todo!()
    }
}
