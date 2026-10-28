//! Owned Python snapshots of evaluated constraints, selected by family.
//!
//! Root objects retain identity and lifecycle ownership. These wrappers copy
//! validated rows and their context; they cannot mutate the enclosing result.

use crate::constraint_base::ConstraintView;
use ommx::{EvaluatedConstraintBehavior, EvaluatedConstraintData, SampledConstraintData};
use pyo3::prelude::*;
use std::collections::{BTreeMap, BTreeSet};

/// Shared context for all result rows. IDs remain in the enclosing dictionary.
macro_rules! constraint_result_context {
    ($name:ident) => {
        impl $name {
            /// Copy the owning result's lifecycle metadata into this snapshot.
            pub fn with_removed_reason(mut self, reason: Option<ommx::RemovedReason>) -> Self {
                self.2 = reason;
                self
            }
        }

        #[pyo3_stub_gen::derive::gen_stub_pymethods]
        #[pymethods]
        impl $name {
            /// Modeling name, if supplied.
            #[getter]
            pub fn name(&self) -> Option<String> {
                self.1.label.name.clone()
            }

            /// Modeling subscripts.
            #[getter]
            pub fn subscripts(&self) -> Vec<i64> {
                self.1.label.subscripts.clone()
            }

            /// Modeling parameters.
            #[getter]
            pub fn parameters(&self) -> std::collections::HashMap<String, String> {
                self.1
                    .label
                    .parameters
                    .iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect()
            }

            /// Modeling description, if supplied.
            #[getter]
            pub fn description(&self) -> Option<String> {
                self.1.label.description.clone()
            }

            /// Transformation lineage, preserving the source constraint family.
            #[getter]
            pub fn provenance(&self) -> Vec<crate::Provenance> {
                crate::provenance_list(&self.1)
            }

            /// Reason this constraint was removed before evaluation, or None.
            #[getter]
            pub fn removed_reason(&self) -> Option<String> {
                self.2.as_ref().map(|r| r.reason.clone())
            }

            /// Parameters of the removal reason; empty for an active constraint.
            #[getter]
            pub fn removed_reason_parameters(&self) -> std::collections::HashMap<String, String> {
                self.2
                    .as_ref()
                    .map(|r| {
                        r.parameters
                            .iter()
                            .map(|(k, v)| (k.clone(), v.clone()))
                            .collect()
                    })
                    .unwrap_or_default()
            }
        }
    };
}

macro_rules! result_class {
    ($name:ident, $inner:ty, $doc:literal) => {
        #[doc = $doc]
        #[pyo3_stub_gen::derive::gen_stub_pyclass]
        #[pyclass(eq, extends = ConstraintView)]
        #[derive(PartialEq)]
        pub struct $name($inner, ommx::ConstraintContext, Option<ommx::RemovedReason>);
        crate::constraint_base::constraint_subclass!($name, crate::constraint_base::ConstraintView);

        impl $name {
            /// The result collection builds one owned row with its owner-held context.
            pub(crate) fn from_parts(inner: $inner, context: ommx::ConstraintContext) -> Self {
                Self(inner, context, None)
            }
        }
        constraint_result_context!($name);
    };
}

result_class!(EvaluatedIndicatorConstraint, ommx::EvaluatedIndicatorConstraint, "An owned evaluated indicator result. Fetched by typed ID from the owning result's constraints collection.");
result_class!(SampledIndicatorConstraint, ommx::SampledIndicatorConstraint, "An owned sampled indicator result. Fetched by typed ID from the owning result's constraints collection.");
result_class!(EvaluatedOneHotConstraint, ommx::EvaluatedOneHotConstraint, "An owned evaluated one-hot result. Fetched by typed ID from the owning result's constraints collection.");
result_class!(SampledOneHotConstraint, ommx::SampledOneHotConstraint, "An owned sampled one-hot result. Fetched by typed ID from the owning result's constraints collection.");
result_class!(EvaluatedSos1Constraint, ommx::EvaluatedSos1Constraint, "An owned evaluated SOS1 result. Fetched by typed ID from the owning result's constraints collection.");
result_class!(SampledSos1Constraint, ommx::SampledSos1Constraint, "An owned sampled SOS1 result. Fetched by typed ID from the owning result's constraints collection.");

macro_rules! evaluated_methods {
    ($name:ident) => {
        #[pyo3_stub_gen::derive::gen_stub_pymethods]
        #[pymethods]
        impl $name {
            /// Nonnegative violation retained by the evaluation.
            pub fn violation(&self) -> f64 {
                self.0.violation()
            }

            /// Compare the stored violation with atol without changing activation diagnostics.
            #[pyo3(signature = (*, atol))]
            pub fn is_feasible(&self, atol: f64) -> crate::error::OmmxPyResult<bool> {
                Ok(self.0.is_feasible(ommx::ATol::new(atol)?))
            }

            /// Tolerance used when the activation diagnostic was computed.
            #[getter]
            pub fn activation_atol(&self) -> f64 {
                self.0.stage.activation_atol.into_inner()
            }

            /// IDs of decision variables used to evaluate this constraint.
            #[getter]
            pub fn used_decision_variable_ids(&self) -> BTreeSet<u64> {
                self.0
                    .used_decision_variable_ids()
                    .iter()
                    .map(|id| id.into_inner())
                    .collect()
            }
        }
    };
}

