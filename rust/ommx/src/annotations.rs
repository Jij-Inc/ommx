use std::collections::HashMap;

const RESERVED_PREFIX: &str = "org.ommx.v1.";

pub fn is_reserved_annotation_key(key: &str) -> bool {
    key.starts_with(RESERVED_PREFIX)
}

/// Keep protobuf extension maps free of reserved metadata keys, including when
/// callers have directly mutated a domain object's public annotation map.
pub fn protobuf_extension_annotations(
    annotations: HashMap<String, String>,
) -> HashMap<String, String> {
    annotations
        .into_iter()
        .filter(|(key, _)| !is_reserved_annotation_key(key))
        .collect()
}
