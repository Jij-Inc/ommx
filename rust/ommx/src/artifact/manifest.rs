use super::{
    digest::{sha256_digest, validate_digest},
    media_types,
};
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const OCI_IMAGE_MANIFEST_MEDIA_TYPE: &str = "application/vnd.oci.image.manifest.v1+json";
pub const OCI_EMPTY_JSON_MEDIA_TYPE: &str = "application/vnd.oci.empty.v1+json";

const OCI_MANIFEST_SCHEMA_VERSION: u32 = 2;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactDescriptor {
    media_type: String,
    digest: String,
    size: u64,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    annotations: BTreeMap<String, String>,
}

impl ArtifactDescriptor {
    pub fn new(
        media_type: impl Into<String>,
        digest: impl Into<String>,
        size: u64,
    ) -> Result<Self> {
        Self::with_annotations(media_type, digest, size, BTreeMap::new())
    }

    pub fn with_annotations(
        media_type: impl Into<String>,
        digest: impl Into<String>,
        size: u64,
        annotations: BTreeMap<String, String>,
    ) -> Result<Self> {
        let media_type = media_type.into();
        let digest = digest.into();
        ensure!(!media_type.is_empty(), "Descriptor media type is empty");
        validate_digest(&digest)?;
        Ok(Self {
            media_type,
            digest,
            size,
            annotations,
        })
    }

    pub fn from_bytes(
        media_type: impl Into<String>,
        bytes: &[u8],
        annotations: BTreeMap<String, String>,
    ) -> Self {
        Self {
            media_type: media_type.into(),
            digest: sha256_digest(bytes),
            size: bytes.len() as u64,
            annotations,
        }
    }

    pub fn media_type(&self) -> &str {
        &self.media_type
    }

    pub fn digest(&self) -> &str {
        &self.digest
    }

    pub fn size(&self) -> u64 {
        self.size
    }

    pub fn annotations(&self) -> &BTreeMap<String, String> {
        &self.annotations
    }

