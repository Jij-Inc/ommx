use pyo3::prelude::*;

/// Sense of optimization (minimize or maximize)
#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pyclass)]
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

impl Sense {
    /// Convert from Rust ommx::Sense
    pub fn from_rust(sense: ommx::Sense) -> Self {
        match sense {
            ommx::Sense::Minimize => Sense::Minimize,
            ommx::Sense::Maximize => Sense::Maximize,
        }
    }

    /// Convert to Rust ommx::Sense
    pub fn to_rust(&self) -> ommx::Sense {
        match self {
            Sense::Minimize => ommx::Sense::Minimize,
            Sense::Maximize => ommx::Sense::Maximize,
        }
    }
}

/// Equality type for constraints
#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pyclass)]
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

impl Equality {
    /// Convert from Rust ommx::Equality
    pub fn from_rust(equality: ommx::Equality) -> Self {
        match equality {
            ommx::Equality::EqualToZero => Equality::EqualToZero,
            ommx::Equality::LessThanOrEqualToZero => Equality::LessThanOrEqualToZero,
        }
    }

    /// Convert to Rust ommx::Equality
    pub fn to_rust(&self) -> ommx::Equality {
        match self {
            Equality::EqualToZero => ommx::Equality::EqualToZero,
            Equality::LessThanOrEqualToZero => ommx::Equality::LessThanOrEqualToZero,
        }
    }
}