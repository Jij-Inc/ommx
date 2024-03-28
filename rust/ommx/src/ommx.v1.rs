/// Decison variable which mathematical programming solver will optimize.
/// It must have its kind, i.e. binary, integer, real or others and unique identifier of 64-bit integer.
/// It may have its name and subscripts which are used to identify in modeling tools.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DecisionVariable {
    /// Unique identifier of the decision variable.
    #[prost(uint64, tag = "1")]
    pub id: u64,
    /// Kind of the decision variable
    #[prost(enumeration = "decision_variable::Kind", tag = "2")]
    pub kind: i32,
    /// This is optional since the name and subscripts does not exist in general mathematical programming situation
    #[prost(message, optional, tag = "3")]
    pub description: ::core::option::Option<decision_variable::Description>,
}
/// Nested message and enum types in `DecisionVariable`.
pub mod decision_variable {
    /// Human readable description of the decision variable.
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct Description {
        /// Name of the decision variable.
        #[prost(string, tag = "1")]
        pub name: ::prost::alloc::string::String,
        /// The subscripts of a deicision variable which is defined as multi-dimensional array.
        /// Empty list means that the decision variable is scalar
        #[prost(uint64, repeated, tag = "2")]
        pub subscripts: ::prost::alloc::vec::Vec<u64>,
    }
    /// Kind of the decision variable
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
    #[repr(i32)]
    pub enum Kind {
        Unspecified = 0,
        Binary = 1,
        Integer = 2,
        Real = 3,
    }
    impl Kind {
        /// String value of the enum field names used in the ProtoBuf definition.
        ///
        /// The values are not transformed in any way and thus are considered stable
        /// (if the ProtoBuf definition does not change) and safe for programmatic use.
        pub fn as_str_name(&self) -> &'static str {
            match self {
                Kind::Unspecified => "KIND_UNSPECIFIED",
                Kind::Binary => "KIND_BINARY",
                Kind::Integer => "KIND_INTEGER",
                Kind::Real => "KIND_REAL",
            }
        }
        /// Creates an enum from field names used in the ProtoBuf definition.
        pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
            match value {
                "KIND_UNSPECIFIED" => Some(Self::Unspecified),
                "KIND_BINARY" => Some(Self::Binary),
                "KIND_INTEGER" => Some(Self::Integer),
                "KIND_REAL" => Some(Self::Real),
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
/// Real-valued multivariate function used for objective function and constraints.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Function {
    #[prost(enumeration = "function::Kind", tag = "1")]
    pub kind: i32,
    #[prost(double, optional, tag = "2")]
    pub constant: ::core::option::Option<f64>,
    /// optional Linear linear = 3;
    /// optional Quadratic quadratic = 4;
    #[prost(message, optional, tag = "5")]
    pub polynomial: ::core::option::Option<Polynomial>,
}
/// Nested message and enum types in `Function`.
pub mod function {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
    #[repr(i32)]
    pub enum Kind {
        Unspecified = 0,
        Constant = 1,
        Linear = 2,
        Quadratic = 3,
        Polynomial = 4,
    }
    impl Kind {
        /// String value of the enum field names used in the ProtoBuf definition.
        ///
        /// The values are not transformed in any way and thus are considered stable
        /// (if the ProtoBuf definition does not change) and safe for programmatic use.
        pub fn as_str_name(&self) -> &'static str {
            match self {
                Kind::Unspecified => "KIND_UNSPECIFIED",
                Kind::Constant => "KIND_CONSTANT",
                Kind::Linear => "KIND_LINEAR",
                Kind::Quadratic => "KIND_QUADRATIC",
                Kind::Polynomial => "KIND_POLYNOMIAL",
            }
        }
        /// Creates an enum from field names used in the ProtoBuf definition.
        pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
            match value {
                "KIND_UNSPECIFIED" => Some(Self::Unspecified),
                "KIND_CONSTANT" => Some(Self::Constant),
                "KIND_LINEAR" => Some(Self::Linear),
                "KIND_QUADRATIC" => Some(Self::Quadratic),
                "KIND_POLYNOMIAL" => Some(Self::Polynomial),
                _ => None,
            }
        }
    }
}
