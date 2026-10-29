use crate::error::OmmxPyResult;
use crate::{Instance, InstanceClass, Sense, SpecialConstraintKind};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use std::collections::{BTreeMap, HashSet};

fn resolve_atol(value: Option<f64>) -> OmmxPyResult<ommx::ATol> {
    Ok(match value {
        Some(value) => ommx::ATol::new(value)?,
        None => ommx::ATol::default(),
    })
}

/// Selection for the special-constraint Preparation phase.
///
/// Construct a value with an owner-operation factory. Validation and mutation
/// semantics remain owned by that {class}`~ommx.Instance` operation.
#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass(eq, frozen)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecialConstraintPreparation {
    inner: ommx::SpecialConstraintPreparation,
}

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl SpecialConstraintPreparation {
    /// Select {meth}`~ommx.Instance.lower_special_constraints`.
    ///
    /// ``kinds`` is passed unchanged as the set of active special-constraint
    /// families to lower. See the owner operation and its per-family conversion
    /// methods for formulas, prerequisites, generated artifacts, stored removal
    /// reasons and provenance, and failure semantics.
    ///
    /// ``atol`` is used when Indicator Function bodies require zero-sensitive
    /// interval evaluation. If omitted, :attr:`DEFAULT_ATOL` is stored in the
    /// preparation step.
    #[staticmethod]
    #[pyo3(signature = (*, kinds, atol=None))]
    pub fn lower_special_constraints(
        kinds: HashSet<SpecialConstraintKind>,
        atol: Option<f64>,
    ) -> OmmxPyResult<Self> {
        Ok(Self {
            inner: ommx::SpecialConstraintPreparation::LowerSpecialConstraints {
                kinds: kinds.into_iter().map(Into::into).collect(),
                atol: resolve_atol(atol)?,
            },
        })
    }
}

impl From<SpecialConstraintPreparation> for ommx::SpecialConstraintPreparation {
    fn from(value: SpecialConstraintPreparation) -> Self {
        value.inner
    }
}

impl From<ommx::SpecialConstraintPreparation> for SpecialConstraintPreparation {
    fn from(inner: ommx::SpecialConstraintPreparation) -> Self {
        Self { inner }
    }
}

/// Selection for the active-objective Preparation phase.
///
/// The phase invokes {meth}`~ommx.Instance.convert_active_objective`. When the
/// target differs from the active sense, that owner operation changes only the
/// solver-facing objective. An existing {attr}`~ommx.Instance.output_objective`
/// is retained; otherwise the pre-conversion objective and sense are captured
/// there. Solution and sample evaluation retain the entry output semantics.
#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass(eq, frozen)]
#[derive(Debug, Clone, PartialEq, Eq)]
/// Convert the active objective to ``target`` during Preparation.
///
/// # Invariants
///
/// The immutable target records the solver-facing sense requested by Preparation.
///
/// >>> from ommx import ObjectivePreparation, Sense
/// >>> preparation = ObjectivePreparation(target=Sense.Minimize)
/// >>> assert preparation.target == Sense.Minimize
pub struct ObjectivePreparation {
    inner: ommx::ObjectivePreparation,
}

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl ObjectivePreparation {
    #[new]
    #[pyo3(signature = (*, target))]
    pub fn new(target: Sense) -> Self {
        Self {
            inner: ommx::ObjectivePreparation {
                target: target.into(),
            },
        }
    }

    #[getter]
    pub fn target(&self) -> Sense {
        self.inner.target.into()
    }
}

impl From<ObjectivePreparation> for ommx::ObjectivePreparation {
    fn from(value: ObjectivePreparation) -> Self {
        value.inner
    }
}

impl From<ommx::ObjectivePreparation> for ObjectivePreparation {
    fn from(inner: ommx::ObjectivePreparation) -> Self {
        Self { inner }
    }
}

