use oci_spec::image::MediaType;

pub const OCI_IMAGE_MANIFEST_MEDIA_TYPE: &str = "application/vnd.oci.image.manifest.v1+json";

/// Media type of the OCI 1.1 "empty descriptor" used as the `config`
/// blob for OMMX Image Manifests. See
/// <https://github.com/opencontainers/image-spec/blob/main/manifest.md#guidance-for-an-empty-descriptor>.
pub const OCI_EMPTY_CONFIG_MEDIA_TYPE: &str = "application/vnd.oci.empty.v1+json";
/// Body of the OCI empty descriptor: the two-byte JSON document `{}`.
pub const OCI_EMPTY_CONFIG_BYTES: &[u8] = b"{}";
/// SHA-256 digest of [`OCI_EMPTY_CONFIG_BYTES`]. Hard-coded so the
/// constant can be used in `const` contexts without re-hashing.
pub const OCI_EMPTY_CONFIG_DIGEST: &str =
    "sha256:44136fa355b3678a1146ad16f7e8649e94fb4fc21fe77e8310c060f61caaff8a";

pub const V1_ARTIFACT_MEDIA_TYPE: &str = "application/org.ommx.v1.artifact";
pub const V1_CONFIG_MEDIA_TYPE: &str = "application/org.ommx.v1.config+json";
pub const V1_INSTANCE_MEDIA_TYPE: &str = "application/org.ommx.v1.instance";
pub const V1_PARAMETRIC_INSTANCE_MEDIA_TYPE: &str = "application/org.ommx.v1.parametric-instance";
pub const V1_SOLUTION_MEDIA_TYPE: &str = "application/org.ommx.v1.solution";
pub const V1_SAMPLE_SET_MEDIA_TYPE: &str = "application/org.ommx.v1.sample-set";

/// Media type of [crate::artifact::LocalArtifact], `application/org.ommx.v1.artifact`
pub fn v1_artifact() -> MediaType {
    MediaType::Other(V1_ARTIFACT_MEDIA_TYPE.to_string())
}

/// Media type of the `config` storing [crate::artifact::Config], `application/org.ommx.v1.config+json`
pub fn v1_config() -> MediaType {
    MediaType::Other(V1_CONFIG_MEDIA_TYPE.to_string())
}

/// Media type of the layer storing [crate::v1::Instance] with [crate::artifact::InstanceAnnotations], `application/org.ommx.v1.instance`
pub fn v1_instance() -> MediaType {
    MediaType::Other(V1_INSTANCE_MEDIA_TYPE.to_string())
}

/// Media type of the layer storing [crate::v1::ParametricInstance] with [crate::artifact::InstanceAnnotations], `application/org.ommx.v1.parametric-instance`
pub fn v1_parametric_instance() -> MediaType {
    MediaType::Other(V1_PARAMETRIC_INSTANCE_MEDIA_TYPE.to_string())
}

/// Media type of the layer storing [crate::v1::Solution] with [crate::artifact::SolutionAnnotations], `application/org.ommx.v1.solution`
pub fn v1_solution() -> MediaType {
    MediaType::Other(V1_SOLUTION_MEDIA_TYPE.to_string())
}

/// Media type of the layer storing [crate::v1::SampleSet] with [crate::artifact::SolutionAnnotations], `application/org.ommx.v1.sample-set`
pub fn v1_sample_set() -> MediaType {
    MediaType::Other(V1_SAMPLE_SET_MEDIA_TYPE.to_string())
}
