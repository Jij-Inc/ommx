use crate::constraint_base::ConstraintView;
use std::collections::BTreeSet;

use pyo3::prelude::*;

#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass(extends = ConstraintView)]
pub struct EvaluatedConstraint(
    pub ommx::EvaluatedConstraint,
    pub ommx::ConstraintContext,
    pub Option<ommx::RemovedReason>,
);
crate::constraint_base::constraint_subclass!(
    EvaluatedConstraint,
    crate::constraint_base::ConstraintView
);

impl EvaluatedConstraint {
    pub fn standalone(inner: ommx::EvaluatedConstraint) -> Self {
        Self(inner, ommx::ConstraintContext::default(), None)
    }

    pub fn from_parts(inner: ommx::EvaluatedConstraint, context: ommx::ConstraintContext) -> Self {
        Self(inner, context, None)
    }
}

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl EvaluatedConstraint {
    /// Get the constraint equality type
    #[getter]
    pub fn equality(&self) -> crate::Equality {
        self.0.equality.into()
    }

    /// Get the evaluated constraint value
    #[getter]
    pub fn evaluated_value(&self) -> f64 {
        self.0.stage.evaluated_value
    }

    /// Get the dual variable value
    #[getter]
    pub fn dual_variable(&self) -> Option<f64> {
        self.0.stage.dual_variable
    }

    /// Set the dual variable value
    #[setter]
    pub fn set_dual_variable(&mut self, value: Option<f64>) {
        self.0.stage.dual_variable = value;
    }

    /// Check feasibility using the supplied absolute tolerance.
    ///
    /// The query compares the stored violation with ``atol``. It does not
    /// re-evaluate the function or canonicalize the input state.
    #[pyo3(signature = (*, atol))]
    pub fn is_feasible(&self, atol: f64) -> crate::error::OmmxPyResult<bool> {
        Ok(self.0.is_feasible(ommx::ATol::new(atol)?))
    }

    /// Get the used decision variable IDs
    #[getter]
    pub fn used_decision_variable_ids(&self) -> BTreeSet<u64> {
        self.0
            .stage
            .used_decision_variable_ids
            .iter()
            .map(|id| id.into_inner())
            .collect()
    }

    /// Calculate the violation (constraint breach) value for this constraint
    ///
    /// Returns the amount by which this constraint is violated:
    /// - For $f(x) = 0$: returns $|f(x)|$
    /// - For $f(x) \leq 0$: returns $\max(0, f(x))$
    ///
    /// Zero implies feasibility. A small positive violation may also be feasible
    /// within the supplied query tolerance; the residual is not rounded to zero.
    pub fn violation(&self) -> f64 {
        self.0.violation()
    }
}

constraint_result_context!(EvaluatedConstraint);