/// Integer-slack Preparation for active regular inequalities.
///
/// Preparation first attempts exact conversion to equality using
/// ``max_integer_range`` and ``atol``. ``slack_upper_bound=None`` requires the
/// constraint to become an equality or be removed as trivially satisfied, so
/// {class}`~ommx.ExactIntegerSlackError` is propagated. An integer value permits the
/// constraint to remain an inequality: only that signal selects the
/// inequality-preserving owner operation with this upper bound. The latter is
/// not an approximation of the original feasible set. Every other owner error
/// is propagated unchanged.
#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass(eq, frozen)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IntegerSlackPreparation {
    inner: ommx::IntegerSlackPreparation,
}

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl IntegerSlackPreparation {
    /// Configure Integer-slack Preparation for active regular inequalities.
    ///
    /// ``max_integer_range`` and ``atol`` are passed to
    /// {meth}`~ommx.Instance.convert_inequality_to_equality_with_integer_slack`.
    /// ``slack_upper_bound`` has the fallback semantics documented by this
    /// class. Inequalities are processed in ascending constraint-ID order; a
    /// failure on a later constraint does not roll back conversions already
    /// completed by this phase.
    #[new]
    #[pyo3(signature = (*, max_integer_range, atol=None, slack_upper_bound=None))]
    pub fn new(
        max_integer_range: u64,
        atol: Option<f64>,
        slack_upper_bound: Option<u64>,
    ) -> OmmxPyResult<Self> {
        Ok(Self {
            inner: ommx::IntegerSlackPreparation {
                max_integer_range,
                atol: resolve_atol(atol)?,
                slack_upper_bound,
            },
        })
    }

    /// Maximum finite range accepted for exact Integer slack.
    #[getter]
    pub fn max_integer_range(&self) -> u64 {
        self.inner.max_integer_range
    }

    /// Absolute tolerance used to normalize bounds to integers.
    #[getter]
    pub fn atol(&self) -> f64 {
        self.inner.atol.into_inner()
    }

    /// Optional upper bound for inequality-preserving Integer slack.
    ///
    /// ``None`` requires equality. An integer permits the inequality-preserving
    /// owner operation only after {class}`~ommx.ExactIntegerSlackError`.
    #[getter]
    pub fn slack_upper_bound(&self) -> Option<u64> {
        self.inner.slack_upper_bound
    }
}

impl From<IntegerSlackPreparation> for ommx::IntegerSlackPreparation {
    fn from(value: IntegerSlackPreparation) -> Self {
        value.inner
    }
}

impl From<ommx::IntegerSlackPreparation> for IntegerSlackPreparation {
    fn from(inner: ommx::IntegerSlackPreparation) -> Self {
        Self { inner }
    }
}

/// Selection for the used-Integer encoding Preparation phase.
///
/// Exactly one encoding owner operation is selected. Validation and mutation
/// semantics remain owned by that operation.
#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass(eq, frozen)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntegerEncodingPreparation {
    inner: ommx::IntegerEncodingPreparation,
}

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl IntegerEncodingPreparation {
    /// Select log encoding of every used Integer decision variable.
    ///
    /// The complete target set is determined from
    /// {attr}`~ommx.Instance.used_decision_variables` before mutation and encoded
    /// together with the same mathematical rule as {meth}`~ommx.Instance.log_encode`.
    /// ``atol`` normalizes bounds that are within tolerance of integers. On
    /// success, no used Integer decision variable remains; irrelevant Integer
    /// variables are not encoded.
    ///
    /// A used Integer with a non-finite or otherwise non-encodable bound raises
    /// {class}`~ommx.LogEncodingError`. Active SOS1 members cannot be substituted
    /// and must be lowered first. Any error leaves this phase's input unchanged.
    #[staticmethod]
    #[pyo3(signature = (*, atol=None))]
    pub fn log_encode_all_used_integers(atol: Option<f64>) -> OmmxPyResult<Self> {
        Ok(Self {
            inner: ommx::IntegerEncodingPreparation::LogEncodeAllUsedIntegers {
                atol: resolve_atol(atol)?,
            },
        })
    }
}

impl From<IntegerEncodingPreparation> for ommx::IntegerEncodingPreparation {
    fn from(value: IntegerEncodingPreparation) -> Self {
        value.inner
    }
}

impl From<ommx::IntegerEncodingPreparation> for IntegerEncodingPreparation {
    fn from(inner: ommx::IntegerEncodingPreparation) -> Self {
        Self { inner }
    }
}

/// Reduce powers of active Binary variables during Preparation.
///
/// This phase invokes {meth}`~ommx.Instance.reduce_binary_power`, using
/// $x^n=x$ for Binary variables and positive integer $n$. If it rewrites the
/// active objective and no {attr}`~ommx.Instance.output_objective` exists, the unreduced objective and
/// sense are captured there. An existing output objective is retained, so
/// solution and sample evaluation retain the entry output semantics.
/// A coefficient error leaves the instance unchanged.
#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass(eq, frozen)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BinaryPowerPreparation {
    inner: ommx::BinaryPowerPreparation,
}

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl BinaryPowerPreparation {
    #[new]
    pub fn new() -> Self {
        Self::default()
    }
}

