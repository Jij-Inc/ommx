use pyo3::prelude::*;

/// Sense of optimization (minimize or maximize)
#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pyclass_enum)]
#[pyclass(eq, eq_int)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Sense {
    /// Minimize the objective function
    Minimize = 1,
    /// Maximize the objective function
    Maximize = 2,
}

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pymethods)]
#[pymethods]
impl Sense {
    /// Convert from Protocol Buffer sense value
    #[staticmethod]
    pub fn from_pb(value: i32) -> PyResult<Self> {
        match value {
            1 => Ok(Sense::Minimize),
            2 => Ok(Sense::Maximize),
            _ => Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(format!(
                "Invalid sense value: {}",
                value
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
#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pyclass_enum)]
#[pyclass(eq, eq_int)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Equality {
    /// Equal to zero constraint (=)
    EqualToZero = 1,
    /// Less than or equal to zero constraint (<=)
    LessThanOrEqualToZero = 2,
}

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pymethods)]
#[pymethods]
impl Equality {
    /// Convert from Protocol Buffer equality value
    #[staticmethod]
    pub fn from_pb(value: i32) -> PyResult<Self> {
        match value {
            1 => Ok(Equality::EqualToZero),
            2 => Ok(Equality::LessThanOrEqualToZero),
            _ => Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(format!(
                "Invalid equality value: {}",
                value
            ))),
        }
    }

    /// Convert to Protocol Buffer equality value
    pub fn to_pb(&self) -> i32 {
        *self as i32
    }

    fn __repr__(&self) -> String {
        match self {
            Equality::EqualToZero => "Equality.EqualToZero".to_string(),
            Equality::LessThanOrEqualToZero => "Equality.LessThanOrEqualToZero".to_string(),
        }
    }

    fn __str__(&self) -> String {
        format!("{}", *self as i32)
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

/// Kind of decision variable
#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pyclass_enum)]
#[pyclass(eq, eq_int)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Kind {
    /// Binary decision variable (0 or 1)
    Binary = 1,
    /// Integer decision variable
    Integer = 2,
    /// Continuous decision variable (real-valued)
    Continuous = 3,
    /// Semi-integer decision variable (integer in range or zero)
    SemiInteger = 4,
    /// Semi-continuous decision variable (continuous in range or zero)
    SemiContinuous = 5,
}

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pymethods)]
#[pymethods]
impl Kind {
    /// Convert from Protocol Buffer kind value
    #[staticmethod]
    pub fn from_pb(value: i32) -> PyResult<Self> {
        match value {
            1 => Ok(Kind::Binary),
            2 => Ok(Kind::Integer),
            3 => Ok(Kind::Continuous),
            4 => Ok(Kind::SemiInteger),
            5 => Ok(Kind::SemiContinuous),
            _ => Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(format!(
                "Invalid kind value: {}",
                value
            ))),
        }
    }

    /// Convert to Protocol Buffer kind value
    pub fn to_pb(&self) -> i32 {
        *self as i32
    }

    fn __repr__(&self) -> String {
        match self {
            Kind::Binary => "Kind.Binary".to_string(),
            Kind::Integer => "Kind.Integer".to_string(),
            Kind::Continuous => "Kind.Continuous".to_string(),
            Kind::SemiInteger => "Kind.SemiInteger".to_string(),
            Kind::SemiContinuous => "Kind.SemiContinuous".to_string(),
        }
    }

    fn __str__(&self) -> String {
        let rust_kind: ommx::Kind = (*self).into();
        format!("{:?}", rust_kind)
    }
}

impl From<ommx::Kind> for Kind {
    fn from(kind: ommx::Kind) -> Self {
        match kind {
            ommx::Kind::Binary => Kind::Binary,
            ommx::Kind::Integer => Kind::Integer,
            ommx::Kind::Continuous => Kind::Continuous,
            ommx::Kind::SemiInteger => Kind::SemiInteger,
            ommx::Kind::SemiContinuous => Kind::SemiContinuous,
        }
    }
}

impl From<Kind> for ommx::Kind {
    fn from(kind: Kind) -> Self {
        match kind {
            Kind::Binary => ommx::Kind::Binary,
            Kind::Integer => ommx::Kind::Integer,
            Kind::Continuous => ommx::Kind::Continuous,
            Kind::SemiInteger => ommx::Kind::SemiInteger,
            Kind::SemiContinuous => ommx::Kind::SemiContinuous,
        }
    }
}

/// Optimality status of a solution
#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pyclass_enum)]
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

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pymethods)]
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
                "Invalid optimality value: {}",
                value
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
#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pyclass_enum)]
#[pyclass(eq, eq_int)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Relaxation {
    /// No relaxation is used
    Unspecified = 0,
    /// The solution is obtained by a relaxed linear programming problem
    LpRelaxed = 1,
}

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pymethods)]
#[pymethods]
impl Relaxation {
    /// Convert from Protocol Buffer relaxation value
    #[staticmethod]
    pub fn from_pb(value: i32) -> PyResult<Self> {
        match value {
            0 => Ok(Relaxation::Unspecified),
            1 => Ok(Relaxation::LpRelaxed),
            _ => Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(format!(
                "Invalid relaxation value: {}",
                value
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
