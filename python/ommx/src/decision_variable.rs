use crate::{Constraint, Function, Linear, Polynomial, Quadratic, VariableBound};
use anyhow::Result;
use ommx::{v1, ATol, LinearMonomial, VariableID};
use pyo3::{prelude::*, Bound, PyAny};
use std::collections::HashMap;

/// Decision variable in an optimization problem.
///
/// This class represents a variable that will be optimized in a mathematical programming problem.
/// It supports various types (binary, integer, continuous, semi-integer, semi-continuous) and
/// can be used in arithmetic expressions to build objective functions and constraints.
///
/// Note that this object overloads `==` for creating a constraint, not for equality comparison.
///
/// # Examples
///
/// ```python
/// >>> x = DecisionVariable.integer(1)
/// >>> x == 1  # Returns Constraint, not bool
/// Constraint(...)
/// ```
///
/// For object equality comparison, use the ``equals_to()`` method or compare IDs:
///
/// ```python
/// >>> y = DecisionVariable.integer(2)
/// >>> x.id == y.id
/// False
/// ```
#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass]
#[derive(Clone)]
pub struct DecisionVariable(
    pub ommx::DecisionVariable,
    pub ommx::DecisionVariableMetadata,
);

impl DecisionVariable {
    /// Helper to create a Linear term from this decision variable with coefficient 1
    fn as_linear(&self) -> ommx::Linear {
        ommx::Linear::single_term(LinearMonomial::Variable(self.0.id()), ommx::coeff!(1.0))
    }

    pub fn standalone(inner: ommx::DecisionVariable) -> Self {
        Self(inner, ommx::DecisionVariableMetadata::default())
    }

    pub fn from_parts(
        inner: ommx::DecisionVariable,
        metadata: ommx::DecisionVariableMetadata,
    ) -> Self {
        Self(inner, metadata)
    }
}