impl From<BinaryPowerPreparation> for ommx::BinaryPowerPreparation {
    fn from(value: BinaryPowerPreparation) -> Self {
        value.inner
    }
}

impl From<ommx::BinaryPowerPreparation> for BinaryPowerPreparation {
    fn from(inner: ommx::BinaryPowerPreparation) -> Self {
        Self { inner }
    }
}

/// Selection for the fixed-weight penalty Preparation phase.
///
/// Let $F(x)$ be the active objective and $g_i(x)$ the body of each active
/// regular constraint. With normalized nonnegative penalty magnitudes $w_i$,
/// the keyed operation replaces the active objective by
///
/// $$
/// F(x) + \sum_i w_i g_i(x)^2
/// $$
///
/// for minimization, and by $F(x) - \sum_i w_i g_i(x)^2$ for maximization. The
/// uniform operation uses one $w$ for every active regular constraint. For an
/// inequality $g_i(x) \leq 0$, this squares $g_i(x)$ itself; it is not the
/// one-sided penalty $\max(0, g_i(x))^2$.
///
/// On success, every active regular constraint moves to
/// {attr}`~ommx.Instance.removed_constraints`, while existing removed
/// constraints are preserved. When at least one active regular constraint is
/// converted, the entry output semantics are preserved in
/// {attr}`~ommx.Instance.output_objective` for solution and sample evaluation
/// (retaining an existing output objective, or capturing the active pair),
/// but active-formulation optimality is no longer transported. Active
/// Indicator, OneHot, or SOS1 constraints are not penalty-converted and cause
/// an error before mutation. Validation, objective construction, and regular
/// constraint lifecycle changes are atomic: any error leaves the instance
/// unchanged. When a penalty is applied to active constraints, OMMX validates
/// the weight domain but cannot decide whether a weight is large enough for an
/// application's penalty rule; that choice belongs to the caller.
#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass(eq, frozen)]
#[derive(Debug, Clone, PartialEq)]
pub struct FixedPenaltyPreparation {
    inner: ommx::FixedPenaltyPreparation,
}

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl FixedPenaltyPreparation {
    /// Select the keyed fixed-weight penalty owner operation.
    ///
    /// The keys of ``weights`` must be exactly the active regular constraint
    /// IDs. Each value is the corresponding $w_i$ in the class formula.
    /// ``atol`` is used by the owner operation to accept a penalty magnitude
    /// down to ``-atol`` and normalize a tolerated negative value to zero. Every
    /// weight must be finite.
    /// Removed constraints record
    /// ``reason="ommx.Instance.penalty_method_with_fixed_weights"`` with no
    /// reason parameters.
    #[staticmethod]
    #[pyo3(signature = (*, weights, atol=None))]
    pub fn penalty_method_with_fixed_weights(
        weights: BTreeMap<u64, f64>,
        atol: Option<f64>,
    ) -> OmmxPyResult<Self> {
        Ok(Self {
            inner: ommx::FixedPenaltyPreparation::PenaltyMethodWithFixedWeights {
                weights: weights
                    .into_iter()
                    .map(|(id, weight)| (ommx::ConstraintID::from(id), weight))
                    .collect(),
                atol: resolve_atol(atol)?,
            },
        })
    }

    #[doc = include_str!("docstrings/ommx/FixedPenaltyPreparation.uniform_penalty_method_with_fixed_weight.md")]
    #[staticmethod]
    #[pyo3(signature = (*, weight, atol=None))]
    pub fn uniform_penalty_method_with_fixed_weight(
        weight: f64,
        atol: Option<f64>,
    ) -> OmmxPyResult<Self> {
        Ok(Self {
            inner: ommx::FixedPenaltyPreparation::UniformPenaltyMethodWithFixedWeight {
                weight,
                atol: resolve_atol(atol)?,
            },
        })
    }
}

impl From<FixedPenaltyPreparation> for ommx::FixedPenaltyPreparation {
    fn from(value: FixedPenaltyPreparation) -> Self {
        value.inner
    }
}

