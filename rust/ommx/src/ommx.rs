/// Decison variable which mathematical programming solver will optimize
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DecisionVariable {
    /// Unique identifier of the decision variable
    #[prost(uint64, tag = "1")]
    pub id: u64,
    /// Kind of the decision variable
    #[prost(enumeration = "decision_variable::Kind", tag = "2")]
    pub kind: i32,
    /// Name of the decision variable
    #[prost(string, optional, tag = "3")]
    pub name: ::core::option::Option<::prost::alloc::string::String>,
    #[prost(uint64, repeated, tag = "4")]
    pub subscripts: ::prost::alloc::vec::Vec<u64>,
}
/// Nested message and enum types in `DecisionVariable`.
pub mod decision_variable {
    /// Kind of the decision variable
    #[derive(
        Clone,
        Copy,
        Debug,
        PartialEq,
        Eq,
        Hash,
        PartialOrd,
        Ord,
        ::prost::Enumeration
    )]
    #[repr(i32)]
    pub enum Kind {
        Binary = 0,
        Integer = 1,
        Real = 2,
    }
    impl Kind {
        /// String value of the enum field names used in the ProtoBuf definition.
        ///
        /// The values are not transformed in any way and thus are considered stable
        /// (if the ProtoBuf definition does not change) and safe for programmatic use.
        pub fn as_str_name(&self) -> &'static str {
            match self {
                Kind::Binary => "Binary",
                Kind::Integer => "Integer",
                Kind::Real => "Real",
            }
        }
        /// Creates an enum from field names used in the ProtoBuf definition.
        pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
            match value {
                "Binary" => Some(Self::Binary),
                "Integer" => Some(Self::Integer),
                "Real" => Some(Self::Real),
                _ => None,
            }
        }
    }
}
/// A monomial in a multivariate polynomial.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Monomial {
    #[prost(uint64, repeated, tag = "1")]
    pub ids: ::prost::alloc::vec::Vec<u64>,
    #[prost(fixed64, tag = "2")]
    pub coefficient: u64,
}
/// Multi­variate polynomial
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Polynomial {
    #[prost(message, repeated, tag = "1")]
    pub terms: ::prost::alloc::vec::Vec<Monomial>,
}