    fn validate(&self) -> Result<()> {
        ensure!(
            !self.media_type.is_empty(),
            "Descriptor media type is empty"
        );
        validate_digest(&self.digest)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactBlob {
    descriptor: ArtifactDescriptor,
    bytes: Vec<u8>,
}

impl ArtifactBlob {
    pub fn new(
        media_type: impl Into<String>,
        bytes: Vec<u8>,
        annotations: BTreeMap<String, String>,
    ) -> Self {
        let descriptor = ArtifactDescriptor::from_bytes(media_type, &bytes, annotations);
        Self { descriptor, bytes }
    }

    pub fn descriptor(&self) -> &ArtifactDescriptor {
        &self.descriptor
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactManifest {
    schema_version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    media_type: Option<String>,
    artifact_type: String,
    config: ArtifactDescriptor,
    layers: Vec<ArtifactDescriptor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    subject: Option<ArtifactDescriptor>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    annotations: BTreeMap<String, String>,
}

impl ArtifactManifest {
    pub fn from_json_bytes(bytes: &[u8]) -> Result<Self> {
        let manifest: Self =
            serde_json::from_slice(bytes).context("Failed to parse artifact manifest JSON")?;
        manifest.validate()?;
        Ok(manifest)
    }

    pub fn to_json_bytes(&self) -> Result<Vec<u8>> {
        self.validate()?;
        serde_json::to_vec(self).context("Failed to encode artifact manifest JSON")
    }

    pub fn schema_version(&self) -> u32 {
        self.schema_version
    }

    pub fn media_type(&self) -> Option<&str> {
        self.media_type.as_deref()
    }

    pub fn artifact_type(&self) -> &str {
        &self.artifact_type
    }

    pub fn config(&self) -> &ArtifactDescriptor {
        &self.config
    }

    pub fn layers(&self) -> &[ArtifactDescriptor] {
        &self.layers
    }

    pub fn subject(&self) -> Option<&ArtifactDescriptor> {
        self.subject.as_ref()
    }

    pub fn annotations(&self) -> &BTreeMap<String, String> {
        &self.annotations
    }

    fn validate(&self) -> Result<()> {
        ensure!(
            self.schema_version == OCI_MANIFEST_SCHEMA_VERSION,
            "Unsupported OCI manifest schema version: {}",
            self.schema_version
        );
        if let Some(media_type) = &self.media_type {
            ensure!(
                media_type == OCI_IMAGE_MANIFEST_MEDIA_TYPE,
                "Unsupported OCI manifest media type: {media_type}"
            );
        }
        ensure!(!self.artifact_type.is_empty(), "Artifact type is empty");
        self.config
            .validate()
            .context("Invalid config descriptor")?;
        for (position, layer) in self.layers.iter().enumerate() {
            layer
                .validate()
                .with_context(|| format!("Invalid layer descriptor at position {position}"))?;
        }
        if let Some(subject) = &self.subject {
            subject.validate().context("Invalid subject descriptor")?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuiltArtifactManifest {
    manifest: ArtifactManifest,
    manifest_bytes: Vec<u8>,
    manifest_descriptor: ArtifactDescriptor,
    config: ArtifactBlob,
    layers: Vec<ArtifactBlob>,
}

impl BuiltArtifactManifest {
    pub fn manifest(&self) -> &ArtifactManifest {
        &self.manifest
    }

    pub fn manifest_bytes(&self) -> &[u8] {
        &self.manifest_bytes
    }

    pub fn manifest_descriptor(&self) -> &ArtifactDescriptor {
        &self.manifest_descriptor
    }

    pub fn config(&self) -> &ArtifactBlob {
        &self.config
    }

    pub fn layers(&self) -> &[ArtifactBlob] {
        &self.layers
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactManifestBuilder {
    artifact_type: String,
    config: Option<ArtifactBlob>,
    layers: Vec<ArtifactBlob>,
    subject: Option<ArtifactDescriptor>,
    annotations: BTreeMap<String, String>,
}

impl ArtifactManifestBuilder {
    pub fn new(artifact_type: impl Into<String>) -> Self {
        Self {
            artifact_type: artifact_type.into(),
            config: None,
            layers: Vec::new(),
            subject: None,
            annotations: BTreeMap::new(),
        }
    }

    pub fn new_ommx() -> Self {
        Self::new(media_types::V1_ARTIFACT_MEDIA_TYPE)
    }

    pub fn add_config_bytes(
        &mut self,
        media_type: impl Into<String>,
        bytes: Vec<u8>,
        annotations: BTreeMap<String, String>,
    ) -> ArtifactDescriptor {
        let blob = ArtifactBlob::new(media_type, bytes, annotations);
        let descriptor = blob.descriptor.clone();
        self.config = Some(blob);
        descriptor
    }

    pub fn add_empty_config(&mut self) -> ArtifactDescriptor {
        self.add_config_bytes(OCI_EMPTY_JSON_MEDIA_TYPE, b"{}".to_vec(), BTreeMap::new())
    }

    pub fn add_layer_bytes(
        &mut self,
        media_type: impl Into<String>,
        bytes: Vec<u8>,
        annotations: BTreeMap<String, String>,
    ) -> ArtifactDescriptor {
        let blob = ArtifactBlob::new(media_type, bytes, annotations);
        let descriptor = blob.descriptor.clone();
        self.layers.push(blob);
        descriptor
    }

    pub fn set_subject(&mut self, subject: ArtifactDescriptor) -> &mut Self {
        self.subject = Some(subject);
        self
    }

    pub fn insert_annotation(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.annotations.insert(key.into(), value.into());
    }

    pub fn build(self) -> Result<BuiltArtifactManifest> {
        let config = self.config.context("Artifact manifest config is not set")?;
        let manifest = ArtifactManifest {
            schema_version: OCI_MANIFEST_SCHEMA_VERSION,
            media_type: Some(OCI_IMAGE_MANIFEST_MEDIA_TYPE.to_string()),
            artifact_type: self.artifact_type,
            config: config.descriptor.clone(),
            layers: self
                .layers
                .iter()
                .map(|layer| layer.descriptor.clone())
                .collect(),
            subject: self.subject,
            annotations: self.annotations,
        };
        let manifest_bytes = manifest.to_json_bytes()?;
        let manifest_descriptor = ArtifactDescriptor::from_bytes(
            OCI_IMAGE_MANIFEST_MEDIA_TYPE,
            &manifest_bytes,
            BTreeMap::new(),
        );
        Ok(BuiltArtifactManifest {
            manifest,
            manifest_bytes,
            manifest_descriptor,
            config,
            layers: self.layers,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_and_reads_ommx_artifact_manifest() -> Result<()> {
        let mut builder = ArtifactManifestBuilder::new_ommx();
        builder.add_config_bytes(
            media_types::V1_CONFIG_MEDIA_TYPE,
            b"{}".to_vec(),
            BTreeMap::new(),
        );
        let layer = builder.add_layer_bytes(
            media_types::V1_INSTANCE_MEDIA_TYPE,
            b"instance".to_vec(),
            BTreeMap::from([("org.ommx.v1.instance.title".to_string(), "demo".to_string())]),
        );
        builder.insert_annotation("org.opencontainers.image.ref.name", "example.com/demo:v1");

        let built = builder.build()?;
        assert_eq!(
            built.manifest_descriptor().media_type(),
            OCI_IMAGE_MANIFEST_MEDIA_TYPE
        );
        assert_eq!(
            built.manifest_descriptor().digest(),
            sha256_digest(built.manifest_bytes()).as_str()
        );

        let parsed = ArtifactManifest::from_json_bytes(built.manifest_bytes())?;
        assert_eq!(parsed.artifact_type(), media_types::V1_ARTIFACT_MEDIA_TYPE);
        assert_eq!(parsed.media_type(), Some(OCI_IMAGE_MANIFEST_MEDIA_TYPE));
        assert_eq!(parsed.layers(), &[layer]);
        assert_eq!(
            parsed
                .annotations()
                .get("org.opencontainers.image.ref.name"),
            Some(&"example.com/demo:v1".to_string())
        );
        Ok(())
    }

    #[test]
    fn rejects_invalid_descriptor_digest() {
        assert!(ArtifactDescriptor::new("application/octet-stream", "sha256:../bad", 1).is_err());
    }

    #[test]
    fn reads_manifest_without_optional_top_level_media_type() -> Result<()> {
        let config =
            ArtifactDescriptor::from_bytes(OCI_EMPTY_JSON_MEDIA_TYPE, b"{}", BTreeMap::new());
        let manifest = serde_json::json!({
            "schemaVersion": 2,
            "artifactType": media_types::V1_ARTIFACT_MEDIA_TYPE,
            "config": config,
            "layers": []
        });

        let parsed = ArtifactManifest::from_json_bytes(&serde_json::to_vec(&manifest)?)?;
        assert_eq!(parsed.media_type(), None);
        assert_eq!(parsed.config(), &config);
        Ok(())
    }
}
