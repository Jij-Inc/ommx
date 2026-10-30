// These selectors are nominal values, not a closed Python Enum. Keep the Rust
// domain value inside the wrapper so future SDK kinds survive generic queries.
macro_rules! open_kind {
    ($(#[$doc:meta])* $name:ident, $core:ty, $code:expr; $( $variant:ident => $core_variant:ident ),+ $(,)?) => {
        $(#[$doc])*
        #[pyo3_stub_gen::derive::gen_stub_pyclass]
        #[pyclass(frozen)]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name($core);

        #[allow(non_upper_case_globals)]
        #[pymethods]
        impl $name {
            $(#[classattr]
            pub const $variant: $name = $name(<$core>::$core_variant);)+
        }

        #[pyo3_stub_gen::derive::gen_stub_pymethods]
        #[pymethods]
        impl $name {
            fn __hash__(&self) -> isize {
                let code: fn($core) -> i32 = $code;
                code(self.0) as isize
            }
            fn __int__(&self) -> i32 {
                self.__hash__() as i32
            }
            fn __eq__(&self, other: &Bound<'_, PyAny>) -> bool {
                if let Ok(other) = other.extract::<Self>() {
                    self.0 == other.0
                } else if let Ok(code) = other.extract::<i32>() {
                    self.__hash__() == code as isize
                } else {
                    false
                }
            }
            fn __repr__(&self) -> String {
                $(if self.0 == <$core>::$core_variant {
                    return concat!(stringify!($name), ".", stringify!($variant)).to_string();
                })+
                format!("{}.{:?}", stringify!($name), self.0)
            }
        }
        impl From<$core> for $name {
            fn from(value: $core) -> Self { Self(value) }
        }
        impl From<$name> for $core {
            fn from(value: $name) -> Self { value.0 }
        }
        // Const default generation emits a self-reference before the Python
        // class exists. Record these typed attributes without that initializer.
        pyo3_stub_gen::inventory::submit! {
            pyo3_stub_gen::type_info::PyMethodsInfo {
                struct_id: std::any::TypeId::of::<$name>,
                attrs: &[$(pyo3_stub_gen::type_info::MemberInfo {
                    name: stringify!($variant),
                    r#type: <$name as pyo3_stub_gen::PyStubType>::type_output,
                    doc: "", default: None, deprecated: None,
                }),+],
                getters: &[], setters: &[], methods: &[],
                file: file!(), line: line!(), column: column!(),
            }
        }
    };
}

use pyo3::prelude::*;

pub(crate) use open_kind;

/// How a decision variable obtains its value in an {class}`~ommx.Instance`.
///
/// Roles are derived from the Instance's current structure and can change after
/// transformation. They are separate from Binary, Integer, or other variable kinds.
/// Query one role with {meth}`~ommx.Instance.decision_variable_role`, or all roles
/// with {meth}`~ommx.Instance.decision_variable_roles`.
///
/// | Role | Meaning | Value for candidate evaluation |
/// | --- | --- | --- |
/// | `Used` | Referenced by the current objective or active constraints | Supplied by the caller |
/// | `Fixed` | A retained, non-used variable with a recorded fixed value | Restored from that value |
/// | `Dependent` | Neither used nor fixed; represented by an expression in other variables | Computed from that expression |
/// | `Irrelevant` | Neither used, fixed, nor dependent | Supplied by the caller, or filled with a default when omitted |
///
/// Active constraints include regular and special constraints, including an
/// Indicator's controlling Binary variable. References only from named functions,
/// removed constraints, output objectives, or dependency right-hand sides do not
/// make a variable used. Being irrelevant to solver input does not mean its value
/// cannot affect reporting.
///
/// {meth}`~ommx.Instance.populate_state` explains reconstruction, defaults,
/// canonicalization, and consistency assertions with executable examples.
#[pyo3_stub_gen::derive::gen_stub_pyclass_enum]
#[pyclass(eq, eq_int, frozen)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DecisionVariableRole {
    /// Used by the objective or active constraints passed to solvers
    Used = 1,
    /// Fixed by the instance-owned fixed-value table and not used by solver input
    Fixed = 2,
    /// Defined by decision_variable_dependency
    Dependent = 3,
    /// Not used, fixed, or dependent
    Irrelevant = 4,
}

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl DecisionVariableRole {
    fn __hash__(&self) -> isize {
        *self as isize
    }

    fn __repr__(&self) -> &'static str {
        match self {
            DecisionVariableRole::Used => "DecisionVariableRole.Used",
            DecisionVariableRole::Fixed => "DecisionVariableRole.Fixed",
            DecisionVariableRole::Dependent => "DecisionVariableRole.Dependent",
            DecisionVariableRole::Irrelevant => "DecisionVariableRole.Irrelevant",
        }
    }

    fn __str__(&self) -> &'static str {
        match self {
            DecisionVariableRole::Used => "used",
            DecisionVariableRole::Fixed => "fixed",
            DecisionVariableRole::Dependent => "dependent",
            DecisionVariableRole::Irrelevant => "irrelevant",
        }
    }
}

