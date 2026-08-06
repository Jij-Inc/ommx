use crate::{error::OmmxPyResult, instance::PenaltyMethodWithWeightsInput, SpecialConstraintKind};
use pyo3::prelude::*;
use std::collections::{BTreeMap, HashSet};

/// Arguments for existing :class:`Instance` operations used by
/// :meth:`Instance.prepare`.
///
/// Each optional field stores the arguments for one existing :class:`Instance`
/// operation. ``None`` means preparation does not call that operation.
/// ``as_minimization_problem`` selects its no-argument operation with a Boolean.
/// Pass the target :class:`InstanceClass` separately to :meth:`Instance.prepare`.
/// Solver Adapters may recommend a Policy; the caller independently supplies the
/// target, often the Adapter's ``INPUT_CLASS``.
#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass(frozen)]
#[derive(Clone)]
pub struct PreparationPolicy(ommx::PreparationPolicy);

impl PreparationPolicy {
    /// Read-only bridge used by the Instance binding that executes this Policy.
    pub(crate) fn as_core(&self) -> &ommx::PreparationPolicy {
        &self.0
    }
}

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl PreparationPolicy {
    #[new]
    #[pyo3(signature = (*, lower_special_constraints=None, log_encode_used_integers=None, as_minimization_problem=false, convert_inequality_to_equality_with_integer_slack=None, add_integer_slack_to_inequality=None, uniform_penalty_method_with_weight=None, penalty_method_with_weights=None))]
    #[allow(clippy::too_many_arguments)]
    fn new(
        lower_special_constraints: Option<HashSet<SpecialConstraintKind>>,
        log_encode_used_integers: Option<f64>,
        as_minimization_problem: bool,
        convert_inequality_to_equality_with_integer_slack: Option<(u64, f64)>,
        add_integer_slack_to_inequality: Option<u64>,
        uniform_penalty_method_with_weight: Option<f64>,
        penalty_method_with_weights: Option<PenaltyMethodWithWeightsInput>,
    ) -> OmmxPyResult<Self> {
        let lower_special_constraints = lower_special_constraints.map(|kinds| {
            kinds
                .into_iter()
                .map(Into::into)
                .collect::<ommx::SpecialConstraintKinds>()
        });
        let log_encode_used_integers = log_encode_used_integers.map(ommx::ATol::new).transpose()?;
        let convert_inequality_to_equality_with_integer_slack =
            match convert_inequality_to_equality_with_integer_slack {
                Some((max_integer_range, atol)) => {
                    Some((max_integer_range, ommx::ATol::new(atol)?))
                }
                None => None,
            };
        let penalty_method_with_weights =
            penalty_method_with_weights.map(PenaltyMethodWithWeightsInput::into_core);

        Ok(Self(ommx::PreparationPolicy::new(
            lower_special_constraints,
            log_encode_used_integers,
            as_minimization_problem,
            convert_inequality_to_equality_with_integer_slack,
            add_integer_slack_to_inequality,
            uniform_penalty_method_with_weight,
            penalty_method_with_weights,
        )))
    }

    #[getter]
    /// Arguments for :meth:`Instance.lower_special_constraints`.
    pub fn lower_special_constraints(&self) -> Option<HashSet<SpecialConstraintKind>> {
        self.0
            .lower_special_constraints()
            .map(|kinds| kinds.iter().copied().map(Into::into).collect())
    }

    #[getter]
    /// Argument for the bounded-Integer log-encoding owner operation.
    pub fn log_encode_used_integers(&self) -> Option<f64> {
        self.0
            .log_encode_used_integers()
            .map(|atol| atol.into_inner())
    }

    #[getter]
    /// Whether :meth:`Instance.as_minimization_problem` is invoked.
    pub fn as_minimization_problem(&self) -> bool {
        self.0.as_minimization_problem()
    }

    #[getter]
    /// Arguments after ``constraint_id`` for the exact Integer-slack owner API.
    pub fn convert_inequality_to_equality_with_integer_slack(&self) -> Option<(u64, f64)> {
        self.0
            .convert_inequality_to_equality_with_integer_slack()
            .map(|(max_integer_range, atol)| (max_integer_range, atol.into_inner()))
    }

    #[getter]
    /// Argument after ``constraint_id`` for the approximate Integer-slack API.
    pub fn add_integer_slack_to_inequality(&self) -> Option<u64> {
        self.0.add_integer_slack_to_inequality()
    }

    #[getter]
    /// Argument for the uniform fixed-weight penalty owner API.
    pub fn uniform_penalty_method_with_weight(&self) -> Option<f64> {
        self.0.uniform_penalty_method_with_weight()
    }

    #[getter]
    /// Arguments for the per-constraint fixed-weight penalty owner API.
    pub fn penalty_method_with_weights(&self) -> Option<BTreeMap<u64, f64>> {
        self.0.penalty_method_with_weights().map(|weights| {
            weights
                .iter()
                .map(|(id, weight)| (id.into_inner(), *weight))
                .collect()
        })
    }

    pub fn __repr__(&self) -> String {
        format!("PreparationPolicy({:?})", self.0)
    }
}