impl From<ommx::FixedPenaltyPreparation> for FixedPenaltyPreparation {
    fn from(inner: ommx::FixedPenaltyPreparation) -> Self {
        Self { inner }
    }
}

#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass(eq)]
#[derive(Debug, Clone, PartialEq, Default)]
#[doc = include_str!("docstrings/ommx/PreparationPolicy.md")]
pub struct PreparationPolicy {
    inner: ommx::PreparationPolicy,
}

fn configure_qubo_hubo_policy(
    mut inner: ommx::PreparationPolicy,
    uniform_penalty_weight: Option<f64>,
    penalty_weights: Option<BTreeMap<u64, f64>>,
    inequality_integer_slack_max_range: u64,
) -> OmmxPyResult<PreparationPolicy> {
    if uniform_penalty_weight.is_some() && penalty_weights.is_some() {
        return Err(PyValueError::new_err(
            "Both uniform_penalty_weight and penalty_weights are specified. Please choose one.",
        )
        .into());
    }

    inner.integer_slack = Some(ommx::IntegerSlackPreparation {
        max_integer_range: inequality_integer_slack_max_range,
        atol: ommx::ATol::default(),
        slack_upper_bound: Some(inequality_integer_slack_max_range),
    });
    if let Some(weights) = penalty_weights {
        inner.fixed_penalty = Some(
            ommx::FixedPenaltyPreparation::PenaltyMethodWithFixedWeights {
                weights: weights
                    .into_iter()
                    .map(|(id, weight)| (ommx::ConstraintID::from(id), weight))
                    .collect(),
                atol: ommx::ATol::default(),
            },
        );
    } else if let Some(weight) = uniform_penalty_weight {
        inner.fixed_penalty = Some(
            ommx::FixedPenaltyPreparation::UniformPenaltyMethodWithFixedWeight {
                weight,
                atol: ommx::ATol::default(),
            },
        );
    }

    Ok(PreparationPolicy { inner })
}

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl PreparationPolicy {
    #[new]
    #[pyo3(signature = (*, special_constraints=None, objective=None, integer_slack=None, integer_encoding=None, fixed_penalty=None, binary_power_reduction=None))]
    pub fn new(
        special_constraints: Option<SpecialConstraintPreparation>,
        objective: Option<ObjectivePreparation>,
        integer_slack: Option<IntegerSlackPreparation>,
        integer_encoding: Option<IntegerEncodingPreparation>,
        fixed_penalty: Option<FixedPenaltyPreparation>,
        binary_power_reduction: Option<BinaryPowerPreparation>,
    ) -> Self {
        let mut inner = ommx::PreparationPolicy::default();
        inner.special_constraints = special_constraints.map(Into::into);
        inner.objective = objective.map(Into::into);
        inner.integer_slack = integer_slack.map(Into::into);
        inner.integer_encoding = integer_encoding.map(Into::into);
        inner.fixed_penalty = fixed_penalty.map(Into::into);
        inner.binary_power_reduction = binary_power_reduction.map(Into::into);
        Self { inner }
    }

    /// Return a fresh policy for preparing an instance for QUBO formatting.
    ///
    /// ``uniform_penalty_weight`` and ``penalty_weights`` override the default
    /// uniform penalty weight of 1.0 and are mutually exclusive. The keyed form
    /// must cover exactly the active regular constraints at the penalty phase.
    /// ``inequality_integer_slack_max_range`` defaults to 31 and configures both
    /// the exact Integer-slack range and the fallback slack upper bound. This
    /// QUBO policy also reduces powers of Binary variables before checking the
    /// quadratic target.
    ///
    /// # Postconditions
    ///
    /// Each call returns a fresh complete QUBO policy whose optional weights and slack range are applied exactly.
    ///
    /// >>> from ommx import (
    /// ...     BinaryPowerPreparation, FixedPenaltyPreparation,
    /// ...     IntegerEncodingPreparation, IntegerSlackPreparation,
    /// ...     ObjectivePreparation, PreparationPolicy, Sense,
    /// ...     SpecialConstraintKind, SpecialConstraintPreparation,
    /// ... )
    /// >>> expected_special = SpecialConstraintPreparation.lower_special_constraints(
    /// ...     kinds={
    /// ...         SpecialConstraintKind.Indicator,
    /// ...         SpecialConstraintKind.OneHot,
    /// ...         SpecialConstraintKind.Sos1,
    /// ...     }
    /// ... )
    /// >>> expected_penalty = FixedPenaltyPreparation.uniform_penalty_method_with_fixed_weight(weight=1.0)
    /// >>> first = PreparationPolicy.for_qubo(inequality_integer_slack_max_range=17)
    /// >>> second = PreparationPolicy.for_qubo(inequality_integer_slack_max_range=17)
    /// >>> assert first is not second
    /// >>> assert first.special_constraints == expected_special
    /// >>> assert first.objective == ObjectivePreparation(target=Sense.Minimize)
    /// >>> assert first.integer_slack == IntegerSlackPreparation(max_integer_range=17, slack_upper_bound=17)
    /// >>> assert first.integer_encoding == IntegerEncodingPreparation.log_encode_all_used_integers()
    /// >>> assert first.fixed_penalty == expected_penalty
    /// >>> assert first.binary_power_reduction == BinaryPowerPreparation()
    /// >>> first.fixed_penalty = None
    /// >>> first.binary_power_reduction = None
    /// >>> assert second.fixed_penalty == expected_penalty
    /// >>> assert second.binary_power_reduction == BinaryPowerPreparation()
    /// >>> keyed = PreparationPolicy.for_qubo(penalty_weights={3: 2.0})
    /// >>> assert keyed.fixed_penalty == FixedPenaltyPreparation.penalty_method_with_fixed_weights(weights={3: 2.0})
    ///
    /// # Errors
    ///
    /// Supplying uniform and keyed penalty weights together raises ``ValueError``.
    ///
    /// >>> try:
    /// ...     PreparationPolicy.for_qubo(uniform_penalty_weight=1.0, penalty_weights={3: 2.0})
    /// ... except ValueError as error:
    /// ...     assert "Both uniform_penalty_weight" in str(error)
    /// ... else:
    /// ...     raise AssertionError("mutually exclusive penalty options were accepted")
    #[staticmethod]
    #[pyo3(signature = (*, uniform_penalty_weight=None, penalty_weights=None, inequality_integer_slack_max_range=31))]
    pub fn for_qubo(
        uniform_penalty_weight: Option<f64>,
        penalty_weights: Option<BTreeMap<u64, f64>>,
        inequality_integer_slack_max_range: u64,
    ) -> OmmxPyResult<Self> {
        configure_qubo_hubo_policy(
            ommx::PreparationPolicy::for_qubo(),
            uniform_penalty_weight,
            penalty_weights,
            inequality_integer_slack_max_range,
        )
    }

    /// Return a fresh policy for preparing an instance for HUBO formatting.
    ///
    /// ``uniform_penalty_weight`` and ``penalty_weights`` override the default
    /// uniform penalty weight of 1.0 and are mutually exclusive. The keyed form
    /// must cover exactly the active regular constraints at the penalty phase.
    /// ``inequality_integer_slack_max_range`` defaults to 31 and configures both
    /// the exact Integer-slack range and the fallback slack upper bound. Unlike
    /// {meth}`for_qubo`, this policy leaves Binary-power reduction disabled
    /// because HUBO accepts arbitrary polynomial degree.
    ///
    /// # Postconditions
    ///
    /// Each call returns a fresh complete HUBO policy with no Binary-power reduction and exact overrides.
    ///
    /// >>> from ommx import (
    /// ...     FixedPenaltyPreparation, IntegerEncodingPreparation,
    /// ...     IntegerSlackPreparation, ObjectivePreparation,
    /// ...     PreparationPolicy, Sense, SpecialConstraintKind,
    /// ...     SpecialConstraintPreparation,
    /// ... )
    /// >>> expected_special = SpecialConstraintPreparation.lower_special_constraints(
    /// ...     kinds={
    /// ...         SpecialConstraintKind.Indicator,
    /// ...         SpecialConstraintKind.OneHot,
    /// ...         SpecialConstraintKind.Sos1,
    /// ...     }
    /// ... )
    /// >>> first = PreparationPolicy.for_hubo(inequality_integer_slack_max_range=17)
    /// >>> second = PreparationPolicy.for_hubo(inequality_integer_slack_max_range=17)
    /// >>> assert first is not second
    /// >>> assert first.special_constraints == expected_special
    /// >>> assert first.objective == ObjectivePreparation(target=Sense.Minimize)
    /// >>> assert first.integer_slack == IntegerSlackPreparation(max_integer_range=17, slack_upper_bound=17)
    /// >>> assert first.integer_encoding == IntegerEncodingPreparation.log_encode_all_used_integers()
    /// >>> assert first.fixed_penalty == FixedPenaltyPreparation.uniform_penalty_method_with_fixed_weight(weight=1.0)
    /// >>> assert first.binary_power_reduction is None
    /// >>> first.fixed_penalty = None
    /// >>> assert second.fixed_penalty == FixedPenaltyPreparation.uniform_penalty_method_with_fixed_weight(weight=1.0)
    /// >>> uniform = PreparationPolicy.for_hubo(uniform_penalty_weight=4.0)
    /// >>> assert uniform.fixed_penalty == FixedPenaltyPreparation.uniform_penalty_method_with_fixed_weight(weight=4.0)
    ///
    /// # Errors
    ///
    /// Supplying uniform and keyed penalty weights together raises ``ValueError``.
    ///
    /// >>> try:
    /// ...     PreparationPolicy.for_hubo(uniform_penalty_weight=1.0, penalty_weights={3: 2.0})
    /// ... except ValueError as error:
    /// ...     assert "Both uniform_penalty_weight" in str(error)
    /// ... else:
    /// ...     raise AssertionError("mutually exclusive penalty options were accepted")
    #[staticmethod]
    #[pyo3(signature = (*, uniform_penalty_weight=None, penalty_weights=None, inequality_integer_slack_max_range=31))]
    pub fn for_hubo(
        uniform_penalty_weight: Option<f64>,
        penalty_weights: Option<BTreeMap<u64, f64>>,
        inequality_integer_slack_max_range: u64,
    ) -> OmmxPyResult<Self> {
        configure_qubo_hubo_policy(
            ommx::PreparationPolicy::for_hubo(),
            uniform_penalty_weight,
            penalty_weights,
            inequality_integer_slack_max_range,
        )
    }

    #[getter]
    pub fn special_constraints(&self) -> Option<SpecialConstraintPreparation> {
        self.inner.special_constraints.clone().map(Into::into)
    }

    #[setter]
    pub fn set_special_constraints(&mut self, value: Option<SpecialConstraintPreparation>) {
        self.inner.special_constraints = value.map(Into::into);
    }

    #[getter]
    pub fn objective(&self) -> Option<ObjectivePreparation> {
        self.inner.objective.map(Into::into)
    }

    #[setter]
    pub fn set_objective(&mut self, value: Option<ObjectivePreparation>) {
        self.inner.objective = value.map(Into::into);
    }

    #[getter]
    pub fn integer_slack(&self) -> Option<IntegerSlackPreparation> {
        self.inner.integer_slack.map(Into::into)
    }

    #[setter]
    pub fn set_integer_slack(&mut self, value: Option<IntegerSlackPreparation>) {
        self.inner.integer_slack = value.map(Into::into);
    }

    #[getter]
    pub fn integer_encoding(&self) -> Option<IntegerEncodingPreparation> {
        self.inner.integer_encoding.map(Into::into)
    }

    #[setter]
    pub fn set_integer_encoding(&mut self, value: Option<IntegerEncodingPreparation>) {
        self.inner.integer_encoding = value.map(Into::into);
    }

    #[getter]
    pub fn fixed_penalty(&self) -> Option<FixedPenaltyPreparation> {
        self.inner.fixed_penalty.clone().map(Into::into)
    }

    #[setter]
    pub fn set_fixed_penalty(&mut self, value: Option<FixedPenaltyPreparation>) {
        self.inner.fixed_penalty = value.map(Into::into);
    }

    #[getter]
    pub fn binary_power_reduction(&self) -> Option<BinaryPowerPreparation> {
        self.inner.binary_power_reduction.map(Into::into)
    }

    #[setter]
    pub fn set_binary_power_reduction(&mut self, value: Option<BinaryPowerPreparation>) {
        self.inner.binary_power_reduction = value.map(Into::into);
    }
}

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl Instance {
    #[doc = include_str!("docstrings/ommx/Instance.prepare.md")]
    pub fn prepare(
        &mut self,
        py: Python<'_>,
        input_class: &InstanceClass,
        policy: &PreparationPolicy,
    ) -> OmmxPyResult<()> {
        let _guard = crate::TRACING.attach_parent_context(py);
        self.inner.prepare(&input_class.0, &policy.inner)?;
        Ok(())
    }
}
