//! Private bridge protocol implementations.

// Runtime names shared by senders and receiver registration. The enclosing
// private module keeps these details out of the consumer API.
pub const SUPPORTED_PROTOCOLS: &str = "_bridge_supported_protocols";
pub const BRIDGE_ERROR: &str = "BridgeError";
pub const V1_INSTANCE: &str = "_bridge_protobuf_v1_instance_from_bytes";
pub const V1_PARAMETRIC_INSTANCE: &str = "_bridge_protobuf_v1_parametric_instance_from_bytes";
pub const V1_SOLUTION: &str = "_bridge_protobuf_v1_solution_from_bytes";
pub const V1_SAMPLE_SET: &str = "_bridge_protobuf_v1_sample_set_from_bytes";
pub const V2_INSTANCE: &str = "_bridge_protobuf_v2_instance_from_bytes";
pub const V2_PARAMETRIC_INSTANCE: &str = "_bridge_protobuf_v2_parametric_instance_from_bytes";
pub const V2_SOLUTION: &str = "_bridge_protobuf_v2_solution_from_bytes";
pub const V2_SAMPLE_SET: &str = "_bridge_protobuf_v2_sample_set_from_bytes";
pub const V2_FUNCTION: &str = "_bridge_protobuf_v2_function_from_bytes";
pub const V2_CONSTRAINT: &str = "_bridge_protobuf_v2_constraint_from_bytes";
pub const V2_DECISION_VARIABLE: &str = "_bridge_protobuf_v2_decision_variable_from_bytes";
pub const V1_FUNCTION: &str = "_bridge_protobuf_v1_function_from_bytes";
pub const V1_CONSTRAINT: &str = "_bridge_protobuf_v1_constraint_from_bytes";
pub const V1_DECISION_VARIABLE: &str = "_bridge_protobuf_v1_decision_variable_from_bytes";

/// Fixed transfer contracts, independent of SDK and protobuf version numbers.
/// IDs are never reused and their numeric order does not express preference.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
#[non_exhaustive]
pub enum TransferProtocolId {
    /// Legacy protobuf roots and complete, ID-bearing component messages.
    ProtobufV1 = 1,
    /// Normalized protobuf roots and detached component reconstruction.
    ProtobufV2 = 2,
}

impl std::fmt::Display for TransferProtocolId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::ProtobufV1 => "protobuf v1",
            Self::ProtobufV2 => "protobuf v2",
        })
    }
}
