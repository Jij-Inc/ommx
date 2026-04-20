use pyo3::prelude::*;
use pyo3_stub_gen::derive::{gen_stub_pyclass, gen_stub_pymethods};
use std::collections::HashMap;

use crate::{DecisionVariable, Equality, Function};

#[gen_stub_pyclass]
#[pyclass]
#[derive(Clone)]
pub struct IndicatorConstraint(pub ommx::IndicatorConstraint);

#[gen_stub_pymethods]
#[pymethods]
impl IndicatorConstraint {
    /// Create a new indicator constraint.
    ///
    /// An indicator constraint is: `indicator_variable = 1 → f(x) <= 0` (or `f(x) = 0`).
    ///
    /// **Args:**
    ///
    /// - `indicator_variable`: A binary decision variable that activates this constraint
    /// - `function`: The constraint function
    /// - `equality`: The equality type (EqualToZero or LessThanOrEqualToZero)
    /// - `name`: Optional name for the constraint
    /// - `subscripts`: Optional subscripts for indexing
    /// - `description`: Optional description
    /// - `parameters`: Optional key-value parameters
    #[new]
    #[pyo3(signature = (*, indicator_variable, function, equality, name=None, subscripts=Vec::new(), description=None, parameters=HashMap::default()))]
    pub fn new(
        indicator_variable: &DecisionVariable,
        function: Function,
        equality: Equality,
        name: Option<String>,
        subscripts: Vec<i64>,
        description: Option<String>,
        parameters: HashMap<String, String>,
    ) -> PyResult<Self> {
        let mut ic =
            ommx::IndicatorConstraint::new(indicator_variable.0.id(), equality.into(), function.0);
        ic.metadata = ommx::ConstraintMetadata {
            name,
            subscripts,
            parameters: parameters.into_iter().collect(),
            description,
            provenance: Vec::new(),
        };
        Ok(Self(ic))
    }

    #[getter]
    pub fn indicator_variable_id(&self) -> u64 {
        self.0.indicator_variable.into_inner()
    }

    #[getter]
    pub fn function(&self) -> Function {
        Function(self.0.function().clone())
    }

    #[getter]
    pub fn equality(&self) -> Equality {
        self.0.equality.into()
    }

    #[getter]
    pub fn name(&self) -> Option<String> {
        self.0.metadata.name.clone()
    }

    #[getter]
    pub fn subscripts(&self) -> Vec<i64> {
        self.0.metadata.subscripts.clone()
    }

    #[getter]
    pub fn description(&self) -> Option<String> {
        self.0.metadata.description.clone()
    }

    #[getter]
    pub fn parameters(&self) -> HashMap<String, String> {
        self.0.metadata.parameters.clone().into_iter().collect()
    }

    /// Set the constraint name. Returns a new IndicatorConstraint.
    pub fn set_name(&self, name: String) -> Self {
        let mut ic = self.clone();
        ic.0.metadata.name = Some(name);
        ic
    }

    fn __repr__(&self) -> String {
        format!("{}", self.0)
    }

    fn __copy__(&self) -> Self {
        self.clone()
    }

    fn __deepcopy__(&self, _memo: pyo3::Bound<pyo3::types::PyAny>) -> Self {
        self.clone()
    }
}

/// A removed indicator constraint together with the reason it was removed.
#[gen_stub_pyclass]
#[pyclass]
#[derive(Clone)]
pub struct RemovedIndicatorConstraint {
    pub constraint: ommx::IndicatorConstraint,
    pub removed_reason: ommx::RemovedReason,
}

impl RemovedIndicatorConstraint {
    pub fn from_pair(
        constraint: ommx::IndicatorConstraint,
        removed_reason: ommx::RemovedReason,
    ) -> Self {
        Self {
            constraint,
            removed_reason,
        }
    }
}

#[gen_stub_pymethods]
#[pymethods]
impl RemovedIndicatorConstraint {
    #[getter]
    pub fn constraint(&self) -> IndicatorConstraint {
        IndicatorConstraint(self.constraint.clone())
    }

    #[getter]
    pub fn indicator_variable_id(&self) -> u64 {
        self.constraint.indicator_variable.into_inner()
    }

    #[getter]
    pub fn equality(&self) -> Equality {
        self.constraint.equality.into()
    }

    #[getter]
    pub fn function(&self) -> Function {
        Function(self.constraint.function().clone())
    }

    #[getter]
    pub fn removed_reason(&self) -> String {
        self.removed_reason.reason.clone()
    }

    #[getter]
    pub fn removed_reason_parameters(&self) -> HashMap<String, String> {
        self.removed_reason
            .parameters
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }

    fn __repr__(&self) -> String {
        let mut extras: Vec<String> = self
            .removed_reason
            .parameters
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect();
        extras.sort();
        let mut head = format!("{}, reason={}", self.constraint, self.removed_reason.reason);
        if !extras.is_empty() {
            head.push_str(", ");
            head.push_str(&extras.join(", "));
        }
        format!("RemovedIndicatorConstraint({head})")
    }
}
