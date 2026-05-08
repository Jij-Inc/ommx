use oci_spec::image::MediaType;

pub const V1_ARTIFACT_MEDIA_TYPE: &str = "application/org.ommx.v1.artifact";
pub const V1_CONFIG_MEDIA_TYPE: &str = "application/org.ommx.v1.config+json";
pub const V1_INSTANCE_MEDIA_TYPE: &str = "application/org.ommx.v1.instance";
pub const V1_PARAMETRIC_INSTANCE_MEDIA_TYPE: &str = "application/org.ommx.v1.parametric-instance";
pub const V1_SOLUTION_MEDIA_TYPE: &str = "application/org.ommx.v1.solution";
pub const V1_SAMPLE_SET_MEDIA_TYPE: &str = "application/org.ommx.v1.sample-set";

/// Media type of [crate::artifact::Artifact], `application/org.ommx.v1.artifact`
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
