use ommx::{SampledConstraintBehavior, SampledConstraintData};
use pyo3::prelude::*;
use std::collections::{BTreeMap, BTreeSet};

#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass]
pub struct SampledConstraint(
    pub ommx::SampledConstraint,
    pub ommx::ConstraintContext,
    pub Option<ommx::RemovedReason>,
);

impl SampledConstraint {
    pub fn standalone(inner: ommx::SampledConstraint) -> Self {
        Self(inner, ommx::ConstraintContext::default(), None)
    }

    pub fn from_parts(inner: ommx::SampledConstraint, context: ommx::ConstraintContext) -> Self {
        Self(inner, context, None)
    }
}

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl SampledConstraint {
    /// Get the constraint equality type
    #[getter]
    pub fn equality(&self) -> crate::Equality {
        self.0.equality.into()
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

    /// Get the evaluated values for all samples
    #[getter]
    pub fn evaluated_values(&self) -> BTreeMap<u64, f64> {
        self.0
            .stage
            .evaluated_values
            .iter()
            .map(|(&sample_id, value)| (sample_id.into_inner(), *value))
            .collect()
    }

    /// Compute feasibility for every sample using the supplied tolerance.
    ///
    /// This query does not re-evaluate the functions or change stored values.
    #[pyo3(signature = (*, atol))]
    pub fn feasible(&self, atol: f64) -> crate::error::OmmxPyResult<BTreeMap<u64, bool>> {
        let atol = ommx::ATol::new(atol)?;
        Ok(self
            .0
            .stage
            .evaluated_values
            .iter()
            .map(|(&sample_id, _)| {
                (
                    sample_id.into_inner(),
                    self.0
                        .is_feasible_for(sample_id, atol)
                        .expect("sample exists"),
                )
            })
            .collect())
    }

    /// Nonnegative violations keyed by sample ID.
    #[getter]
    pub fn violations(&self) -> BTreeMap<u64, f64> {
        self.0
            .stage
            .evaluated_values
            .iter()
            .map(|(id, _)| {
                (
                    id.into_inner(),
                    self.0.violation_for(*id).expect("validated sample ID"),
                )
            })
            .collect()
    }
}

constraint_result_context!(SampledConstraint);
