// This file is @generated by prost-build.
#[non_exhaustive]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Linear {
    #[prost(message, repeated, tag = "1")]
    pub terms: ::prost::alloc::vec::Vec<linear::Term>,
    #[prost(double, tag = "2")]
    pub constant: f64,
}
/// Nested message and enum types in `Linear`.
pub mod linear {
    #[non_exhaustive]
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
#[non_exhaustive]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Monomial {
    #[prost(uint64, repeated, tag = "1")]
    pub ids: ::prost::alloc::vec::Vec<u64>,
    #[prost(double, tag = "2")]
    pub coefficient: f64,
}
/// Multi­variate polynomial
#[non_exhaustive]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Polynomial {
    #[prost(message, repeated, tag = "1")]
    pub terms: ::prost::alloc::vec::Vec<Monomial>,
}
/// Quadratic function as a COO-style sparse matrix and linear sparse vector.
///
/// COOrdinate format, also known as triplet format, is a way to represent sparse matrices as a list of non-zero elements.
/// It consists of three lists: the row indices, the column indices, and the values of the non-zero elements with following constraints:
///
/// - Entries and coordinates sorted by row, then column.
/// - There are no duplicate entries (i.e. duplicate (i,j) locations)
/// - Data arrays MAY have explicit zeros.
///
/// Note that this matrix is not assured to be symmetric nor upper triangular.
/// For example, a quadratic function `x1^2 + x2^2 + 2x1*x2` can be represented as:
///
/// - `{ rows: \[0, 0, 1\], columns: \[0, 1, 1\], values: \[1, 2, 1\] }`, i.e. an upper triangular matrix `\[[1, 2\], \[0, 1\]`
/// - `{ rows: \[0, 0, 1, 1\], columns: \[0, 1, 0, 1\], values: \[1, 1, 1, 1\] }`, i.e. a symmetric matrix `\[[1, 1\], \[1, 1]\]`
///
/// or even a non-symmetric, non-trianglar matrix as `x1^2 + 3x1*x2 - x2*x1 + x2^2`:
///
/// - `{ rows: \[0, 0, 1, 1\], columns: \[0, 1, 0, 1\], values: \[1, 3, -1, 1\] }`, i.e. a non-symmetric matrix `\[[1, 3\], \[-1, 1]\]`
///
#[non_exhaustive]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Quadratic {
    #[prost(uint64, repeated, tag = "1")]
    pub rows: ::prost::alloc::vec::Vec<u64>,
    #[prost(uint64, repeated, tag = "2")]
    pub columns: ::prost::alloc::vec::Vec<u64>,
    #[prost(double, repeated, tag = "3")]
    pub values: ::prost::alloc::vec::Vec<f64>,
    #[prost(message, optional, tag = "4")]
    pub linear: ::core::option::Option<Linear>,
}
/// Real-valued multivariate function used for objective function and constraints.
#[non_exhaustive]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Function {
    #[prost(oneof = "function::Function", tags = "1, 2, 3, 4")]
    pub function: ::core::option::Option<function::Function>,
}
/// Nested message and enum types in `Function`.
pub mod function {
    #[non_exhaustive]
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
#[non_exhaustive]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Constraint {
    /// Constraint ID
    ///
    /// - Constraint IDs are managed separately from decision variable IDs.
    ///    We can use the same ID for both. For example, we have a decision variable `x` with decision variable ID `1``
    ///    and constraint `x == 0` with constraint ID `1`.
    /// - IDs are not required to be sequential.
    /// - IDs must be unique with other types of constraints.
    #[prost(uint64, tag = "1")]
    pub id: u64,
    #[prost(enumeration = "Equality", tag = "2")]
    pub equality: i32,
    #[prost(message, optional, tag = "3")]
    pub function: ::core::option::Option<Function>,
    /// Integer parameters of the constraint.
    ///
    /// Consider for example a problem constains a series of constraints `x\[i, j\] + y\[i, j\] <= 10` for `i = 1, 2, 3` and `j = 4, 5`,
    /// then 6 = 3x2 `Constraint` messages should be created corresponding to each pair of `i` and `j`.
    /// The `name` field of this message is intended to be a human-readable name of `x\[i, j\] + y\[i, j\] <= 10`,
    /// and the `subscripts` field is intended to be the value of `\[i, j\]` like `\[1, 5\]`.
    ///
    #[prost(int64, repeated, tag = "8")]
    pub subscripts: ::prost::alloc::vec::Vec<i64>,
    /// Key-value parameters of the constraint.
    #[prost(map = "string, string", tag = "5")]
    pub parameters:
        ::std::collections::HashMap<::prost::alloc::string::String, ::prost::alloc::string::String>,
    /// Name of the constraint.
    #[prost(string, optional, tag = "6")]
    pub name: ::core::option::Option<::prost::alloc::string::String>,
    /// Detail human-readable description of the constraint.
    #[prost(string, optional, tag = "7")]
    pub description: ::core::option::Option<::prost::alloc::string::String>,
}
/// A constraint evaluated with a state
#[non_exhaustive]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct EvaluatedConstraint {
    #[prost(uint64, tag = "1")]
    pub id: u64,
    #[prost(enumeration = "Equality", tag = "2")]
    pub equality: i32,
    /// The value of function for the state
    #[prost(double, tag = "3")]
    pub evaluated_value: f64,
    /// IDs of decision variables used to evalute this constraint
    #[prost(uint64, repeated, tag = "4")]
    pub used_decision_variable_ids: ::prost::alloc::vec::Vec<u64>,
    /// Integer parameters of the constraint.
    #[prost(int64, repeated, tag = "9")]
    pub subscripts: ::prost::alloc::vec::Vec<i64>,
    /// Key-value parameters of the constraint.
    #[prost(map = "string, string", tag = "5")]
    pub parameters:
        ::std::collections::HashMap<::prost::alloc::string::String, ::prost::alloc::string::String>,
    /// Name of the constraint.
    #[prost(string, optional, tag = "6")]
    pub name: ::core::option::Option<::prost::alloc::string::String>,
    /// Detail human-readable description of the constraint.
    #[prost(string, optional, tag = "7")]
    pub description: ::core::option::Option<::prost::alloc::string::String>,
    /// Value for the Lagrangian dual variable of this constraint.
    /// This is optional because not all solvers support to evaluate dual variables.
    #[prost(double, optional, tag = "8")]
    pub dual_variable: ::core::option::Option<f64>,
    /// Short removed reason of the constraint. This field exists only if this message is evaluated from a removed constraint.
    #[prost(string, optional, tag = "10")]
    pub removed_reason: ::core::option::Option<::prost::alloc::string::String>,
    /// Detailed parameters why the constraint is removed. This field exists only if this message is evaluated from a removed constraint.
    #[prost(map = "string, string", tag = "11")]
    pub removed_reason_parameters:
        ::std::collections::HashMap<::prost::alloc::string::String, ::prost::alloc::string::String>,
}
#[non_exhaustive]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RemovedConstraint {
    /// The removed constraint
    #[prost(message, optional, tag = "1")]
    pub constraint: ::core::option::Option<Constraint>,
    /// Short reason why the constraint was removed.
    ///
    /// This should be the name of method, function or application which remove the constraint.
    #[prost(string, tag = "2")]
    pub removed_reason: ::prost::alloc::string::String,
    /// Arbitrary key-value parameters representing why the constraint was removed.
    ///
    /// This should be human-readable and can be used for debugging.
    #[prost(map = "string, string", tag = "3")]
    pub removed_reason_parameters:
        ::std::collections::HashMap<::prost::alloc::string::String, ::prost::alloc::string::String>,
}
/// Equality of a constraint.
#[non_exhaustive]
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
/// A message representing a one-hot constraint.
#[non_exhaustive]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct OneHot {
    /// The ID of the constraint.
    #[prost(uint64, tag = "1")]
    pub constraint_id: u64,
    /// The list of ids of decision variables that are constrained to be one-hot.
    #[prost(uint64, repeated, tag = "2")]
    pub decision_variables: ::prost::alloc::vec::Vec<u64>,
}
/// A message representing a [Spcial Ordered Set constraint of Type 1](<https://en.wikipedia.org/wiki/Special_ordered_set#Types>) (SOS1).
/// SOS1 constraint on non-negative variables x_1, ..., x_n
/// requires that at most one of x_i can be non-zero.
#[non_exhaustive]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Sos1 {
    /// The ID of the SOS1 constraint on binary variables.
    #[prost(uint64, tag = "1")]
    pub binary_constraint_id: u64,
    /// The IDs of the big-M constraint on non-binary variables.
    #[prost(uint64, repeated, tag = "2")]
    pub big_m_constraint_ids: ::prost::alloc::vec::Vec<u64>,
    /// The list of ids of decision variables that are constrained to be one-hot.
    #[prost(uint64, repeated, tag = "3")]
    pub decision_variables: ::prost::alloc::vec::Vec<u64>,
}
/// A message representing a k-hot constraint.
#[non_exhaustive]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct KHot {
    /// The ID of the constraint.
    #[prost(uint64, tag = "1")]
    pub constraint_id: u64,
    /// The list of ids of decision variables that are constrained to be k-hot.
    #[prost(uint64, repeated, tag = "2")]
    pub decision_variables: ::prost::alloc::vec::Vec<u64>,
    /// The number of variables that should be set to 1 (i.e., the value of k).
    #[prost(uint64, tag = "3")]
    pub num_hot_vars: u64,
}
/// A constraint hint is an additional inforomation to be used by solver to gain performance.
/// They are derived from one-or-more constraints in the instance and typically contains information of special types of constraints (e.g. one-hot, SOS, ...).
#[non_exhaustive]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ConstraintHints {
    /// One-hot constraint: e.g. `x_1 + ... + x_n = 1` for binary variables `x_1, ..., x_n`.
    #[deprecated]
    #[prost(message, repeated, tag = "2")]
    pub one_hot_constraints: ::prost::alloc::vec::Vec<OneHot>,
    /// SOS1 constraint: at most one of x_1, ..., x_n can be non-zero.
    #[prost(message, repeated, tag = "3")]
    pub sos1_constraints: ::prost::alloc::vec::Vec<Sos1>,
    /// K-hot constraints: map from k to a list of k-hot constraints.
    #[prost(map = "uint64, message", tag = "4")]
    pub k_hot_constraints: ::std::collections::HashMap<u64, KHotList>,
}
/// A list of KHot constraints with the same k value.
#[non_exhaustive]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct KHotList {
    #[prost(message, repeated, tag = "1")]
    pub constraints: ::prost::alloc::vec::Vec<KHot>,
}
/// Upper and lower bound of the decision variable.
#[non_exhaustive]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Bound {
    /// Lower bound of the decision variable.
    #[prost(double, tag = "1")]
    pub lower: f64,
    /// Upper bound of the decision variable.
    #[prost(double, tag = "2")]
    pub upper: f64,
}
/// Decison variable which mathematical programming solver will optimize.
/// It must have its kind, i.e. binary, integer, real or others and unique identifier of 64-bit integer.
/// It may have its name and subscripts which are used to identify in modeling tools.
#[non_exhaustive]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DecisionVariable {
    /// Decision variable ID.
    ///
    /// - IDs are not required to be sequential.
    #[prost(uint64, tag = "1")]
    pub id: u64,
    /// Kind of the decision variable
    #[prost(enumeration = "decision_variable::Kind", tag = "2")]
    pub kind: i32,
    /// Bound of the decision variable
    /// If the bound is not specified, the decision variable is considered as unbounded.
    #[prost(message, optional, tag = "3")]
    pub bound: ::core::option::Option<Bound>,
    /// Name of the decision variable. e.g. `x`
    #[prost(string, optional, tag = "4")]
    pub name: ::core::option::Option<::prost::alloc::string::String>,
    /// Subscripts of the decision variable. e.g. `\[1, 3\]` for an element of multidimensional deicion variable `x\[1, 3\]`
    #[prost(int64, repeated, tag = "5")]
    pub subscripts: ::prost::alloc::vec::Vec<i64>,
    /// Additional parameters for decision variables
    #[prost(map = "string, string", tag = "6")]
    pub parameters:
        ::std::collections::HashMap<::prost::alloc::string::String, ::prost::alloc::string::String>,
    /// Detail human-readable description of the decision variable.
    #[prost(string, optional, tag = "7")]
    pub description: ::core::option::Option<::prost::alloc::string::String>,
    /// The value substituted by partial evaluation of the instance. Not determined by the solver.
    #[prost(double, optional, tag = "8")]
    pub substituted_value: ::core::option::Option<f64>,
}
/// Nested message and enum types in `DecisionVariable`.
pub mod decision_variable {
    /// Kind of the decision variable
    #[non_exhaustive]
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
    #[repr(i32)]
    pub enum Kind {
        Unspecified = 0,
        Binary = 1,
        Integer = 2,
        Continuous = 3,
        /// Semi-integer decision variable is a decision variable that can take only integer values in the given range or zero.
        SemiInteger = 4,
        /// Semi-continuous decision variable is a decision variable that can take only continuous values in the given range or zero.
        SemiContinuous = 5,
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
                Kind::Continuous => "KIND_CONTINUOUS",
                Kind::SemiInteger => "KIND_SEMI_INTEGER",
                Kind::SemiContinuous => "KIND_SEMI_CONTINUOUS",
            }
        }
        /// Creates an enum from field names used in the ProtoBuf definition.
        pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
            match value {
                "KIND_UNSPECIFIED" => Some(Self::Unspecified),
                "KIND_BINARY" => Some(Self::Binary),
                "KIND_INTEGER" => Some(Self::Integer),
                "KIND_CONTINUOUS" => Some(Self::Continuous),
                "KIND_SEMI_INTEGER" => Some(Self::SemiInteger),
                "KIND_SEMI_CONTINUOUS" => Some(Self::SemiContinuous),
                _ => None,
            }
        }
    }
}
/// A set of parameters for instantiating an optimization problem from a parametric instance
#[non_exhaustive]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Parameters {
    #[prost(map = "uint64, double", tag = "1")]
    pub entries: ::std::collections::HashMap<u64, f64>,
}
#[non_exhaustive]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Instance {
    #[prost(message, optional, tag = "1")]
    pub description: ::core::option::Option<instance::Description>,
    /// Decision variables used in this instance
    ///
    /// - This must constain every decision variables used in the objective and constraints.
    /// - This can contains a decision variable that is not used in the objective or constraints.
    #[prost(message, repeated, tag = "2")]
    pub decision_variables: ::prost::alloc::vec::Vec<DecisionVariable>,
    #[prost(message, optional, tag = "3")]
    pub objective: ::core::option::Option<Function>,
    /// Constraints of the optimization problem
    #[prost(message, repeated, tag = "4")]
    pub constraints: ::prost::alloc::vec::Vec<Constraint>,
    /// The sense of this problem, i.e. minimize the objective or maximize it.
    ///
    /// Design decision note:
    /// - This is a required field. Most mathematical modeling tools allow for an empty sense and default to minimization. Alternatively, some tools do not create such a field and represent maximization problems by negating the objective function. This project prefers explicit descriptions over implicit ones to avoid such ambiguity and to make it unnecessary for developers to look up the reference for the treatment of omitted cases.
    ///
    #[prost(enumeration = "instance::Sense", tag = "5")]
    pub sense: i32,
    /// Parameters used when instantiating this instance
    #[prost(message, optional, tag = "6")]
    pub parameters: ::core::option::Option<Parameters>,
    /// Constraint hints to be used by solver to gain performance. They are derived from one-or-more constraints in the instance and typically contains information of special types of constraints (e.g. one-hot, SOS, ...).
    #[prost(message, optional, tag = "7")]
    pub constraint_hints: ::core::option::Option<ConstraintHints>,
    /// Constraints removed via preprocessing. These are restored when evaluated into `ommx.v1.Solution`.
    #[prost(message, repeated, tag = "8")]
    pub removed_constraints: ::prost::alloc::vec::Vec<RemovedConstraint>,
    /// When a decision variable is dependent on another decision variable as polynomial, this map contains the ID of the dependent decision variable as key and the polynomial as value.
    #[prost(map = "uint64, message", tag = "9")]
    pub decision_variable_dependency: ::std::collections::HashMap<u64, Function>,
}
/// Nested message and enum types in `Instance`.
pub mod instance {
    #[non_exhaustive]
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
    /// The sense of this instance
    #[non_exhaustive]
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
    #[repr(i32)]
    pub enum Sense {
        Unspecified = 0,
        Minimize = 1,
        Maximize = 2,
    }
    impl Sense {
        /// String value of the enum field names used in the ProtoBuf definition.
        ///
        /// The values are not transformed in any way and thus are considered stable
        /// (if the ProtoBuf definition does not change) and safe for programmatic use.
        pub fn as_str_name(&self) -> &'static str {
            match self {
                Sense::Unspecified => "SENSE_UNSPECIFIED",
                Sense::Minimize => "SENSE_MINIMIZE",
                Sense::Maximize => "SENSE_MAXIMIZE",
            }
        }
        /// Creates an enum from field names used in the ProtoBuf definition.
        pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
            match value {
                "SENSE_UNSPECIFIED" => Some(Self::Unspecified),
                "SENSE_MINIMIZE" => Some(Self::Minimize),
                "SENSE_MAXIMIZE" => Some(Self::Maximize),
                _ => None,
            }
        }
    }
}
/// Placeholder of a parameter in a parametrized optimization problem
#[non_exhaustive]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Parameter {
    /// ID for the parameter
    ///
    /// - IDs are not required to be sequential.
    /// - The ID must be unique within the instance including the decision variables.
    #[prost(uint64, tag = "1")]
    pub id: u64,
    /// Name of the parameter. e.g. `x`
    #[prost(string, optional, tag = "2")]
    pub name: ::core::option::Option<::prost::alloc::string::String>,
    /// Subscripts of the parameter, same usage as DecisionVariable.subscripts
    #[prost(int64, repeated, tag = "3")]
    pub subscripts: ::prost::alloc::vec::Vec<i64>,
    /// Additional metadata for the parameter, same usage as DecisionVariable.parameters
    #[prost(map = "string, string", tag = "4")]
    pub parameters:
        ::std::collections::HashMap<::prost::alloc::string::String, ::prost::alloc::string::String>,
    /// Human-readable description for the parameter
    #[prost(string, optional, tag = "5")]
    pub description: ::core::option::Option<::prost::alloc::string::String>,
}
/// Optimization problem including parameter, variables varying while solving the problem like penalty weights or dual variables.
/// These parameters are not decision variables.
#[non_exhaustive]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ParametricInstance {
    #[prost(message, optional, tag = "1")]
    pub description: ::core::option::Option<instance::Description>,
    /// Decision variables used in this instance
    #[prost(message, repeated, tag = "2")]
    pub decision_variables: ::prost::alloc::vec::Vec<DecisionVariable>,
    /// Parameters of this instance
    ///
    /// - The ID must be unique within the instance including the decision variables.
    #[prost(message, repeated, tag = "3")]
    pub parameters: ::prost::alloc::vec::Vec<Parameter>,
    /// Objective function of the optimization problem. This may contain parameters in addition to the decision variables.
    #[prost(message, optional, tag = "4")]
    pub objective: ::core::option::Option<Function>,
    /// Constraints of the optimization problem. This may contain parameters in addition to the decision variables.
    #[prost(message, repeated, tag = "5")]
    pub constraints: ::prost::alloc::vec::Vec<Constraint>,
    /// The sense of this problem, i.e. minimize the objective or maximize it.
    #[prost(enumeration = "instance::Sense", tag = "6")]
    pub sense: i32,
    /// Constraint hints to be used by solver to gain performance. They are derived from one-or-more constraints in the instance and typically contains information of special types of constraints (e.g. one-hot, SOS, ...).
    #[prost(message, optional, tag = "7")]
    pub constraint_hints: ::core::option::Option<ConstraintHints>,
    /// Constraints removed via preprocessing. These are restored when evaluated into `ommx.v1.Solution`.
    #[prost(message, repeated, tag = "8")]
    pub removed_constraints: ::prost::alloc::vec::Vec<RemovedConstraint>,
    /// When a decision variable is dependent on another decision variable as polynomial, this map contains the ID of the dependent decision variable as key and the polynomial as value.
    #[prost(map = "uint64, message", tag = "9")]
    pub decision_variable_dependency: ::std::collections::HashMap<u64, Function>,
}
/// A set of values of decision variables, without any evaluation, even the
/// feasiblity of the solution.
#[non_exhaustive]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct State {
    /// The value of the solution for each variable ID.
    #[prost(map = "uint64, double", tag = "1")]
    pub entries: ::std::collections::HashMap<u64, f64>,
}
/// Solution with evaluated objective and constraints
#[non_exhaustive]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Solution {
    #[prost(message, optional, tag = "1")]
    pub state: ::core::option::Option<State>,
    #[prost(double, tag = "2")]
    pub objective: f64,
    #[prost(message, repeated, tag = "3")]
    pub decision_variables: ::prost::alloc::vec::Vec<DecisionVariable>,
    #[prost(message, repeated, tag = "4")]
    pub evaluated_constraints: ::prost::alloc::vec::Vec<EvaluatedConstraint>,
    /// The feasibility of the solution for all, remaining and removed constraints.
    ///
    /// The feasibility for the remaining constraints is represented by the `feasible_relaxed` field.
    #[prost(bool, tag = "5")]
    pub feasible: bool,
    /// Feasibility of the solution for remaining constraints, ignoring removed constraints.
    ///
    /// This is optional due to the backward compatibility.
    /// If this field is NULL, the `feasible` field represents relaxed feasibility,
    /// and the deprecated `feasible_unrelaxed` field represents the feasibility including removed constraints.
    #[prost(bool, optional, tag = "9")]
    pub feasible_relaxed: ::core::option::Option<bool>,
    /// \[DEPRECATED\] Feasibility of the solution for all constraints.
    /// This field has been introduced in Python SDK 1.6.0 and deprecated in 1.7.0.
    /// The feasibility in this sense is represented by the `feasible` field after 1.7.0.
    #[deprecated]
    #[prost(bool, tag = "8")]
    pub feasible_unrelaxed: bool,
    /// The optimality of the solution.
    #[prost(enumeration = "Optimality", tag = "6")]
    pub optimality: i32,
    /// Whether the solution is obtained by a relaxed linear programming solver.
    #[prost(enumeration = "Relaxation", tag = "7")]
    pub relaxation: i32,
}
/// The solver proved that the problem is infeasible.
///
/// TODO: Add more information about the infeasibility.
#[non_exhaustive]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Infeasible {}
/// The solver proved that the problem is unbounded.
///
/// TODO: Add more information about the unboundedness.
#[non_exhaustive]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Unbounded {}
#[non_exhaustive]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Result {
    #[prost(oneof = "result::Result", tags = "1, 2, 3, 4")]
    pub result: ::core::option::Option<result::Result>,
}
/// Nested message and enum types in `Result`.
pub mod result {
    #[non_exhaustive]
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Result {
        /// Error information by the solver which cannot be expressed by other messages.
        /// This string should be human-readable.
        #[prost(string, tag = "1")]
        Error(::prost::alloc::string::String),
        /// Some feasible or infeasible solution for the problem is found. Most of heuristic solvers should use this value.
        #[prost(message, tag = "2")]
        Solution(super::Solution),
        /// The solver proved that the problem is infeasible, i.e. all solutions of the problem are infeasible.
        /// If the solver cannot get the proof of infeasibility,
        /// and just cannot find any feasible solution due to the time limit or due to heuristic algorithm limitation,
        /// the solver should return its *best* `Solution` message with `feasible` field set to `false`.
        #[prost(message, tag = "3")]
        Infeasible(super::Infeasible),
        /// The solver proved that the problem is unbounded.
        #[prost(message, tag = "4")]
        Unbounded(super::Unbounded),
    }
}
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum Optimality {
    /// The solver cannot determine whether the solution is optimal. Most of heuristic solvers should use this value.
    Unspecified = 0,
    /// The solver has determined that the solution is optimal.
    Optimal = 1,
    /// The solver has determined that the solution is not optimal.
    NotOptimal = 2,
}
impl Optimality {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            Optimality::Unspecified => "OPTIMALITY_UNSPECIFIED",
            Optimality::Optimal => "OPTIMALITY_OPTIMAL",
            Optimality::NotOptimal => "OPTIMALITY_NOT_OPTIMAL",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "OPTIMALITY_UNSPECIFIED" => Some(Self::Unspecified),
            "OPTIMALITY_OPTIMAL" => Some(Self::Optimal),
            "OPTIMALITY_NOT_OPTIMAL" => Some(Self::NotOptimal),
            _ => None,
        }
    }
}
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum Relaxation {
    /// No relaxation is used.
    Unspecified = 0,
    /// The solution is obtained by a relaxed linear programming problem.
    LpRelaxed = 1,
}
impl Relaxation {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            Relaxation::Unspecified => "RELAXATION_UNSPECIFIED",
            Relaxation::LpRelaxed => "RELAXATION_LP_RELAXED",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "RELAXATION_UNSPECIFIED" => Some(Self::Unspecified),
            "RELAXATION_LP_RELAXED" => Some(Self::LpRelaxed),
            _ => None,
        }
    }
}
/// A map from sample ID to state
#[non_exhaustive]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Samples {
    #[prost(message, repeated, tag = "1")]
    pub entries: ::prost::alloc::vec::Vec<samples::SamplesEntry>,
}
/// Nested message and enum types in `Samples`.
pub mod samples {
    /// Sampling processes are likely to generate same samples multiple times. We compress the same samples into one entry.
    /// Note that uncompressed state is also valid. The reader should not assume that every states are distinct.
    #[non_exhaustive]
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct SamplesEntry {
        /// State of the sample
        #[prost(message, optional, tag = "1")]
        pub state: ::core::option::Option<super::State>,
        /// IDs of the sample
        #[prost(uint64, repeated, tag = "2")]
        pub ids: ::prost::alloc::vec::Vec<u64>,
    }
}
/// A map from sample IDs to sampled values
#[non_exhaustive]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SampledValues {
    #[prost(message, repeated, tag = "1")]
    pub entries: ::prost::alloc::vec::Vec<sampled_values::SampledValuesEntry>,
}
/// Nested message and enum types in `SampledValues`.
pub mod sampled_values {
    /// Compressed sampled values, but uncompressed state is also valid. The reader should not assume that every states are distinct.
    #[non_exhaustive]
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct SampledValuesEntry {
        #[prost(double, tag = "1")]
        pub value: f64,
        /// IDs of the sample
        #[prost(uint64, repeated, tag = "2")]
        pub ids: ::prost::alloc::vec::Vec<u64>,
    }
}
/// A pair of decision variable description and its sampled values
#[non_exhaustive]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SampledDecisionVariable {
    #[prost(message, optional, tag = "1")]
    pub decision_variable: ::core::option::Option<DecisionVariable>,
    /// Sampled values of decision variable. This becomes `None` if the decision variable is not sampled.
    #[prost(message, optional, tag = "2")]
    pub samples: ::core::option::Option<SampledValues>,
}
/// Evaluated constraint for samples
#[non_exhaustive]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SampledConstraint {
    /// Constraint ID
    #[prost(uint64, tag = "1")]
    pub id: u64,
    #[prost(enumeration = "Equality", tag = "2")]
    pub equality: i32,
    /// Name of the constraint.
    #[prost(string, optional, tag = "3")]
    pub name: ::core::option::Option<::prost::alloc::string::String>,
    /// Integer parameters of the constraint.
    #[prost(int64, repeated, tag = "4")]
    pub subscripts: ::prost::alloc::vec::Vec<i64>,
    /// Key-value parameters of the constraint.
    #[prost(map = "string, string", tag = "5")]
    pub parameters:
        ::std::collections::HashMap<::prost::alloc::string::String, ::prost::alloc::string::String>,
    /// Detail human-readable description of the constraint.
    #[prost(string, optional, tag = "6")]
    pub description: ::core::option::Option<::prost::alloc::string::String>,
    /// Short removed reason of the constraint. This field exists only if this message is evaluated from a removed constraint.
    #[prost(string, optional, tag = "7")]
    pub removed_reason: ::core::option::Option<::prost::alloc::string::String>,
    /// Detailed parameters why the constraint is removed. This field exists only if this message is evaluated from a removed constraint.
    #[prost(map = "string, string", tag = "8")]
    pub removed_reason_parameters:
        ::std::collections::HashMap<::prost::alloc::string::String, ::prost::alloc::string::String>,
    /// Evaluated values of constraint for each sample
    #[prost(message, optional, tag = "9")]
    pub evaluated_values: ::core::option::Option<SampledValues>,
    /// IDs of decision variables used to evaluate this constraint
    #[prost(uint64, repeated, tag = "10")]
    pub used_decision_variable_ids: ::prost::alloc::vec::Vec<u64>,
    /// Feasibility of each sample
    #[prost(map = "uint64, bool", tag = "11")]
    pub feasible: ::std::collections::HashMap<u64, bool>,
}
/// Output of the sampling process.
#[non_exhaustive]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SampleSet {
    #[prost(message, optional, tag = "1")]
    pub objectives: ::core::option::Option<SampledValues>,
    #[prost(message, repeated, tag = "2")]
    pub decision_variables: ::prost::alloc::vec::Vec<SampledDecisionVariable>,
    #[prost(message, repeated, tag = "3")]
    pub constraints: ::prost::alloc::vec::Vec<SampledConstraint>,
    /// Feasibility for *both* remaining and removed constraints of each sample.
    ///
    /// The meaning of `feasible` field in SDK changes between Python SDK 1.6.0 to 1.7.0.
    /// In Python SDK 1.6.0, `feasible` represents the feasibility of remaining constraints of each sample,
    /// i.e. removed constraints (introduced in 1.6.0) are not considered.
    /// After Python SDK 1.7.0, `feasible` represents the feasibility of all constraints of each sample.
    /// The feasibility of 1.6.0 is renamed to `feasible_relaxed` in 1.7.0.
    #[prost(map = "uint64, bool", tag = "4")]
    pub feasible: ::std::collections::HashMap<u64, bool>,
    /// \[Deprecated\] This field has been introduced in Python SDK 1.6.0 to represent
    /// the feasibility of all constraints of each sample.
    /// The `feasible` field is used in this sense after Python SDK 1.7.0.
    #[prost(map = "uint64, bool", tag = "6")]
    #[deprecated]
    pub feasible_unrelaxed: ::std::collections::HashMap<u64, bool>,
    /// Feasibility for remaining (non-removed) constraints of each sample.
    #[prost(map = "uint64, bool", tag = "7")]
    pub feasible_relaxed: ::std::collections::HashMap<u64, bool>,
    /// Minimize or Maximize
    #[prost(enumeration = "instance::Sense", tag = "5")]
    pub sense: i32,
}