macro_rules! sampled_methods {
    ($name:ident, $values:ident) => {
        #[pyo3_stub_gen::derive::gen_stub_pymethods]
        #[pymethods]
        impl $name {
            /// Nonnegative violations keyed by sample ID.
            #[getter]
            pub fn violations(&self) -> BTreeMap<u64, f64> {
                self.0
                    .stage
                    .$values
                    .iter()
                    .map(|(id, _)| {
                        (
                            id.into_inner(),
                            self.0.violation_for(*id).expect("validated sample ID"),
                        )
                    })
                    .collect()
            }

            /// Compare each stored violation with atol without changing activation diagnostics.
            #[pyo3(signature = (*, atol))]
            pub fn feasible(&self, atol: f64) -> crate::error::OmmxPyResult<BTreeMap<u64, bool>> {
                use ommx::SampledConstraintBehavior;
                let atol = ommx::ATol::new(atol)?;
                Ok(self
                    .0
                    .stage
                    .$values
                    .iter()
                    .map(|(id, _)| {
                        (
                            id.into_inner(),
                            self.0
                                .is_feasible_for(*id, atol)
                                .expect("validated sample ID"),
                        )
                    })
                    .collect())
            }

            /// Tolerance used when the activation diagnostics were computed.
            #[getter]
            pub fn activation_atol(&self) -> f64 {
                self.0.stage.activation_atol.into_inner()
            }

            /// IDs of decision variables used to evaluate this constraint.
            #[getter]
            pub fn used_decision_variable_ids(&self) -> BTreeSet<u64> {
                self.0
                    .used_decision_variable_ids()
                    .iter()
                    .map(|id| id.into_inner())
                    .collect()
            }
        }
    };
}

evaluated_methods!(EvaluatedIndicatorConstraint);
evaluated_methods!(EvaluatedOneHotConstraint);
evaluated_methods!(EvaluatedSos1Constraint);
sampled_methods!(SampledIndicatorConstraint, evaluated_values);
sampled_methods!(SampledOneHotConstraint, violations);
sampled_methods!(SampledSos1Constraint, violations);

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl EvaluatedIndicatorConstraint {
    /// Decision variable that activates this constraint.
    #[getter]
    pub fn indicator_variable_id(&self) -> u64 {
        self.0.indicator_variable.into_inner()
    }

    /// Equality of the inner constraint.
    #[getter]
    pub fn equality(&self) -> crate::Equality {
        self.0.equality.into()
    }

    /// Inner residual at the evaluated state.
    #[getter]
    pub fn evaluated_value(&self) -> f64 {
        self.0.stage.evaluated_value
    }

    /// Whether the indicator was active at evaluation time.
    #[getter]
    pub fn indicator_active(&self) -> bool {
        self.0.stage.indicator_active
    }
}

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl EvaluatedOneHotConstraint {
    /// Member decision variable IDs.
    #[getter]
    pub fn variables(&self) -> BTreeSet<u64> {
        self.0.variables.iter().map(|id| id.into_inner()).collect()
    }

    /// Retained selected-variable diagnostic, using activation_atol.
    #[getter]
    pub fn active_variable(&self) -> Option<u64> {
        self.0.stage.active_variable.map(|id| id.into_inner())
    }
}

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl EvaluatedSos1Constraint {
    /// Member decision variable IDs.
    #[getter]
    pub fn variables(&self) -> BTreeSet<u64> {
        self.0.variables.iter().map(|id| id.into_inner()).collect()
    }

    /// Retained selected-variable diagnostic, using activation_atol.
    #[getter]
    pub fn active_variable(&self) -> Option<u64> {
        self.0.stage.active_variable.map(|id| id.into_inner())
    }
}

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl SampledIndicatorConstraint {
    /// Decision variable that activates this constraint.
    #[getter]
    pub fn indicator_variable_id(&self) -> u64 {
        self.0.indicator_variable.into_inner()
    }

    /// Equality of the inner constraint.
    #[getter]
    pub fn equality(&self) -> crate::Equality {
        self.0.equality.into()
    }

    /// Inner residuals keyed by sample ID.
    #[getter]
    pub fn evaluated_values(&self) -> BTreeMap<u64, f64> {
        self.0
            .stage
            .evaluated_values
            .iter()
            .map(|(id, value)| (id.into_inner(), *value))
            .collect()
    }

    /// Activation diagnostics keyed by sample ID.
    #[getter]
    pub fn indicator_active(&self) -> BTreeMap<u64, bool> {
        self.0
            .stage
            .indicator_active
            .iter()
            .map(|(id, active)| (id.into_inner(), *active))
            .collect()
    }
}

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl SampledOneHotConstraint {
    /// Member decision variable IDs.
    #[getter]
    pub fn variables(&self) -> BTreeSet<u64> {
        self.0.variables.iter().map(|id| id.into_inner()).collect()
    }

    /// Retained selected-variable diagnostics keyed by sample ID, using activation_atol.
    #[getter]
    pub fn active_variable(&self) -> BTreeMap<u64, Option<u64>> {
        self.0
            .stage
            .active_variable
            .iter()
            .map(|(id, active)| (id.into_inner(), active.map(|v| v.into_inner())))
            .collect()
    }
}

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl SampledSos1Constraint {
    /// Member decision variable IDs.
    #[getter]
    pub fn variables(&self) -> BTreeSet<u64> {
        self.0.variables.iter().map(|id| id.into_inner()).collect()
    }

    /// Retained selected-variable diagnostics keyed by sample ID, using activation_atol.
    #[getter]
    pub fn active_variable(&self) -> BTreeMap<u64, Option<u64>> {
        self.0
            .stage
            .active_variable
            .iter()
            .map(|(id, active)| (id.into_inner(), active.map(|v| v.into_inner())))
            .collect()
    }
}