impl From<ommx::DecisionVariableRole> for DecisionVariableRole {
    fn from(role: ommx::DecisionVariableRole) -> Self {
        match role {
            ommx::DecisionVariableRole::Used => DecisionVariableRole::Used,
            ommx::DecisionVariableRole::Fixed => DecisionVariableRole::Fixed,
            ommx::DecisionVariableRole::Dependent => DecisionVariableRole::Dependent,
            ommx::DecisionVariableRole::Irrelevant => DecisionVariableRole::Irrelevant,
        }
    }
}

impl From<DecisionVariableRole> for ommx::DecisionVariableRole {
    fn from(role: DecisionVariableRole) -> Self {
        match role {
            DecisionVariableRole::Used => ommx::DecisionVariableRole::Used,
            DecisionVariableRole::Fixed => ommx::DecisionVariableRole::Fixed,
            DecisionVariableRole::Dependent => ommx::DecisionVariableRole::Dependent,
            DecisionVariableRole::Irrelevant => ommx::DecisionVariableRole::Irrelevant,
        }
    }
}

/// Sense of optimization (minimize or maximize)
#[pyo3_stub_gen::derive::gen_stub_pyclass_enum]
#[pyclass(eq, eq_int, frozen)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Sense {
    /// Minimize the objective function
    Minimize = 1,
    /// Maximize the objective function
    Maximize = 2,
}

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl Sense {
    fn __hash__(&self) -> isize {
        *self as isize
    }

    /// Convert from Protocol Buffer sense value
    #[staticmethod]
    pub fn from_pb(value: i32) -> PyResult<Self> {
        match value {
            1 => Ok(Sense::Minimize),
            2 => Ok(Sense::Maximize),
            _ => Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(format!(
                "Invalid sense value: {value}"
            ))),
        }
    }

    /// Convert to Protocol Buffer sense value
    pub fn to_pb(&self) -> i32 {
        *self as i32
    }

    fn __repr__(&self) -> String {
        match self {
            Sense::Minimize => "Sense.Minimize".to_string(),
            Sense::Maximize => "Sense.Maximize".to_string(),
        }
    }

    fn __str__(&self) -> String {
        format!("{}", *self as i32)
    }
}

impl From<ommx::Sense> for Sense {
    fn from(sense: ommx::Sense) -> Self {
        match sense {
            ommx::Sense::Minimize => Sense::Minimize,
            ommx::Sense::Maximize => Sense::Maximize,
        }
    }
}

impl From<Sense> for ommx::Sense {
    fn from(sense: Sense) -> Self {
        match sense {
            Sense::Minimize => ommx::Sense::Minimize,
            Sense::Maximize => ommx::Sense::Maximize,
        }
    }
}

/// Equality type for constraints
#[pyo3_stub_gen::derive::gen_stub_pyclass_enum]
#[pyclass(eq, eq_int, frozen)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Equality {
    /// Equal to zero constraint (=)
    EqualToZero = 1,
    /// Less than or equal to zero constraint (<=)
    LessThanOrEqualToZero = 2,
}

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl Equality {
    fn __hash__(&self) -> isize {
        *self as isize
    }

    /// Convert from Protocol Buffer equality value
    #[staticmethod]
    pub fn from_pb(value: i32) -> PyResult<Self> {
        match value {
            1 => Ok(Equality::EqualToZero),
            2 => Ok(Equality::LessThanOrEqualToZero),
            _ => Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(format!(
                "Invalid equality value: {value}"
            ))),
        }
    }

    /// Convert to Protocol Buffer equality value
    pub fn to_pb(&self) -> i32 {
        *self as i32
    }

    fn __repr__(&self) -> &str {
        match self {
            Equality::EqualToZero => "Equality.EqualToZero",
            Equality::LessThanOrEqualToZero => "Equality.LessThanOrEqualToZero",
        }
    }

    fn __str__(&self) -> &str {
        match self {
            Equality::EqualToZero => "=0",
            Equality::LessThanOrEqualToZero => "<=0",
        }
    }
}

impl From<ommx::Equality> for Equality {
    fn from(equality: ommx::Equality) -> Self {
        match equality {
            ommx::Equality::EqualToZero => Equality::EqualToZero,
            ommx::Equality::LessThanOrEqualToZero => Equality::LessThanOrEqualToZero,
        }
    }
}

impl From<Equality> for ommx::Equality {
    fn from(equality: Equality) -> Self {
        match equality {
            Equality::EqualToZero => ommx::Equality::EqualToZero,
            Equality::LessThanOrEqualToZero => ommx::Equality::LessThanOrEqualToZero,
        }
    }
}

