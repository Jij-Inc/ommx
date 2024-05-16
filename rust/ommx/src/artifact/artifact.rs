use crate::{
    artifact::{media_type, Config, InstanceAnnotations, SolutionAnnotations},
    v1,
};
use anyhow::{bail, Result};
use ocipkg::{
    image::{Image, OciArtifact},
    Digest,
};
use prost::Message;
use std::ops::{Deref, DerefMut};

/// OMMX Artifact, an OCI Artifact of type [`application/org.ommx.v1.artifact`][v1_artifact]
pub struct Artifact<Base: Image>(OciArtifact<Base>);

impl<Base: Image> Deref for Artifact<Base> {
    type Target = OciArtifact<Base>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<Base: Image> DerefMut for Artifact<Base> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<Base: Image> Artifact<Base> {
    pub(crate) fn new(artifact: OciArtifact<Base>) -> Self {
        Self(artifact)
    }

    pub fn get_config(&mut self) -> Result<Config> {
        let (_desc, blob) = self.0.get_config()?;
        let config = serde_json::from_slice(&blob)?;
        Ok(config)
    }

    pub fn get_solution(&mut self, digest: &Digest) -> Result<(v1::Solution, SolutionAnnotations)> {
        for (desc, blob) in self.0.get_layers()? {
            if desc.media_type() != &media_type::v1_solution()
                || desc.digest() != &digest.to_string()
            {
                continue;
            }
            let solution = v1::Solution::decode(blob.as_slice())?;
            let annotations = if let Some(annotations) = desc.annotations() {
                SolutionAnnotations::try_from(annotations.clone())?
            } else {
                SolutionAnnotations::default()
            };
            return Ok((solution, annotations));
        }
        // TODO: Seek from other artifacts
        bail!("Solution of digest {} not found", digest)
    }

    pub fn get_instance(&mut self, digest: &Digest) -> Result<(v1::Instance, InstanceAnnotations)> {
        for (desc, blob) in self.0.get_layers()? {
            if desc.media_type() != &media_type::v1_instance()
                || desc.digest() != &digest.to_string()
            {
                continue;
            }
            let instance = v1::Instance::decode(blob.as_slice())?;
            let annotations = if let Some(annotations) = desc.annotations() {
                InstanceAnnotations::try_from(annotations.clone())?
            } else {
                InstanceAnnotations::default()
            };
            return Ok((instance, annotations));
        }
        bail!("Instance of digest {} not found", digest)
    }

    pub fn get_solutions(&mut self) -> Result<Vec<(v1::Solution, SolutionAnnotations)>> {
        let mut out = Vec::new();
        for (desc, blob) in self.0.get_layers()? {
            if desc.media_type() != &media_type::v1_solution() {
                continue;
            }
            let solution = v1::Solution::decode(blob.as_slice())?;
            let annotations = if let Some(annotations) = desc.annotations() {
                SolutionAnnotations::try_from(annotations.clone())?
            } else {
                SolutionAnnotations::default()
            };
            out.push((solution, annotations));
        }
        Ok(out)
    }

    pub fn get_instances(&mut self) -> Result<Vec<(v1::Instance, InstanceAnnotations)>> {
        let mut out = Vec::new();
        for (desc, blob) in self.0.get_layers()? {
            if desc.media_type() != &media_type::v1_instance() {
                continue;
            }
            let instance = v1::Instance::decode(blob.as_slice())?;
            let annotations = if let Some(annotations) = desc.annotations() {
                InstanceAnnotations::try_from(annotations.clone())?
            } else {
                InstanceAnnotations::default()
            };
            out.push((instance, annotations));
        }
        Ok(out)
    }
}
