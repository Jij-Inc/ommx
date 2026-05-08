#![allow(dead_code)]

use super::{
    digest::sha256_digest,
    local_registry::{LocalRegistry, RefConflictPolicy, RefUpdate},
    media_types,
};
use anyhow::{Context, Result};
use oci_spec::image::{
    ArtifactManifest, ArtifactManifestBuilder as OciArtifactManifestBuilder, Descriptor,
    DescriptorBuilder, Digest, MediaType,
};
use serde::Serialize;
use std::{collections::HashMap, str::FromStr};

pub const OCI_ARTIFACT_MANIFEST_MEDIA_TYPE: &str = "application/vnd.oci.artifact.manifest.v1+json";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PendingArtifactBlob {
    descriptor: Descriptor,
    bytes: Vec<u8>,
}

impl PendingArtifactBlob {
    pub(crate) fn new(
        media_type: MediaType,
        bytes: Vec<u8>,
        annotations: HashMap<String, String>,
    ) -> Result<Self> {
        let descriptor = descriptor_from_bytes(media_type, &bytes, annotations)?;
        Ok(Self { descriptor, bytes })
    }

    pub(crate) fn descriptor(&self) -> &Descriptor {
        &self.descriptor
    }

    pub(crate) fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LocalArtifactBuild {
    manifest_descriptor: Descriptor,
    ref_update: RefUpdate,
}

impl LocalArtifactBuild {
    pub(crate) fn manifest_descriptor(&self) -> &Descriptor {
        &self.manifest_descriptor
    }

