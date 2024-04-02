#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Linear {
    #[prost(message, repeated, tag = "1")]
    pub terms: ::prost::alloc::vec::Vec<linear::Term>,
}
/// Nested message and enum types in `Linear`.
pub mod linear {
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct Term {
        #[prost(uint64, tag = "1")]
        pub id: u64,
        #[prost(double, tag = "2")]
        pub coefficient: f64,
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
/// COO format of sparse matrix. The components must have same size.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SparseMatrix {
    #[prost(uint64, repeated, tag = "1")]
    pub rows: ::prost::alloc::vec::Vec<u64>,
    #[prost(uint64, repeated, tag = "2")]
    pub columns: ::prost::alloc::vec::Vec<u64>,
    #[prost(double, repeated, tag = "3")]
    pub values: ::prost::alloc::vec::Vec<f64>,
}
/// Quadratic function as a sparse matrix and linear sparse vector.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Quadratic {
    #[prost(message, optional, tag = "1")]
    pub quadradic: ::core::option::Option<SparseMatrix>,
    #[prost(message, optional, tag = "2")]
    pub linear: ::core::option::Option<Linear>,
}
/// Real-valued multivariate function used for objective function and constraints.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Function {
    #[prost(oneof = "function::Function", tags = "1, 2, 3, 4")]
    pub function: ::core::option::Option<function::Function>,
}
/// Nested message and enum types in `Function`.
pub mod function {
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Function {
        /// Constant function like `f(x_1, x_2) = 2`
        #[prost(double, tag = "1")]
        Constant(f64),
        /// Linear function like `f(x_1, x_2) = 2 x_1 + 3 x_2`
        #[prost(message, tag = "2")]
        Linear(super::Linear),
        /// Quadratic function like `f(x_1, x_2) = 4 x_1 x_2 + 5 x_2`
        #[prost(message, tag = "3")]
        Quadratic(super::Quadratic),
        /// Polynomial like `f(x_1, x_2) = 4 x_1^2 + 5 x_2^3 + 6 x_1 x_2^2 + 7 x_2^2 + 8 x_1 x_2 + 9 x_1 + 10 x_2 + 11`
        #[prost(message, tag = "4")]
        Polynomial(super::Polynomial),
    }
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Constraint {
    #[prost(uint64, tag = "1")]
    pub id: u64,
    #[prost(enumeration = "constraint::Equality", tag = "2")]
    pub equality: i32,
    #[prost(message, optional, tag = "3")]
    pub function: ::core::option::Option<Function>,
    #[prost(message, optional, tag = "4")]
    pub description: ::core::option::Option<constraint::Description>,
}
/// Nested message and enum types in `Constraint`.
pub mod constraint {
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct Description {
        #[prost(string, tag = "1")]
        pub name: ::prost::alloc::string::String,
        #[prost(uint64, repeated, tag = "2")]
        pub forall: ::prost::alloc::vec::Vec<u64>,
    }
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
    #[repr(i32)]
    pub enum Equality {
        Unspecified = 0,
        EqualToZero = 1,
        LessThanOrEqualToZero = 2,
    }
    impl Equality {
        /// String value of the enum field names used in the ProtoBuf definition.
        ///
        /// The values are not transformed in any way and thus are considered stable
        /// (if the ProtoBuf definition does not change) and safe for programmatic use.
        pub fn as_str_name(&self) -> &'static str {
            match self {
                Equality::Unspecified => "EQUALITY_UNSPECIFIED",
                Equality::EqualToZero => "EQUALITY_EQUAL_TO_ZERO",
                Equality::LessThanOrEqualToZero => "EQUALITY_LESS_THAN_OR_EQUAL_TO_ZERO",
            }
        }
        /// Creates an enum from field names used in the ProtoBuf definition.
        pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
            match value {
                "EQUALITY_UNSPECIFIED" => Some(Self::Unspecified),
                "EQUALITY_EQUAL_TO_ZERO" => Some(Self::EqualToZero),
                "EQUALITY_LESS_THAN_OR_EQUAL_TO_ZERO" => Some(Self::LessThanOrEqualToZero),
                _ => None,
            }
        }
    }
}
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
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Instance {
    #[prost(message, optional, tag = "1")]
    pub description: ::core::option::Option<instance::Description>,
    #[prost(message, optional, tag = "2")]
    pub objective: ::core::option::Option<Function>,
    #[prost(message, repeated, tag = "3")]
    pub constraints: ::prost::alloc::vec::Vec<Constraint>,
}
/// Nested message and enum types in `Instance`.
pub mod instance {
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct Description {
        #[prost(string, optional, tag = "1")]
        pub name: ::core::option::Option<::prost::alloc::string::String>,
        #[prost(string, optional, tag = "2")]
        pub description: ::core::option::Option<::prost::alloc::string::String>,
        #[prost(string, repeated, tag = "3")]
        pub authors: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
        /// The application or library name that created this message.
        #[prost(string, optional, tag = "4")]
        pub created_by: ::core::option::Option<::prost::alloc::string::String>,
    }
}
/// A solution obtained by the solver.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Solution {
    #[prost(map = "uint64, double", tag = "1")]
    pub entries: ::std::collections::HashMap<u64, f64>,
}
/// List of solutions obtained by the solver.
/// This message is for supporting solvers that return multiple solutions.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SolutionList {
    #[prost(message, repeated, tag = "1")]
    pub solutions: ::prost::alloc::vec::Vec<Solution>,
}
