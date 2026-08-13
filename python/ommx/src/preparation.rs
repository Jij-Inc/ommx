use crate::error::OmmxPyResult;
use crate::{Instance, InstanceClass, SpecialConstraintKind};
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
    #[staticmethod]
    #[pyo3(signature = (*, kinds))]
    pub fn lower_special_constraints(kinds: HashSet<SpecialConstraintKind>) -> Self {
        Self {
            inner: ommx::SpecialConstraintPreparation::LowerSpecialConstraints {
                kinds: kinds.into_iter().map(Into::into).collect(),
            },
        }
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

/// Selection for the optimization-sense Preparation phase.
///
/// Construct a value with an owner-operation factory. Validation and mutation
/// semantics remain owned by that {class}`~ommx.Instance` operation.
#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass(eq, frozen)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SensePreparation {
    inner: ommx::SensePreparation,
}

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl SensePreparation {
    /// Select {meth}`~ommx.Instance.as_minimization_problem`.
    #[staticmethod]
    pub fn as_minimization_problem() -> Self {
        Self {
            inner: ommx::SensePreparation::AsMinimizationProblem,
        }
    }
}

impl From<SensePreparation> for ommx::SensePreparation {
    fn from(value: SensePreparation) -> Self {
        value.inner
    }
}

impl From<ommx::SensePreparation> for SensePreparation {
    fn from(inner: ommx::SensePreparation) -> Self {
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

/// Selection for the fixed-weight penalty Preparation phase.
///
/// Let $F(x)$ be the objective and $g_i(x)$ the body of each active regular
/// constraint. With normalized nonnegative penalty magnitudes $w_i$, the keyed
/// operation replaces the objective by
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
/// constraints are preserved. Active Indicator, OneHot, or SOS1 constraints
/// are not penalty-converted and cause an error before mutation. Validation and
/// objective construction are atomic: any error leaves the instance unchanged.
/// When a penalty is applied to active constraints, OMMX validates the weight
/// domain but cannot decide whether a weight is large enough for an application's
/// penalty rule; that choice belongs to the caller.
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

    /// Select the uniform fixed-weight penalty owner operation.
    ///
    /// ``weight`` is the common $w$ in the class formula for every active
    /// regular constraint. In particular, this replaces $F(x)$ by
    /// $F(x) + w \sum_i g_i(x)^2$ for minimization and by
    /// $F(x) - w \sum_i g_i(x)^2$ for maximization, after normalizing $w$.
    /// ``atol`` is used by the owner operation to accept a penalty magnitude
    /// down to ``-atol`` and normalize a tolerated negative value to zero. When
    /// at least one active regular constraint is converted, the weight must be
    /// finite. With no active regular constraints the owner operation is an
    /// identity and does not inspect the weight.
    /// Removed constraints record
    /// ``reason="ommx.Instance.uniform_penalty_method_with_fixed_weight"`` with
    /// no reason parameters.
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

/// Optional phases interpreted by {meth}`~ommx.Instance.prepare`.
///
/// Each property independently selects at most one well-formed phase. Fields
/// may be combined freely, although owner validation and target membership can
/// still make a combination fail for a particular {class}`~ommx.Instance`.
///
/// {meth}`~ommx.Instance.prepare` applies selected phases at most once in the canonical
/// Rust-owned order: special constraints, optimization sense, Integer slack,
/// Integer encoding, then fixed penalty. All phases are disabled by default.
/// Future phases will also default to disabled.
///
/// Construct the table with keyword arguments or assign its public properties:
///
/// ```python
/// from ommx import PreparationPolicy, SensePreparation
///
/// policy = PreparationPolicy()
/// policy.sense = SensePreparation.as_minimization_problem()
/// ```
#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass(eq)]
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PreparationPolicy {
    inner: ommx::PreparationPolicy,
}

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl PreparationPolicy {
    #[new]
    #[pyo3(signature = (*, special_constraints=None, sense=None, integer_slack=None, integer_encoding=None, fixed_penalty=None))]
    pub fn new(
        special_constraints: Option<SpecialConstraintPreparation>,
        sense: Option<SensePreparation>,
        integer_slack: Option<IntegerSlackPreparation>,
        integer_encoding: Option<IntegerEncodingPreparation>,
        fixed_penalty: Option<FixedPenaltyPreparation>,
    ) -> Self {
        let mut inner = ommx::PreparationPolicy::default();
        inner.special_constraints = special_constraints.map(Into::into);
        inner.sense = sense.map(Into::into);
        inner.integer_slack = integer_slack.map(Into::into);
        inner.integer_encoding = integer_encoding.map(Into::into);
        inner.fixed_penalty = fixed_penalty.map(Into::into);
        Self { inner }
    }

    /// Optional special-constraint phase. ``None`` disables this phase.
    #[getter]
    pub fn special_constraints(&self) -> Option<SpecialConstraintPreparation> {
        self.inner.special_constraints.clone().map(Into::into)
    }

    #[setter]
    pub fn set_special_constraints(&mut self, value: Option<SpecialConstraintPreparation>) {
        self.inner.special_constraints = value.map(Into::into);
    }

    /// Optional optimization-sense phase. ``None`` disables this phase.
    #[getter]
    pub fn sense(&self) -> Option<SensePreparation> {
        self.inner.sense.map(Into::into)
    }

    #[setter]
    pub fn set_sense(&mut self, value: Option<SensePreparation>) {
        self.inner.sense = value.map(Into::into);
    }

    /// Optional Integer-slack phase. ``None`` disables this phase.
    #[getter]
    pub fn integer_slack(&self) -> Option<IntegerSlackPreparation> {
        self.inner.integer_slack.map(Into::into)
    }

    #[setter]
    pub fn set_integer_slack(&mut self, value: Option<IntegerSlackPreparation>) {
        self.inner.integer_slack = value.map(Into::into);
    }

    /// Optional used-Integer encoding phase. ``None`` disables this phase.
    #[getter]
    pub fn integer_encoding(&self) -> Option<IntegerEncodingPreparation> {
        self.inner.integer_encoding.map(Into::into)
    }

    #[setter]
    pub fn set_integer_encoding(&mut self, value: Option<IntegerEncodingPreparation>) {
        self.inner.integer_encoding = value.map(Into::into);
    }

    /// Optional fixed-weight penalty phase. ``None`` disables this phase.
    #[getter]
    pub fn fixed_penalty(&self) -> Option<FixedPenaltyPreparation> {
        self.inner.fixed_penalty.clone().map(Into::into)
    }

    #[setter]
    pub fn set_fixed_penalty(&mut self, value: Option<FixedPenaltyPreparation>) {
        self.inner.fixed_penalty = value.map(Into::into);
    }
}

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl Instance {
    /// Prepare this instance in place for membership in ``input_class``.
    ///
    /// ``policy`` selects existing {class}`~ommx.Instance` owner operations. The
    /// method checks whole-class membership before the first phase and after
    /// each selected phase. Each selected phase is applied at most once in the
    /// canonical order documented by {class}`~ommx.PreparationPolicy`, and later
    /// phases are skipped as soon as ``input_class`` contains this instance.
    /// The method then returns ``None``. Success guarantees only that membership;
    /// Adapter-specific applicability remains a separate check.
    ///
    /// Preparation is not globally transactional. Changes committed by an
    /// earlier owner operation remain if a later operation raises an error.
    /// Within the Integer-slack phase, active regular inequalities are processed
    /// in ascending constraint-ID order, and a later failure leaves earlier
    /// conversions committed. Other phases retain the failure semantics of their
    /// owner operations.
    ///
    /// Existing Rust owner signals retain their Python exception mappings.
    /// When configured phases are exhausted without reaching ``input_class``,
    /// {class}`~ommx.PreparationTargetNotReachedError` exposes the final membership
    /// report through its ``report`` attribute.
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
