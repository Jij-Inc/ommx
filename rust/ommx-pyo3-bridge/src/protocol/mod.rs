//! Private bridge protocol implementations.

// Runtime names shared by senders and receiver registration. The enclosing
// private module keeps these details out of the consumer API.
pub const SUPPORTED_PROTOCOLS: &str = "_bridge_supported_protocols";
pub const ROOT_CLASSES: [&str; 4] = ["Instance", "ParametricInstance", "Solution", "SampleSet"];
pub const FROM_V1_BYTES: &str = "from_v1_bytes";
pub const FROM_V2_BYTES: &str = "from_v2_bytes";
pub const V2_FUNCTION: &str = "_bridge_protobuf_v2_function_from_bytes";
pub const V2_CONSTRAINT: &str = "_bridge_protobuf_v2_constraint_from_bytes";
pub const V2_DECISION_VARIABLE: &str = "_bridge_protobuf_v2_decision_variable_from_bytes";
pub const V1_FUNCTION: &str = "_bridge_protobuf_v1_function_from_bytes";
pub const V1_CONSTRAINT: &str = "_bridge_protobuf_v1_constraint_from_bytes";
pub const V1_DECISION_VARIABLE: &str = "_bridge_protobuf_v1_decision_variable_from_bytes";
