//! Manage messages as container
//!

use crate::{error::*, v1};
use ocipkg::ImageName;
use std::path::Path;

pub trait ArtifactMessage: Sized {
    fn save(&self, image_name: &ImageName) -> Result<()>;
    fn save_as_archive(&self, path: &Path) -> Result<()>;
    fn load(image_name: &ImageName) -> Result<Self>;
}

impl ArtifactMessage for v1::Instance {
    fn save(&self, image_name: &ImageName) -> Result<()> {
        todo!()
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
