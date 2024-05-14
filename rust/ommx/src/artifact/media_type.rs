use ocipkg::oci_spec::image::MediaType;

/// Media type `application/org.ommx.v1.artifact` used to store OMMX components
pub fn v1_artifact() -> MediaType {
    MediaType::Other("application/org.ommx.v1.artifact".to_string())
}

/// Media type for OMMX instance stored in OCI Artifact as a layer, `application/org.ommx.v1.instance+protobuf`
pub fn v1_instance() -> MediaType {
    MediaType::Other("application/org.ommx.v1.instance+protobuf".to_string())
}

/// Media type for OMMX solution stored in OCI Artifact as a layer, `application/org.ommx.v1.solution+protobuf`
pub fn v1_solution() -> MediaType {
    MediaType::Other("application/org.ommx.v1.solution+protobuf".to_string())
}
