use ocipkg::oci_spec::image::MediaType;

/// Media type of [crate::artifact::Artifact], `application/org.ommx.v1.artifact`
pub fn v1_artifact() -> MediaType {
    MediaType::Other("application/org.ommx.v1.artifact".to_string())
}

/// Media type of the `config` storing [crate::artifact::Config], `application/org.ommx.v1.config+json`
pub fn v1_config() -> MediaType {
    MediaType::Other("application/org.ommx.v1.config+json".to_string())
}

/// Media type of the layer storing [crate::v1::Instance] with [crate::artifact::InstanceAnnotations], `application/org.ommx.v1.instance`
pub fn v1_instance() -> MediaType {
    MediaType::Other("application/org.ommx.v1.instance".to_string())
}

/// Media type of the layer storing [crate::v1::ParametricInstance] with [crate::artifact::InstanceAnnotations], `application/org.ommx.v1.parametric-instance`
pub fn v1_parametric_instance() -> MediaType {
    MediaType::Other("application/org.ommx.v1.parametric-instance".to_string())
}

/// Media type of the layer storing [crate::v1::Solution] with [crate::artifact::SolutionAnnotations], `application/org.ommx.v1.solution`
pub fn v1_solution() -> MediaType {
    MediaType::Other("application/org.ommx.v1.solution".to_string())
}

/// Media type of the layer storing [crate::v1::SampleSet] with [crate::artifact::SolutionAnnotations], `application/org.ommx.v1.sample-set`
pub fn v1_sample_set() -> MediaType {
    MediaType::Other("application/org.ommx.v1.sample-set".to_string())
}