open_kind! {
    /// Numeric scalar decision-variable domain kind.
    ///
    /// New kinds may be added. This value type does not promise an exhaustive
    /// list. Inspect known constants and reject unsupported kinds in consumers
    /// that interpret domain semantics. Generic metadata queries retain Kind.
    Kind, ommx::Kind, |kind| ommx::v1::decision_variable::Kind::from(kind) as i32;
    Binary => Binary, Integer => Integer, Continuous => Continuous,
    SemiInteger => SemiInteger, SemiContinuous => SemiContinuous, FiniteDomain => FiniteDomain
}

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl Kind {
    /// Convert a known protobuf kind. Unknown semantic kinds are rejected.
    #[staticmethod]
    pub fn from_pb(value: i32) -> PyResult<Self> {
        let wire = ommx::v1::decision_variable::Kind::try_from(value).map_err(|_| {
            pyo3::exceptions::PyValueError::new_err(format!("Invalid kind value: {value}"))
        })?;
        ommx::Kind::try_from(wire).map(Self).map_err(|_| {
            pyo3::exceptions::PyValueError::new_err(format!("Invalid kind value: {value}"))
        })
    }
    /// Convert to the protobuf kind number.
    pub fn to_pb(&self) -> i32 {
        ommx::v1::decision_variable::Kind::from(self.0) as i32
    }
    fn __str__(&self) -> String {
        format!("{:?}", self.0)
    }
}

/// Optimality status of a solution
#[pyo3_stub_gen::derive::gen_stub_pyclass_enum]
#[pyclass(eq, eq_int)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Optimality {
    /// The solver cannot determine whether the solution is optimal
    Unspecified = 0,
    /// The solver has determined that the solution is optimal
    Optimal = 1,
    /// The solver has determined that the solution is not optimal
    NotOptimal = 2,
}

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl Optimality {
    /// Convert from Protocol Buffer optimality value
    #[staticmethod]
    pub fn from_pb(value: i32) -> PyResult<Self> {
        match value {
            0 => Ok(Optimality::Unspecified),
            1 => Ok(Optimality::Optimal),
            2 => Ok(Optimality::NotOptimal),
            _ => Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(format!(
                "Invalid optimality value: {value}"
            ))),
        }
    }

    /// Convert to Protocol Buffer optimality value
    pub fn to_pb(&self) -> i32 {
        *self as i32
    }

    fn __repr__(&self) -> String {
        match self {
            Optimality::Unspecified => "Optimality.Unspecified".to_string(),
            Optimality::Optimal => "Optimality.Optimal".to_string(),
            Optimality::NotOptimal => "Optimality.NotOptimal".to_string(),
        }
    }

    fn __str__(&self) -> String {
        format!("{}", *self as i32)
    }
}

impl From<ommx::v1::Optimality> for Optimality {
    fn from(optimality: ommx::v1::Optimality) -> Self {
        match optimality {
            ommx::v1::Optimality::Unspecified => Optimality::Unspecified,
            ommx::v1::Optimality::Optimal => Optimality::Optimal,
            ommx::v1::Optimality::NotOptimal => Optimality::NotOptimal,
            _ => Optimality::Unspecified, // Default for unknown variants
        }
    }
}

impl From<Optimality> for ommx::v1::Optimality {
    fn from(optimality: Optimality) -> Self {
        match optimality {
            Optimality::Unspecified => ommx::v1::Optimality::Unspecified,
            Optimality::Optimal => ommx::v1::Optimality::Optimal,
            Optimality::NotOptimal => ommx::v1::Optimality::NotOptimal,
        }
    }
}

/// Relaxation status of a solution
#[pyo3_stub_gen::derive::gen_stub_pyclass_enum]
#[pyclass(eq, eq_int)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Relaxation {
    /// No relaxation is used
    Unspecified = 0,
    /// The solution is obtained by a relaxed linear programming problem
    LpRelaxed = 1,
}

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl Relaxation {
    /// Convert from Protocol Buffer relaxation value
    #[staticmethod]
    pub fn from_pb(value: i32) -> PyResult<Self> {
        match value {
            0 => Ok(Relaxation::Unspecified),
            1 => Ok(Relaxation::LpRelaxed),
            _ => Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(format!(
                "Invalid relaxation value: {value}"
            ))),
        }
    }

    /// Convert to Protocol Buffer relaxation value
    pub fn to_pb(&self) -> i32 {
        *self as i32
    }

    fn __repr__(&self) -> String {
        match self {
            Relaxation::Unspecified => "Relaxation.Unspecified".to_string(),
            Relaxation::LpRelaxed => "Relaxation.LpRelaxed".to_string(),
        }
    }

    fn __str__(&self) -> String {
        format!("{}", *self as i32)
    }
}

impl From<ommx::v1::Relaxation> for Relaxation {
    fn from(relaxation: ommx::v1::Relaxation) -> Self {
        match relaxation {
            ommx::v1::Relaxation::Unspecified => Relaxation::Unspecified,
            ommx::v1::Relaxation::LpRelaxed => Relaxation::LpRelaxed,
            _ => Relaxation::Unspecified, // Default for unknown variants
        }
    }
}

impl From<Relaxation> for ommx::v1::Relaxation {
    fn from(relaxation: Relaxation) -> Self {
        match relaxation {
            Relaxation::Unspecified => ommx::v1::Relaxation::Unspecified,
            Relaxation::LpRelaxed => ommx::v1::Relaxation::LpRelaxed,
        }
    }
}
