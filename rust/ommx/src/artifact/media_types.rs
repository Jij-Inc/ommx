use oci_spec::image::MediaType;

use anyhow::{bail, Result};

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
pub const V2_INSTANCE_MEDIA_TYPE: &str = "application/org.ommx.v2.instance";
pub const V2_PARAMETRIC_INSTANCE_MEDIA_TYPE: &str = "application/org.ommx.v2.parametric-instance";
pub const V2_SOLUTION_MEDIA_TYPE: &str = "application/org.ommx.v2.solution";
pub const V2_SAMPLE_SET_MEDIA_TYPE: &str = "application/org.ommx.v2.sample-set";
pub const TRACE_OTLP_PROTOBUF_MEDIA_TYPE: &str = "application/org.ommx.trace.otlp+protobuf";
pub const DIAGNOSTIC_MSGPACK_MEDIA_TYPE: &str = "application/org.ommx.diagnostic+msgpack";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RootPayloadVersion {
    V1,
    V2,
}

fn root_payload_version(
    media_type: &MediaType,
    v1: &'static str,
    v2: &'static str,
) -> Result<RootPayloadVersion> {
    let actual = media_type.as_ref();
    match actual {
        value if value == v1 => Ok(RootPayloadVersion::V1),
        value if value == v2 => Ok(RootPayloadVersion::V2),
        _ => bail!("Expected media type '{v1}' or '{v2}', got '{actual}'"),
    }
}

pub(crate) fn instance_payload_version(media_type: &MediaType) -> Result<RootPayloadVersion> {
    root_payload_version(media_type, V1_INSTANCE_MEDIA_TYPE, V2_INSTANCE_MEDIA_TYPE)
}

pub(crate) fn parametric_instance_payload_version(
    media_type: &MediaType,
) -> Result<RootPayloadVersion> {
    root_payload_version(
        media_type,
        V1_PARAMETRIC_INSTANCE_MEDIA_TYPE,
        V2_PARAMETRIC_INSTANCE_MEDIA_TYPE,
    )
}

pub(crate) fn solution_payload_version(media_type: &MediaType) -> Result<RootPayloadVersion> {
    root_payload_version(media_type, V1_SOLUTION_MEDIA_TYPE, V2_SOLUTION_MEDIA_TYPE)
}

pub(crate) fn sample_set_payload_version(media_type: &MediaType) -> Result<RootPayloadVersion> {
    root_payload_version(
        media_type,
        V1_SAMPLE_SET_MEDIA_TYPE,
        V2_SAMPLE_SET_MEDIA_TYPE,
    )
}

/// Whether the media type stores an [`crate::Instance`] root payload.
pub fn is_instance_payload_media_type(media_type: &MediaType) -> bool {
    instance_payload_version(media_type).is_ok()
}

/// Whether the media type stores a [`crate::ParametricInstance`] root payload.
pub fn is_parametric_instance_payload_media_type(media_type: &MediaType) -> bool {
    parametric_instance_payload_version(media_type).is_ok()
}

/// Whether the media type stores a [`crate::Solution`] root payload.
pub fn is_solution_payload_media_type(media_type: &MediaType) -> bool {
    solution_payload_version(media_type).is_ok()
}

/// Whether the media type stores a [`crate::SampleSet`] root payload.
pub fn is_sample_set_payload_media_type(media_type: &MediaType) -> bool {
    sample_set_payload_version(media_type).is_ok()
}

/// Media type of [crate::artifact::LocalArtifact], `application/org.ommx.v1.artifact`
pub fn v1_artifact() -> MediaType {
    MediaType::Other(V1_ARTIFACT_MEDIA_TYPE.to_string())
}

/// Media type of the `config` storing [crate::artifact::Config], `application/org.ommx.v1.config+json`
pub fn v1_config() -> MediaType {
    MediaType::Other(V1_CONFIG_MEDIA_TYPE.to_string())
}

/// Media type of the layer storing [crate::Instance] as v1 protobuf, with
/// descriptor annotations projected from domain metadata,
/// `application/org.ommx.v1.instance`
pub fn v1_instance() -> MediaType {
    MediaType::Other(V1_INSTANCE_MEDIA_TYPE.to_string())
}

/// Media type of the layer storing [crate::ParametricInstance] as v1 protobuf,
/// with descriptor annotations projected from domain metadata,
/// `application/org.ommx.v1.parametric-instance`
pub fn v1_parametric_instance() -> MediaType {
    MediaType::Other(V1_PARAMETRIC_INSTANCE_MEDIA_TYPE.to_string())
}

/// Media type of the layer storing [crate::Solution] as v1 protobuf, with
/// descriptor annotations projected from domain metadata,
/// `application/org.ommx.v1.solution`
pub fn v1_solution() -> MediaType {
    MediaType::Other(V1_SOLUTION_MEDIA_TYPE.to_string())
}

/// Media type of the layer storing [crate::SampleSet] as v1 protobuf, with
/// descriptor annotations projected from domain metadata,
/// `application/org.ommx.v1.sample-set`
pub fn v1_sample_set() -> MediaType {
    MediaType::Other(V1_SAMPLE_SET_MEDIA_TYPE.to_string())
}

/// Media type of the layer storing [crate::Instance] as v2 protobuf,
/// `application/org.ommx.v2.instance`
pub fn v2_instance() -> MediaType {
    MediaType::Other(V2_INSTANCE_MEDIA_TYPE.to_string())
}

/// Media type of the layer storing [crate::ParametricInstance] as v2 protobuf,
/// `application/org.ommx.v2.parametric-instance`
pub fn v2_parametric_instance() -> MediaType {
    MediaType::Other(V2_PARAMETRIC_INSTANCE_MEDIA_TYPE.to_string())
}

/// Media type of the layer storing [crate::Solution] as v2 protobuf,
/// `application/org.ommx.v2.solution`
pub fn v2_solution() -> MediaType {
    MediaType::Other(V2_SOLUTION_MEDIA_TYPE.to_string())
}

/// Media type of the layer storing [crate::SampleSet] as v2 protobuf,
/// `application/org.ommx.v2.sample-set`
pub fn v2_sample_set() -> MediaType {
    MediaType::Other(V2_SAMPLE_SET_MEDIA_TYPE.to_string())
}

/// Media type of an Experiment Run trace encoded as OTLP protobuf.
pub fn trace_otlp_protobuf() -> MediaType {
    MediaType::Other(TRACE_OTLP_PROTOBUF_MEDIA_TYPE.to_string())
}

/// Media type of an adapter diagnostic encoded as MessagePack.
pub fn diagnostic_msgpack() -> MediaType {
    MediaType::Other(DIAGNOSTIC_MSGPACK_MEDIA_TYPE.to_string())
}
