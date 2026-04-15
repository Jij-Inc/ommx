use pyo3::prelude::*;
use pyo3_stub_gen::derive::{gen_stub_pyclass, gen_stub_pymethods};
use std::collections::HashMap;

use crate::{constraint::next_constraint_id, DecisionVariable, Equality, Function};

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
    /// - `id`: Optional constraint ID (auto-generated if not provided)
    /// - `name`: Optional name for the constraint
    /// - `subscripts`: Optional subscripts for indexing
    /// - `description`: Optional description
    /// - `parameters`: Optional key-value parameters
    #[new]
    #[pyo3(signature = (*, indicator_variable, function, equality, id=None, name=None, subscripts=Vec::new(), description=None, parameters=HashMap::default()))]
    pub fn new(
        indicator_variable: &DecisionVariable,
        function: Function,
        equality: Equality,
        id: Option<u64>,
        name: Option<String>,
        subscripts: Vec<i64>,
        description: Option<String>,
        parameters: HashMap<String, String>,
    ) -> PyResult<Self> {
        let id = id.unwrap_or_else(next_constraint_id);
        let mut ic = ommx::IndicatorConstraint::new(
            ommx::IndicatorConstraintID::from(id),
            indicator_variable.0.id(),
            equality.into(),
            function.0,
        );
        ic.metadata = ommx::ConstraintMetadata {
            name,
            subscripts,
            parameters: parameters.into_iter().collect(),
            description,
        };
        Ok(Self(ic))
    }

    #[getter]
    pub fn id(&self) -> u64 {
        self.0.id.into_inner()
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

    /// Set the constraint ID. Returns a new IndicatorConstraint.
    pub fn set_id(&self, id: u64) -> Self {
        let mut ic = self.clone();
        ic.0.id = ommx::IndicatorConstraintID::from(id);
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
