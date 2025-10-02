use anyhow::{bail, Result};
use ocipkg::{
    image::{OciArchiveBuilder, OciDirBuilder},
    ImageName,
};
use ommx::artifact::Builder;
use pyo3::{prelude::*, types::PyBytes};
use std::{collections::HashMap, path::PathBuf};

use crate::{ArtifactArchive, ArtifactDir, PyDescriptor};

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pyclass)]
#[pyclass]
#[pyo3(module = "ommx._ommx_rust")]
pub struct ArtifactArchiveBuilder(Option<Builder<OciArchiveBuilder>>);

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pymethods)]
#[pymethods]
impl ArtifactArchiveBuilder {
    #[staticmethod]
    pub fn new_unnamed(path: PathBuf) -> Result<Self> {
        let builder = Builder::new_archive_unnamed(path)?;
        Ok(Self(Some(builder)))
    }

    #[staticmethod]
    pub fn new(path: PathBuf, image_name: &str) -> Result<Self> {
        let image_name = ImageName::parse(image_name)?;
        let builder = Builder::new_archive(path, image_name)?;
        Ok(Self(Some(builder)))
    }

    #[staticmethod]
    pub fn temp() -> Result<Self> {
        let builder = Builder::temp_archive()?;
        Ok(Self(Some(builder)))
    }

    #[staticmethod]
    pub fn new_for_local_registry(image_name: &str) -> Result<Self> {
        let image_name = ImageName::parse(image_name)?;
        let builder = Builder::new_for_local_registry(image_name)?;
        Ok(Self(Some(builder)))
    }

    pub fn add_layer(
        &mut self,
        media_type: &str,
        blob: Bound<PyBytes>,
        annotations: HashMap<String, String>,
    ) -> Result<PyDescriptor> {
        if let Some(builder) = self.0.as_mut() {
            let desc = builder.add_layer(media_type.into(), blob.as_bytes(), annotations)?;
            Ok(PyDescriptor::from(desc))
        } else {
            bail!("Already built artifact")
        }
    }

    pub fn add_annotation(&mut self, key: &str, value: &str) -> Result<()> {
        if let Some(builder) = self.0.as_mut() {
            builder.add_annotation(key.into(), value.into());
            Ok(())
        } else {
            bail!("Already built artifact")
        }
    }

    pub fn build(&mut self) -> Result<ArtifactArchive> {
        if let Some(builder) = self.0.take() {
            let artifact = builder.build()?;
            Ok(ArtifactArchive::from(artifact))
        } else {
            bail!("Already built artifact")
        }
    }
}

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pyclass)]
#[pyclass]
#[pyo3(module = "ommx._ommx_rust")]
pub struct ArtifactDirBuilder(Option<Builder<OciDirBuilder>>);

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pymethods)]
#[pymethods]
impl ArtifactDirBuilder {
    #[staticmethod]
    pub fn new(image_name: &str) -> Result<Self> {
        let image_name = ImageName::parse(image_name)?;
        let builder = Builder::new(image_name)?;
        Ok(Self(Some(builder)))
    }

    #[staticmethod]
    pub fn for_github(org: &str, repo: &str, name: &str, tag: &str) -> Result<Self> {
        let builder = Builder::for_github(org, repo, name, tag)?;
        Ok(Self(Some(builder)))
    }
}

    pub fn add_layer(
        &mut self,
        media_type: &str,
        blob: Bound<PyBytes>,
        annotations: HashMap<String, String>,
    ) -> Result<PyDescriptor> {
        if let Some(builder) = self.0.as_mut() {
            let desc = builder.add_layer(media_type.into(), blob.as_bytes(), annotations)?;
            Ok(PyDescriptor::from(desc))
        } else {
            bail!("Already built artifact")
        }
    }

    pub fn add_annotation(&mut self, key: &str, value: &str) -> Result<()> {
        if let Some(builder) = self.0.as_mut() {
            builder.add_annotation(key.into(), value.into());
            Ok(())
        } else {
            bail!("Already built artifact")
        }
    }

    pub fn build(&mut self) -> Result<ArtifactDir> {
        if let Some(builder) = self.0.take() {
            let artifact = builder.build()?;
            Ok(ArtifactDir::from(artifact))
        } else {
            bail!("Already built artifact")
        }
    }
}
