//! Owned Python snapshots of evaluated constraints, selected by family.
//!
//! Root objects retain identity and lifecycle ownership. These wrappers copy
//! validated rows and their context; they cannot mutate the enclosing result.

impl pyo3_stub_gen::PyStubType for EvaluatedConstraints {
    fn type_output() -> pyo3_stub_gen::TypeInfo {
        <BTreeMap<crate::ConstraintID, crate::EvaluatedConstraint> as pyo3_stub_gen::PyStubType>::type_output()
            | <BTreeMap<crate::IndicatorConstraintID, EvaluatedIndicatorConstraint> as pyo3_stub_gen::PyStubType>::type_output()
            | <BTreeMap<crate::OneHotConstraintID, EvaluatedOneHotConstraint> as pyo3_stub_gen::PyStubType>::type_output()
            | <BTreeMap<crate::Sos1ConstraintID, EvaluatedSos1Constraint> as pyo3_stub_gen::PyStubType>::type_output()
    }
}

impl pyo3_stub_gen::PyStubType for SampledConstraints {
    fn type_output() -> pyo3_stub_gen::TypeInfo {
        <BTreeMap<crate::ConstraintID, crate::SampledConstraint> as pyo3_stub_gen::PyStubType>::type_output()
            | <BTreeMap<crate::IndicatorConstraintID, SampledIndicatorConstraint> as pyo3_stub_gen::PyStubType>::type_output()
            | <BTreeMap<crate::OneHotConstraintID, SampledOneHotConstraint> as pyo3_stub_gen::PyStubType>::type_output()
            | <BTreeMap<crate::Sos1ConstraintID, SampledSos1Constraint> as pyo3_stub_gen::PyStubType>::type_output()
    }
}

use crate::constraint_kind::ConstraintKind;
use ommx::{EvaluatedConstraintBehavior, EvaluatedConstraintData, SampledConstraintData};
use pyo3::{prelude::*, types::PyDict};
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
        #[pyclass]
        pub struct $name($inner, ommx::ConstraintContext, Option<ommx::RemovedReason>);

        impl $name {
            fn from_parts(inner: $inner, context: ommx::ConstraintContext) -> Self {
                Self(inner, context, None)
            }
        }
        constraint_result_context!($name);
    };
}

result_class!(EvaluatedIndicatorConstraint, ommx::EvaluatedIndicatorConstraint, "An owned evaluated indicator result. Created by get_constraints(kind=...), with identity in the dictionary key.");
result_class!(SampledIndicatorConstraint, ommx::SampledIndicatorConstraint, "An owned sampled indicator result. Created by get_constraints(kind=...), with identity in the dictionary key.");
result_class!(EvaluatedOneHotConstraint, ommx::EvaluatedOneHotConstraint, "An owned evaluated one-hot result. Created by get_constraints(kind=...), with identity in the dictionary key.");
result_class!(SampledOneHotConstraint, ommx::SampledOneHotConstraint, "An owned sampled one-hot result. Created by get_constraints(kind=...), with identity in the dictionary key.");
result_class!(EvaluatedSos1Constraint, ommx::EvaluatedSos1Constraint, "An owned evaluated SOS1 result. Created by get_constraints(kind=...), with identity in the dictionary key.");
result_class!(SampledSos1Constraint, ommx::SampledSos1Constraint, "An owned sampled SOS1 result. Created by get_constraints(kind=...), with identity in the dictionary key.");

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

// Both access paths copy the row together with sidecars from its owning collection.
macro_rules! collect_results {
    ($collection:expr, $row:ty) => {{
        let collection = $collection;
        collection
            .iter()
            .map(|(id, row)| {
                (
                    (*id).into(),
                    <$row>::from_parts(row.clone(), collection.context().collect_for(*id))
                        .with_removed_reason(collection.removed_reasons().get(id).cloned()),
                )
            })
            .collect()
    }};
}

/// Typed conversion of one family's native result dictionary into Python.
/// Used across the binding boundary by Solution; this is not a Python class.
pub enum EvaluatedConstraints {
    Regular(BTreeMap<crate::ConstraintID, crate::EvaluatedConstraint>),
    Indicator(BTreeMap<crate::IndicatorConstraintID, EvaluatedIndicatorConstraint>),
    OneHot(BTreeMap<crate::OneHotConstraintID, EvaluatedOneHotConstraint>),
    Sos1(BTreeMap<crate::Sos1ConstraintID, EvaluatedSos1Constraint>),
}

impl EvaluatedConstraints {
    pub fn from_result(result: &ommx::Solution, kind: ConstraintKind) -> Self {
        match kind {
            ConstraintKind::Regular => Self::Regular(collect_results!(
                result.evaluated_constraints(),
                crate::EvaluatedConstraint
            )),
            ConstraintKind::Indicator => Self::Indicator(collect_results!(
                result.evaluated_indicator_constraints(),
                EvaluatedIndicatorConstraint
            )),
            ConstraintKind::OneHot => Self::OneHot(collect_results!(
                result.evaluated_one_hot_constraints(),
                EvaluatedOneHotConstraint
            )),
            ConstraintKind::Sos1 => Self::Sos1(collect_results!(
                result.evaluated_sos1_constraints(),
                EvaluatedSos1Constraint
            )),
        }
    }
}

impl<'py> IntoPyObject<'py> for EvaluatedConstraints {
    type Target = PyDict;
    type Output = Bound<'py, PyDict>;
    type Error = PyErr;

    fn into_pyobject(self, py: Python<'py>) -> PyResult<Self::Output> {
        match self {
            Self::Regular(rows) => rows.into_pyobject(py),
            Self::Indicator(rows) => rows.into_pyobject(py),
            Self::OneHot(rows) => rows.into_pyobject(py),
            Self::Sos1(rows) => rows.into_pyobject(py),
        }
    }
}

/// Typed conversion of one family's native result dictionary into Python.
/// Used across the binding boundary by SampleSet; this is not a Python class.
pub enum SampledConstraints {
    Regular(BTreeMap<crate::ConstraintID, crate::SampledConstraint>),
    Indicator(BTreeMap<crate::IndicatorConstraintID, SampledIndicatorConstraint>),
    OneHot(BTreeMap<crate::OneHotConstraintID, SampledOneHotConstraint>),
    Sos1(BTreeMap<crate::Sos1ConstraintID, SampledSos1Constraint>),
}

impl SampledConstraints {
    pub fn from_result(result: &ommx::SampleSet, kind: ConstraintKind) -> Self {
        match kind {
            ConstraintKind::Regular => Self::Regular(collect_results!(
                result.constraints(),
                crate::SampledConstraint
            )),
            ConstraintKind::Indicator => Self::Indicator(collect_results!(
                result.indicator_constraints(),
                SampledIndicatorConstraint
            )),
            ConstraintKind::OneHot => Self::OneHot(collect_results!(
                result.one_hot_constraints(),
                SampledOneHotConstraint
            )),
            ConstraintKind::Sos1 => Self::Sos1(collect_results!(
                result.sos1_constraints(),
                SampledSos1Constraint
            )),
        }
    }
}

impl<'py> IntoPyObject<'py> for SampledConstraints {
    type Target = PyDict;
    type Output = Bound<'py, PyDict>;
    type Error = PyErr;

    fn into_pyobject(self, py: Python<'py>) -> PyResult<Self::Output> {
        match self {
            Self::Regular(rows) => rows.into_pyobject(py),
            Self::Indicator(rows) => rows.into_pyobject(py),
            Self::OneHot(rows) => rows.into_pyobject(py),
            Self::Sos1(rows) => rows.into_pyobject(py),
        }
    }
}
