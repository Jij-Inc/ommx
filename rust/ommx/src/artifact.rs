//! Manage messages as container
//!

use crate::{error::*, v1};
use ocipkg::ImageName;
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
        dbg!(image_name);
        todo!()
    }

    fn load(image_name: &ImageName) -> Result<Self> {
        dbg!(image_name);
        todo!()
    }
}