// Overload stubs for arithmetic operators.
// Must appear before #[gen_stub_pymethods] for correct ordering.
pyo3_stub_gen::inventory::submit! {
    pyo3_stub_gen::derive::gen_methods_from_python! {
        r#"
        class DecisionVariable:
            @overload
            def __add__(self, rhs: ScalarLike | LinearLike | Parameter) -> Linear: ...
            @overload
            def __add__(self, rhs: Quadratic) -> Quadratic: ...
            @overload
            def __add__(self, rhs: Polynomial) -> Polynomial: ...
            @overload
            def __add__(self, rhs: Function) -> Function: ...

            @overload
            def __radd__(self, lhs: ScalarLike | LinearLike | Parameter) -> Linear: ...
            @overload
            def __radd__(self, lhs: Quadratic) -> Quadratic: ...
            @overload
            def __radd__(self, lhs: Polynomial) -> Polynomial: ...
            @overload
            def __radd__(self, lhs: Function) -> Function: ...

            @overload
            def __sub__(self, rhs: ScalarLike | LinearLike | Parameter) -> Linear: ...
            @overload
            def __sub__(self, rhs: Quadratic) -> Quadratic: ...
            @overload
            def __sub__(self, rhs: Polynomial) -> Polynomial: ...
            @overload
            def __sub__(self, rhs: Function) -> Function: ...

            @overload
            def __rsub__(self, lhs: ScalarLike | LinearLike | Parameter) -> Linear: ...
            @overload
            def __rsub__(self, lhs: Quadratic) -> Quadratic: ...
            @overload
            def __rsub__(self, lhs: Polynomial) -> Polynomial: ...
            @overload
            def __rsub__(self, lhs: Function) -> Function: ...

            @overload
            def __mul__(self, rhs: ScalarLike) -> Linear: ...
            @overload
            def __mul__(self, rhs: LinearLike | Parameter) -> Quadratic: ...
            @overload
            def __mul__(self, rhs: Quadratic | Polynomial) -> Polynomial: ...
            @overload
            def __mul__(self, rhs: Function) -> Function: ...

            @overload
            def __rmul__(self, lhs: ScalarLike) -> Linear: ...
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
impl DecisionVariable {
    #[new]
    #[pyo3(signature = (id, kind, bound, name=None, subscripts=Vec::new(), parameters=HashMap::default(), description=None))]
    pub fn new(
        id: u64,
        kind: i32,
        bound: VariableBound,
        name: Option<String>,
        subscripts: Vec<i64>,
        parameters: HashMap<String, String>,
        description: Option<String>,
    ) -> Result<Self> {
        let variable_id = VariableID::from(id);
        let kind = v1::decision_variable::Kind::try_from(kind)?.try_into()?;

        let decision_variable = ommx::DecisionVariable::new(
            variable_id,
            kind,
            bound.0,
            None, // substituted_value
            ATol::default(),
        )?;

        let metadata = ommx::DecisionVariableMetadata {
            name,
            subscripts,
            parameters: parameters.into_iter().collect(),
            description,
        };

        Ok(Self(decision_variable, metadata))
    }

    #[getter]
    pub fn id(&self) -> u64 {
        self.0.id().into_inner()
    }

    #[getter]
    pub fn kind(&self) -> i32 {
        let kind: v1::decision_variable::Kind = self.0.kind().into();
        kind as i32
    }

    #[getter]
    pub fn bound(&self) -> VariableBound {
        VariableBound(self.0.bound())
    }

    #[getter]
    pub fn name(&self) -> String {
        self.1.name.clone().unwrap_or_default()
    }

    #[getter]
    pub fn subscripts(&self) -> Vec<i64> {
        self.1.subscripts.clone()
    }

    #[getter]
    pub fn parameters(&self) -> HashMap<String, String> {
        self.1
            .parameters
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }

    #[getter]
    pub fn description(&self) -> String {
        self.1.description.clone().unwrap_or_default()
    }

    #[getter]
    pub fn substituted_value(&self) -> Option<f64> {
        self.0.substituted_value()
    }

    #[staticmethod]
    #[pyo3(signature = (id, name=None, subscripts=Vec::new(), parameters=HashMap::default(), description=None))]
    pub fn binary(
        id: u64,
        name: Option<String>,
        subscripts: Vec<i64>,
        parameters: HashMap<String, String>,
        description: Option<String>,
    ) -> Result<Self> {
        Self::new(
            id,
            1, // KIND_BINARY
            VariableBound(ommx::Bound::of_binary()),
            name,
            subscripts,
            parameters,
            description,
        )
    }

    #[staticmethod]
    #[pyo3(signature = (id, lower=f64::NEG_INFINITY, upper=f64::INFINITY, name=None, subscripts=Vec::new(), parameters=HashMap::default(), description=None))]
    pub fn integer(
        id: u64,
        lower: f64,
        upper: f64,
        name: Option<String>,
        subscripts: Vec<i64>,
        parameters: HashMap<String, String>,
        description: Option<String>,
    ) -> Result<Self> {
        Self::new(
            id,
            2, // KIND_INTEGER
            VariableBound(ommx::Bound::new(lower, upper)?),
            name,
            subscripts,
            parameters,
            description,
        )
    }

    #[staticmethod]
    #[pyo3(signature = (id, lower=f64::NEG_INFINITY, upper=f64::INFINITY, name=None, subscripts=Vec::new(), parameters=HashMap::default(), description=None))]
    pub fn continuous(
        id: u64,
        lower: f64,
        upper: f64,
        name: Option<String>,
        subscripts: Vec<i64>,
        parameters: HashMap<String, String>,
        description: Option<String>,
    ) -> Result<Self> {
        Self::new(
            id,
            3, // KIND_CONTINUOUS
            VariableBound(ommx::Bound::new(lower, upper)?),
            name,
            subscripts,
            parameters,
            description,
        )
    }

    #[staticmethod]
    #[pyo3(signature = (id, lower=f64::NEG_INFINITY, upper=f64::INFINITY, name=None, subscripts=Vec::new(), parameters=HashMap::default(), description=None))]
    pub fn semi_integer(
        id: u64,
        lower: f64,
        upper: f64,
        name: Option<String>,
        subscripts: Vec<i64>,
        parameters: HashMap<String, String>,
        description: Option<String>,
    ) -> Result<Self> {
        Self::new(
            id,
            4, // KIND_SEMI_INTEGER
            VariableBound(ommx::Bound::new(lower, upper)?),
            name,
            subscripts,
            parameters,
            description,
        )
    }

    #[staticmethod]
    #[pyo3(signature = (id, lower=f64::NEG_INFINITY, upper=f64::INFINITY, name=None, subscripts=Vec::new(), parameters=HashMap::default(), description=None))]
    pub fn semi_continuous(
        id: u64,
        lower: f64,
        upper: f64,
        name: Option<String>,
        subscripts: Vec<i64>,
        parameters: HashMap<String, String>,
        description: Option<String>,
    ) -> Result<Self> {
        Self::new(
            id,
            5, // KIND_SEMI_CONTINUOUS
            VariableBound(ommx::Bound::new(lower, upper)?),
            name,
            subscripts,
            parameters,
            description,
        )
    }

    pub fn __repr__(&self) -> String {
        format!(
            "DecisionVariable(id={}, kind={}, name=\"{}\", bound=[{}, {}])",
            self.id(),
            self.kind(),
            self.name(),
            self.0.bound().lower(),
            self.0.bound().upper()
        )
    }

    fn __copy__(&self) -> Self {
        self.clone()
    }

    // __deepcopy__ can also be implemented with self.clone()
    // memo argument is required to match Python protocol but not used in this implementation
    // Since this implementation contains no PyObject references, simple clone is sufficient
    fn __deepcopy__(&self, _memo: Bound<'_, PyAny>) -> Self {
        self.clone()
    }

    // =====================
    // Class-level constants for variable kinds
    // =====================

    #[classattr]
    const BINARY: i32 = 1;

    #[classattr]
    const INTEGER: i32 = 2;

    #[classattr]
    const CONTINUOUS: i32 = 3;

    #[classattr]
    const SEMI_INTEGER: i32 = 4;

    #[classattr]
    const SEMI_CONTINUOUS: i32 = 5;

    // =====================
    // Comparison for equality (not constraint creation)
    // =====================

    /// Compare two DecisionVariable objects for equality.
    ///
    /// This is different from `__eq__` which creates a Constraint.
    /// Use this method when you want to check if two variables represent the same variable.
    pub fn equals_to(&self, other: &DecisionVariable) -> bool {
        self.0.id() == other.0.id()
            && self.0.kind() == other.0.kind()
            && self.0.bound() == other.0.bound()
    }

    // =====================
    // Arithmetic Operators
    // =====================

    /// Negation operator: -x → Linear(-1 * x)
    pub fn __neg__(&self) -> Linear {
        Linear(ommx::Linear::single_term(
            LinearMonomial::Variable(self.0.id()),
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
        // lhs - self = -self + lhs
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

/// Attached decision variable — a write-through handle bound to a host
/// ({class}`~ommx.v1.Instance` or {class}`~ommx.v1.ParametricInstance`).
///
/// `AttachedDecisionVariable` is returned by `add_decision_variable(v)`
/// (insertion), `attached_decision_variable(id)` (lookup), and the
/// `decision_variables` getter on both hosts. Reads pull live data from
/// the parent host's SoA store and metadata setters write back through to
/// it. Handles also participate in arithmetic (`x + y`, `2 * x` etc.) via
/// `ToFunction` — only the id is consumed for that, no host borrow is
/// taken, so arithmetic works even while the host is mutably borrowed
/// elsewhere. Call {meth}`detach` for an independent
/// {class}`~ommx.v1.DecisionVariable` snapshot.
///
/// `DecisionVariableMetadata` has no `provenance` field, so the
/// write-through surface omits the corresponding getter.
#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass]
pub struct AttachedDecisionVariable {
    pub(crate) host: crate::ConstraintHost,
    pub(crate) id: ommx::VariableID,
}

impl AttachedDecisionVariable {
    pub fn new(host: crate::ConstraintHost, id: ommx::VariableID) -> Self {
        Self { host, id }
    }

    pub fn from_instance(instance: Py<crate::Instance>, id: ommx::VariableID) -> Self {
        Self::new(crate::ConstraintHost::Instance(instance), id)
    }

    pub fn from_parametric(
        parametric: Py<crate::ParametricInstance>,
        id: ommx::VariableID,
    ) -> Self {
        Self::new(crate::ConstraintHost::Parametric(parametric), id)
    }

    /// Build a `Linear` term from this handle's id with coefficient 1.
    ///
    /// Used by the polymorphic arithmetic operators. Only the id is needed —
    /// no host borrow is taken — so this works even if the host is currently
    /// borrowed mutably.
    fn as_linear(&self) -> ommx::Linear {
        ommx::Linear::single_term(LinearMonomial::Variable(self.id), ommx::coeff!(1.0))
    }
}

// Overload stubs for arithmetic operators on AttachedDecisionVariable.
// Mirrors the DecisionVariable overloads — same semantics, since the only thing
// arithmetic uses is the id.
pyo3_stub_gen::inventory::submit! {
    pyo3_stub_gen::derive::gen_methods_from_python! {
        r#"
        class AttachedDecisionVariable:
            @overload
            def __add__(self, rhs: ScalarLike | LinearLike | Parameter) -> Linear: ...
            @overload
            def __add__(self, rhs: Quadratic) -> Quadratic: ...
            @overload
            def __add__(self, rhs: Polynomial) -> Polynomial: ...
            @overload
            def __add__(self, rhs: Function) -> Function: ...

            @overload
            def __radd__(self, lhs: ScalarLike | LinearLike | Parameter) -> Linear: ...
            @overload
            def __radd__(self, lhs: Quadratic) -> Quadratic: ...
            @overload
            def __radd__(self, lhs: Polynomial) -> Polynomial: ...
            @overload
            def __radd__(self, lhs: Function) -> Function: ...

            @overload
            def __sub__(self, rhs: ScalarLike | LinearLike | Parameter) -> Linear: ...
            @overload
            def __sub__(self, rhs: Quadratic) -> Quadratic: ...
            @overload
            def __sub__(self, rhs: Polynomial) -> Polynomial: ...
            @overload
            def __sub__(self, rhs: Function) -> Function: ...

            @overload
            def __rsub__(self, lhs: ScalarLike | LinearLike | Parameter) -> Linear: ...
            @overload
            def __rsub__(self, lhs: Quadratic) -> Quadratic: ...
            @overload
            def __rsub__(self, lhs: Polynomial) -> Polynomial: ...
            @overload
            def __rsub__(self, lhs: Function) -> Function: ...

            @overload
            def __mul__(self, rhs: ScalarLike) -> Linear: ...
            @overload
            def __mul__(self, rhs: LinearLike | Parameter) -> Quadratic: ...
            @overload
            def __mul__(self, rhs: Quadratic | Polynomial) -> Polynomial: ...
            @overload
            def __mul__(self, rhs: Function) -> Function: ...

            @overload
            def __rmul__(self, lhs: ScalarLike) -> Linear: ...
            @overload
            def __rmul__(self, lhs: LinearLike | Parameter) -> Quadratic: ...
            @overload
            def __rmul__(self, lhs: Quadratic | Polynomial) -> Polynomial: ...
            @overload
            def __rmul__(self, lhs: Function) -> Function: ...
        "#
    }
}

fn lookup_variable(
    inst: &ommx::Instance,
    id: ommx::VariableID,
) -> pyo3::PyResult<&ommx::DecisionVariable> {
    inst.decision_variables().get(&id).ok_or_else(|| {
        pyo3::exceptions::PyKeyError::new_err(format!(
            "decision variable id {} not found in instance",
            id.into_inner()
        ))
    })
}

fn lookup_variable_parametric(
    inst: &ommx::ParametricInstance,
    id: ommx::VariableID,
) -> pyo3::PyResult<&ommx::DecisionVariable> {
    inst.decision_variables().get(&id).ok_or_else(|| {
        pyo3::exceptions::PyKeyError::new_err(format!(
            "decision variable id {} not found in parametric instance",
            id.into_inner()
        ))
    })
}

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl AttachedDecisionVariable {
    /// The id this handle points at.
    #[getter]
    pub fn id(&self) -> u64 {
        self.id.into_inner()
    }

    /// The parent host this variable lives in.
    #[getter]
    pub fn instance(&self, py: Python<'_>) -> Py<PyAny> {
        match &self.host {
            crate::ConstraintHost::Instance(p) => p.clone_ref(py).into_any(),
            crate::ConstraintHost::Parametric(p) => p.clone_ref(py).into_any(),
        }
    }

    /// Return a {class}`~ommx.v1.DecisionVariable` snapshot of the current
    /// state. Mutations on the returned object do not propagate back.
    pub fn detach(&self, py: Python<'_>) -> PyResult<DecisionVariable> {
        match &self.host {
            crate::ConstraintHost::Instance(p) => {
                let inst = p.borrow(py);
                let v = lookup_variable(&inst.inner, self.id)?.clone();
                let metadata = inst.inner.variable_metadata().collect_for(self.id);
                Ok(DecisionVariable::from_parts(v, metadata))
            }
            crate::ConstraintHost::Parametric(p) => {
                let inst = p.borrow(py);
                let v = lookup_variable_parametric(&inst.inner, self.id)?.clone();
                let metadata = inst.inner.variable_metadata().collect_for(self.id);
                Ok(DecisionVariable::from_parts(v, metadata))
            }
        }
    }

    #[getter]
    pub fn kind(&self, py: Python<'_>) -> PyResult<i32> {
        let with = |v: &ommx::DecisionVariable| -> i32 {
            let kind: ommx::v1::decision_variable::Kind = v.kind().into();
            kind as i32
        };
        match &self.host {
            crate::ConstraintHost::Instance(p) => {
                let inst = p.borrow(py);
                Ok(with(lookup_variable(&inst.inner, self.id)?))
            }
            crate::ConstraintHost::Parametric(p) => {
                let inst = p.borrow(py);
                Ok(with(lookup_variable_parametric(&inst.inner, self.id)?))
            }
        }
    }

    #[getter]
    pub fn bound(&self, py: Python<'_>) -> PyResult<VariableBound> {
        match &self.host {
            crate::ConstraintHost::Instance(p) => {
                let inst = p.borrow(py);
                Ok(VariableBound(
                    lookup_variable(&inst.inner, self.id)?.bound(),
                ))
            }
            crate::ConstraintHost::Parametric(p) => {
                let inst = p.borrow(py);
                Ok(VariableBound(
                    lookup_variable_parametric(&inst.inner, self.id)?.bound(),
                ))
            }
        }
    }

    #[getter]
    pub fn substituted_value(&self, py: Python<'_>) -> PyResult<Option<f64>> {
        match &self.host {
            crate::ConstraintHost::Instance(p) => {
                let inst = p.borrow(py);
                Ok(lookup_variable(&inst.inner, self.id)?.substituted_value())
            }
            crate::ConstraintHost::Parametric(p) => {
                let inst = p.borrow(py);
                Ok(lookup_variable_parametric(&inst.inner, self.id)?.substituted_value())
            }
        }
    }

    pub fn __repr__(&self, py: Python<'_>) -> String {
        let render = |v: &ommx::DecisionVariable, name: Option<&str>| -> String {
            let kind: ommx::v1::decision_variable::Kind = v.kind().into();
            format!(
                "AttachedDecisionVariable(id={}, kind={}, name=\"{}\", bound=[{}, {}])",
                v.id().into_inner(),
                kind as i32,
                name.unwrap_or(""),
                v.bound().lower(),
                v.bound().upper()
            )
        };
        match &self.host {
            crate::ConstraintHost::Instance(p) => {
                let inst = p.borrow(py);
                match lookup_variable(&inst.inner, self.id) {
                    Ok(v) => render(v, inst.inner.variable_metadata().name(self.id)),
                    Err(_) => format!(
                        "AttachedDecisionVariable(id={}, dropped)",
                        self.id.into_inner()
                    ),
                }
            }
            crate::ConstraintHost::Parametric(p) => {
                let inst = p.borrow(py);
                match lookup_variable_parametric(&inst.inner, self.id) {
                    Ok(v) => render(v, inst.inner.variable_metadata().name(self.id)),
                    Err(_) => format!(
                        "AttachedDecisionVariable(id={}, dropped)",
                        self.id.into_inner()
                    ),
                }
            }
        }
    }

    fn __copy__(&self, py: Python<'_>) -> Self {
        Self {
            host: self.host.clone_ref(py),
            id: self.id,
        }
    }

    fn __deepcopy__(&self, py: Python<'_>, _memo: Bound<'_, PyAny>) -> Self {
        self.__copy__(py)
    }

    // =====================
    // Arithmetic Operators (mirror DecisionVariable; only the id is used)
    // =====================

    /// Negation operator: `-x` → `Linear(-1 * x)`.
    pub fn __neg__(&self) -> Linear {
        Linear(ommx::Linear::single_term(
            LinearMonomial::Variable(self.id),
            ommx::coeff!(-1.0),
        ))
    }

    /// Polymorphic addition. See `DecisionVariable::py_add` —
    /// the dispatch on `crate::FunctionInput` is identical.
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

    #[gen_stub(skip)]
    pub fn __radd__(&self, py: Python<'_>, lhs: crate::FunctionInput) -> PyResult<Py<PyAny>> {
        self.py_add(py, lhs)
    }

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

    #[gen_stub(skip)]
    pub fn __rsub__(&self, py: Python<'_>, lhs: crate::FunctionInput) -> PyResult<Py<PyAny>> {
        let neg = self.__neg__();
        neg.py_add(py, lhs)
    }

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

    #[gen_stub(skip)]
    pub fn __rmul__(&self, py: Python<'_>, lhs: crate::FunctionInput) -> PyResult<Py<PyAny>> {
        self.py_mul(py, lhs)
    }

    // =====================
    // Comparison Operators (return Constraint)
    // =====================

    /// Create an equality constraint: `self == other` → `Constraint` with `EqualToZero`.
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

    /// Create a less-than-or-equal constraint.
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

    /// Create a greater-than-or-equal constraint.
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

crate::attached_variable_metadata_methods!(
    AttachedDecisionVariable,
    variable_metadata,
    variable_metadata_mut
);
