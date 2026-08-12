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
/// semantics remain owned by that :class:`Instance` operation.
#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass(eq, frozen)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecialConstraintPreparation {
    inner: ommx::SpecialConstraintPreparation,
}

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl SpecialConstraintPreparation {
    /// Select :meth:`Instance.lower_special_constraints`.
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
/// semantics remain owned by that :class:`Instance` operation.
#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass(eq, frozen)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SensePreparation {
    inner: ommx::SensePreparation,
}

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl SensePreparation {
    /// Select :meth:`Instance.as_minimization_problem`.
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
/// :class:`ExactIntegerSlackError` is propagated. An integer value permits the
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
    /// owner operation only after :class:`ExactIntegerSlackError`.
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
    /// Select the underlying Rust ``log_encode_all_used_integers`` owner
    /// operation. On success, no used Integer decision variables remain.
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
/// Exactly one fixed-weight owner operation is selected. Weight and
/// constraint-ID validation remains owned by that operation.
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
    /// ``atol`` is used by the owner operation to accept a decision-rule weight
    /// down to ``-atol`` and normalize a tolerated negative value to zero.
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
    /// ``atol`` is used by the owner operation to accept a decision-rule weight
    /// down to ``-atol`` and normalize a tolerated negative value to zero.
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

/// Optional phases interpreted by :meth:`Instance.prepare`.
///
/// Each property independently selects at most one well-formed phase. Fields
/// may be combined freely, although owner validation and target membership can
/// still make a combination fail for a particular :class:`Instance`.
///
/// ``Instance.prepare`` applies selected phases at most once in the canonical
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

    #[getter]
    pub fn special_constraints(&self) -> Option<SpecialConstraintPreparation> {
        self.inner.special_constraints.clone().map(Into::into)
    }

    #[setter]
    pub fn set_special_constraints(&mut self, value: Option<SpecialConstraintPreparation>) {
        self.inner.special_constraints = value.map(Into::into);
    }

    #[getter]
    pub fn sense(&self) -> Option<SensePreparation> {
        self.inner.sense.map(Into::into)
    }

    #[setter]
    pub fn set_sense(&mut self, value: Option<SensePreparation>) {
        self.inner.sense = value.map(Into::into);
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
}

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl Instance {
    /// Prepare this instance in place for membership in ``input_class``.
    ///
    /// ``policy`` selects existing :class:`Instance` owner operations. The
    /// method stops once ``input_class`` contains this instance and returns
    /// ``None``. Success guarantees only that membership; Adapter-specific
    /// applicability remains a separate check.
    ///
    /// Preparation is not globally transactional. Changes committed by an
    /// earlier owner operation remain if a later operation raises an error.
    /// Existing Rust owner signals retain their Python exception mappings.
    /// When configured phases are exhausted without reaching ``input_class``,
    /// :class:`PreparationTargetNotReachedError` exposes the final membership
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