    pub(crate) fn ref_update(&self) -> &RefUpdate {
        &self.ref_update
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LocalArtifactBuilder {
    artifact_type: MediaType,
    blobs: Vec<PendingArtifactBlob>,
    subject: Option<Descriptor>,
    annotations: HashMap<String, String>,
}

impl LocalArtifactBuilder {
    pub(crate) fn new(artifact_type: MediaType) -> Self {
        Self {
            artifact_type,
            blobs: Vec::new(),
            subject: None,
            annotations: HashMap::new(),
        }
    }

    pub(crate) fn new_ommx() -> Self {
        Self::new(MediaType::Other(
            media_types::V1_ARTIFACT_MEDIA_TYPE.to_string(),
        ))
    }

    pub(crate) fn add_blob_bytes(
        &mut self,
        media_type: MediaType,
        bytes: Vec<u8>,
        annotations: HashMap<String, String>,
    ) -> Result<Descriptor> {
        let blob = PendingArtifactBlob::new(media_type, bytes, annotations)?;
        let descriptor = blob.descriptor.clone();
        self.blobs.push(blob);
        Ok(descriptor)
    }

    pub(crate) fn set_subject(&mut self, subject: Descriptor) -> &mut Self {
        self.subject = Some(subject);
        self
    }

    pub(crate) fn insert_annotation(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.annotations.insert(key.into(), value.into());
    }

    pub(crate) fn build_local(
        self,
        registry: &LocalRegistry,
        image_name: &ocipkg::ImageName,
        policy: RefConflictPolicy,
    ) -> Result<LocalArtifactBuild> {
        let prepared = self.prepare()?;
        let ref_update = registry.publish_artifact_manifest(
            image_name,
            &prepared.manifest,
            &prepared.manifest_descriptor,
            &prepared.manifest_bytes,
            &prepared.blobs,
            policy,
        )?;
        Ok(LocalArtifactBuild {
            manifest_descriptor: prepared.manifest_descriptor,
            ref_update,
        })
    }

    fn prepare(self) -> Result<PreparedArtifactManifest> {
        let mut builder = OciArtifactManifestBuilder::default()
            .artifact_type(self.artifact_type)
            .blobs(
                self.blobs
                    .iter()
                    .map(|blob| blob.descriptor.clone())
                    .collect::<Vec<_>>(),
            );
        if let Some(subject) = self.subject {
            builder = builder.subject(subject);
        }
        if !self.annotations.is_empty() {
            builder = builder.annotations(self.annotations);
        }
        let manifest = builder
            .build()
            .context("Failed to build OCI artifact manifest")?;
        let manifest_bytes = stable_json_bytes(&manifest)?;
        let manifest_descriptor =
            descriptor_from_bytes(MediaType::ArtifactManifest, &manifest_bytes, HashMap::new())?;
        Ok(PreparedArtifactManifest {
            manifest,
            manifest_bytes,
            manifest_descriptor,
            blobs: self.blobs,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PreparedArtifactManifest {
    manifest: ArtifactManifest,
    manifest_bytes: Vec<u8>,
    manifest_descriptor: Descriptor,
    blobs: Vec<PendingArtifactBlob>,
}

pub(crate) fn descriptor_from_bytes(
    media_type: MediaType,
    bytes: &[u8],
    annotations: HashMap<String, String>,
) -> Result<Descriptor> {
    let digest = Digest::from_str(&sha256_digest(bytes)).context("Failed to parse blob digest")?;
    let mut builder = DescriptorBuilder::default()
        .media_type(media_type)
        .digest(digest)
        .size(bytes.len() as u64);
    if !annotations.is_empty() {
        builder = builder.annotations(annotations);
    }
    builder.build().context("Failed to build OCI descriptor")
}

pub(crate) fn stable_json_bytes(value: &impl Serialize) -> Result<Vec<u8>> {
    let mut value = serde_json::to_value(value).context("Failed to encode JSON value")?;
    value.sort_all_objects();
    serde_json::to_vec(&value).context("Failed to encode stable JSON bytes")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_native_oci_artifact_manifest() -> Result<()> {
        let mut builder = LocalArtifactBuilder::new_ommx();
        let blob = builder.add_blob_bytes(
            MediaType::Other(media_types::V1_INSTANCE_MEDIA_TYPE.to_string()),
            b"instance".to_vec(),
            HashMap::from([("org.ommx.v1.instance.title".to_string(), "demo".to_string())]),
        )?;
        builder.insert_annotation("org.opencontainers.image.ref.name", "example.com/demo:v1");

        let prepared = builder.prepare()?;
        assert_eq!(prepared.manifest.media_type(), &MediaType::ArtifactManifest);
        assert_eq!(
            prepared.manifest.artifact_type(),
            &MediaType::Other(media_types::V1_ARTIFACT_MEDIA_TYPE.to_string())
        );
        assert_eq!(prepared.manifest.blobs(), &[blob]);
        assert_eq!(
            prepared.manifest_descriptor.media_type(),
            &MediaType::ArtifactManifest
        );
        assert_eq!(
            prepared.manifest_descriptor.digest().to_string(),
            sha256_digest(&prepared.manifest_bytes)
        );

        let parsed: ArtifactManifest = serde_json::from_slice(&prepared.manifest_bytes)?;
        assert_eq!(parsed, prepared.manifest);
        Ok(())
    }

    #[test]
    fn stable_manifest_json_is_independent_of_annotation_insertion_order() -> Result<()> {
        let first = prepared_with_annotations([("b", "2"), ("a", "1")])?;
        let second = prepared_with_annotations([("a", "1"), ("b", "2")])?;

        assert_eq!(first.manifest_bytes, second.manifest_bytes);
        assert_eq!(
            first.manifest_descriptor.digest(),
            second.manifest_descriptor.digest()
        );
        Ok(())
    }

    #[test]
    fn builds_manifest_with_subject() -> Result<()> {
        let subject = descriptor_from_bytes(
            MediaType::ArtifactManifest,
            b"parent manifest",
            HashMap::new(),
        )?;
        let mut builder = LocalArtifactBuilder::new_ommx();
        builder.add_blob_bytes(
            MediaType::Other(media_types::V1_INSTANCE_MEDIA_TYPE.to_string()),
            b"instance".to_vec(),
            HashMap::new(),
        )?;
        builder.set_subject(subject.clone());

        let prepared = builder.prepare()?;
        assert_eq!(prepared.manifest.subject(), &Some(subject));
        Ok(())
    }

    #[test]
    fn rejects_invalid_descriptor_digest_through_oci_spec() {
        assert!(Digest::from_str("sha256:../bad").is_err());
    }

    fn prepared_with_annotations(
        annotations: impl IntoIterator<Item = (&'static str, &'static str)>,
    ) -> Result<PreparedArtifactManifest> {
        let mut builder = LocalArtifactBuilder::new_ommx();
        builder.add_blob_bytes(
            MediaType::Other(media_types::V1_INSTANCE_MEDIA_TYPE.to_string()),
            b"instance".to_vec(),
            HashMap::new(),
        )?;
        for (key, value) in annotations {
            builder.insert_annotation(key, value);
        }
        builder.prepare()
    }
}
