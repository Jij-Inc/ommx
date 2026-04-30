use crate::{Constraint, Function, Linear, Polynomial, Quadratic};
use ommx::{LinearMonomial, VariableID};
use pyo3::{prelude::*, Bound, PyAny};
use std::collections::HashMap;

/// Parameter in an optimization problem.
///
/// Parameters are values that are fixed during optimization but may vary between different
/// runs or scenarios. They share the same ID space with decision variables.
///
/// Note that this object overloads `==` for creating a constraint, not for equality comparison.
///
/// # Examples
///
/// ```python
/// >>> p = Parameter(1, name="penalty")
/// >>> x = DecisionVariable.integer(2)
/// >>> x + p  # Returns Linear expression
/// Linear(...)
/// ```
#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass]
#[derive(Clone)]
pub struct Parameter(pub ommx::v1::Parameter);

impl Parameter {
    /// Helper to create a Linear term from this parameter with coefficient 1
    fn as_linear(&self) -> ommx::Linear {
        ommx::Linear::single_term(
            LinearMonomial::Variable(VariableID::from(self.0.id)),
            ommx::coeff!(1.0),
        )
    }
}

// Overload stubs for arithmetic operators.
// Must appear before #[gen_stub_pymethods] for correct ordering.
pyo3_stub_gen::inventory::submit! {
    pyo3_stub_gen::derive::gen_methods_from_python! {
        r#"
        class Parameter:
            @overload
            def __add__(self, rhs: Scalar | LinearLike | Parameter) -> Linear: ...
            @overload
            def __add__(self, rhs: Quadratic) -> Quadratic: ...
            @overload
            def __add__(self, rhs: Polynomial) -> Polynomial: ...
            @overload
            def __add__(self, rhs: Function) -> Function: ...

            @overload
            def __radd__(self, lhs: Scalar | LinearLike | Parameter) -> Linear: ...
            @overload
            def __radd__(self, lhs: Quadratic) -> Quadratic: ...
            @overload
            def __radd__(self, lhs: Polynomial) -> Polynomial: ...
            @overload
            def __radd__(self, lhs: Function) -> Function: ...

            @overload
            def __sub__(self, rhs: Scalar | LinearLike | Parameter) -> Linear: ...
            @overload
            def __sub__(self, rhs: Quadratic) -> Quadratic: ...
            @overload
            def __sub__(self, rhs: Polynomial) -> Polynomial: ...
            @overload
            def __sub__(self, rhs: Function) -> Function: ...

            @overload
            def __rsub__(self, lhs: Scalar | LinearLike | Parameter) -> Linear: ...
            @overload
            def __rsub__(self, lhs: Quadratic) -> Quadratic: ...
            @overload
            def __rsub__(self, lhs: Polynomial) -> Polynomial: ...
            @overload
            def __rsub__(self, lhs: Function) -> Function: ...

            @overload
            def __mul__(self, rhs: Scalar) -> Linear: ...
            @overload
            def __mul__(self, rhs: LinearLike | Parameter) -> Quadratic: ...
            @overload
            def __mul__(self, rhs: Quadratic | Polynomial) -> Polynomial: ...
            @overload
            def __mul__(self, rhs: Function) -> Function: ...

            @overload
            def __rmul__(self, lhs: Scalar) -> Linear: ...
            @overload
            def __rmul__(self, lhs: LinearLike | Parameter) -> Quadratic: ...
            @overload
            def __rmul__(self, lhs: Quadratic | Polynomial) -> Polynomial: ...
            @overload
            def __rmul__(self, lhs: Function) -> Function: ...
        "#
    }
}

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl Parameter {
    /// Create a new Parameter.
    ///
    /// **Args:**
    ///
    /// - `id`: Unique identifier for the parameter (must be unique within the instance including decision variables)
    /// - `name`: Optional name for the parameter
    /// - `subscripts`: Optional subscripts for indexing
    /// - `parameters`: Optional metadata key-value pairs
    /// - `description`: Optional human-readable description
    #[new]
    #[pyo3(signature = (id, name=None, subscripts=Vec::new(), parameters=HashMap::default(), description=None))]
    pub fn new(
        id: u64,
        name: Option<String>,
        subscripts: Vec<i64>,
        parameters: HashMap<String, String>,
        description: Option<String>,
    ) -> Self {
        let mut param = ommx::v1::Parameter::default();
        param.id = id;
        param.name = name;
        param.subscripts = subscripts;
        param.parameters = parameters;
        param.description = description;
        Self(param)
    }

    #[getter]
    pub fn id(&self) -> u64 {
        self.0.id
    }

    #[getter]
    pub fn name(&self) -> String {
        self.0.name.clone().unwrap_or_default()
    }

    #[getter]
    pub fn subscripts(&self) -> Vec<i64> {
        self.0.subscripts.clone()
    }

    #[getter]
    pub fn parameters(&self) -> HashMap<String, String> {
        self.0.parameters.clone()
    }

    #[getter]
    pub fn description(&self) -> String {
        self.0.description.clone().unwrap_or_default()
    }

    pub fn __repr__(&self) -> String {
        format!("Parameter(id={}, name=\"{}\")", self.id(), self.name(),)
    }

    fn __copy__(&self) -> Self {
        self.clone()
    }

    fn __deepcopy__(&self, _memo: Bound<'_, PyAny>) -> Self {
        self.clone()
    }

    // =====================
    // Arithmetic Operators
    // =====================

    /// Negation operator: -p → Linear(-1 * p)
    pub fn __neg__(&self) -> Linear {
        Linear(ommx::Linear::single_term(
            LinearMonomial::Variable(VariableID::from(self.0.id)),
            ommx::coeff!(-1.0),
        ))
    }

    /// Polymorphic addition. Dispatches on the operand class of `rhs`
    /// (see `crate::FunctionInput`).
    #[gen_stub(skip)]
    #[pyo3(name = "__add__")]
    pub fn py_add(&self, py: Python<'_>, rhs: crate::FunctionInput) -> PyResult<Py<PyAny>> {
        let self_linear = self.as_linear();
        Ok(match rhs {
            crate::FunctionInput::Scalar(None) => {
                Linear(self_linear).into_pyobject(py)?.into_any().unbind()
            }
            crate::FunctionInput::Scalar(Some(c)) => Linear(&self_linear + c)
                .into_pyobject(py)?
                .into_any()
                .unbind(),
            crate::FunctionInput::Linear(l) => Linear(&self_linear + &l)
                .into_pyobject(py)?
                .into_any()
                .unbind(),
            crate::FunctionInput::Quadratic(q) => Quadratic(&q + &self_linear)
                .into_pyobject(py)?
                .into_any()
                .unbind(),
            crate::FunctionInput::Polynomial(p) => Polynomial(&p + &self_linear)
                .into_pyobject(py)?
                .into_any()
                .unbind(),
            crate::FunctionInput::Function(f) => Function(ommx::Function::from(self_linear) + f)
                .into_pyobject(py)?
                .into_any()
                .unbind(),
        })
    }

    /// Reverse addition (lhs + self)
    #[gen_stub(skip)]
    pub fn __radd__(&self, py: Python<'_>, lhs: crate::FunctionInput) -> PyResult<Py<PyAny>> {
        self.py_add(py, lhs) // Addition is commutative
    }

    /// Polymorphic subtraction. See `py_add`.
    #[gen_stub(skip)]
    #[pyo3(name = "__sub__")]
    pub fn py_sub(&self, py: Python<'_>, rhs: crate::FunctionInput) -> PyResult<Py<PyAny>> {
        let self_linear = self.as_linear();
        Ok(match rhs {
            crate::FunctionInput::Scalar(None) => {
                Linear(self_linear).into_pyobject(py)?.into_any().unbind()
            }
            crate::FunctionInput::Scalar(Some(c)) => Linear(&self_linear - c)
                .into_pyobject(py)?
                .into_any()
                .unbind(),
            crate::FunctionInput::Linear(l) => Linear(&self_linear - &l)
                .into_pyobject(py)?
                .into_any()
                .unbind(),
            crate::FunctionInput::Quadratic(q) => Quadratic(-q + &self_linear)
                .into_pyobject(py)?
                .into_any()
                .unbind(),
            crate::FunctionInput::Polynomial(p) => Polynomial(-p + &self_linear)
                .into_pyobject(py)?
                .into_any()
                .unbind(),
            crate::FunctionInput::Function(f) => Function(ommx::Function::from(self_linear) - f)
                .into_pyobject(py)?
                .into_any()
                .unbind(),
        })
    }

    /// Reverse subtraction (lhs - self)
    #[gen_stub(skip)]
    pub fn __rsub__(&self, py: Python<'_>, lhs: crate::FunctionInput) -> PyResult<Py<PyAny>> {
        let neg = self.__neg__();
        neg.py_add(py, lhs)
    }

    /// Polymorphic multiplication. See `py_add`.
    #[gen_stub(skip)]
    #[pyo3(name = "__mul__")]
    pub fn py_mul(&self, py: Python<'_>, rhs: crate::FunctionInput) -> PyResult<Py<PyAny>> {
        let self_linear = self.as_linear();
        Ok(match rhs {
            crate::FunctionInput::Scalar(None) => Linear(ommx::Linear::default())
                .into_pyobject(py)?
                .into_any()
                .unbind(),
            crate::FunctionInput::Scalar(Some(c)) => Linear(self_linear * c)
                .into_pyobject(py)?
                .into_any()
                .unbind(),
            crate::FunctionInput::Linear(l) => Quadratic(&self_linear * &l)
                .into_pyobject(py)?
                .into_any()
                .unbind(),
            crate::FunctionInput::Quadratic(q) => Polynomial(&self_linear * &q)
                .into_pyobject(py)?
                .into_any()
                .unbind(),
            crate::FunctionInput::Polynomial(p) => Polynomial(&self_linear * &p)
                .into_pyobject(py)?
                .into_any()
                .unbind(),
            crate::FunctionInput::Function(f) => Function(ommx::Function::from(self_linear) * f)
                .into_pyobject(py)?
                .into_any()
                .unbind(),
        })
    }

    /// Reverse multiplication (lhs * self)
    #[gen_stub(skip)]
    pub fn __rmul__(&self, py: Python<'_>, lhs: crate::FunctionInput) -> PyResult<Py<PyAny>> {
        self.py_mul(py, lhs) // Multiplication is commutative
    }

    // =====================
    // Comparison Operators (return Constraint)
    // =====================

    /// Create an equality constraint: self == other → Constraint with EqualToZero
    #[gen_stub(type_ignore = ["override"])]
    #[pyo3(name = "__eq__")]
    pub fn py_eq(&self, other: Function) -> Constraint {
        let mut function = -other.0;
        function += &self.as_linear();
        Constraint(
            ommx::Constraint {
                equality: ommx::Equality::EqualToZero,
                stage: ommx::CreatedData { function },
            },
            ommx::ConstraintMetadata::default(),
        )
    }

    /// Create a less-than-or-equal constraint: self <= other → Constraint
    #[pyo3(name = "__le__")]
    pub fn py_le(&self, other: Function) -> Constraint {
        let mut function = -other.0;
        function += &self.as_linear();
        Constraint(
            ommx::Constraint {
                equality: ommx::Equality::LessThanOrEqualToZero,
                stage: ommx::CreatedData { function },
            },
            ommx::ConstraintMetadata::default(),
        )
    }

    /// Create a greater-than-or-equal constraint: self >= other → Constraint
    #[pyo3(name = "__ge__")]
    pub fn py_ge(&self, other: Function) -> Constraint {
        let function = other.0 - &self.as_linear();
        Constraint(
            ommx::Constraint {
                equality: ommx::Equality::LessThanOrEqualToZero,
                stage: ommx::CreatedData { function },
            },
            ommx::ConstraintMetadata::default(),
        )
    }
}
