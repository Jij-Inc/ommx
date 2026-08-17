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

#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass(eq, frozen)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecialConstraintPreparation {
    inner: ommx::SpecialConstraintPreparation,
}

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl SpecialConstraintPreparation {
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

#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass(eq, frozen)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SensePreparation {
    inner: ommx::SensePreparation,
}

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl SensePreparation {
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

    #[getter]
    pub fn max_integer_range(&self) -> u64 {
        self.inner.max_integer_range
    }

    #[getter]
    pub fn atol(&self) -> f64 {
        self.inner.atol.into_inner()
    }

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

#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass(eq, frozen)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntegerEncodingPreparation {
    inner: ommx::IntegerEncodingPreparation,
}

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl IntegerEncodingPreparation {
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

#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass(eq, frozen)]
#[derive(Debug, Clone, PartialEq)]
pub struct FixedPenaltyPreparation {
    inner: ommx::FixedPenaltyPreparation,
}

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl FixedPenaltyPreparation {
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
    /// Apply the caller's ``policy`` to this instance in place to reach
    /// ``input_class`` membership.
    ///
    /// Selected phases are applied at most once in this order, stopping as soon
    /// as membership is reached:
    ///
    /// 1. ``special_constraints``:
    ///    {meth}`~ommx.Instance.lower_special_constraints`
    /// 2. ``sense``: {meth}`~ommx.Instance.as_minimization_problem`
    /// 3. ``integer_slack``:
    ///    {meth}`~ommx.Instance.convert_inequality_to_equality_with_integer_slack`,
    ///    followed by {meth}`~ommx.Instance.add_integer_slack_to_inequality` only
    ///    when exact conversion is unavailable and ``slack_upper_bound`` is set
    /// 4. ``integer_encoding``: {meth}`~ommx.Instance.log_encode`
    /// 5. ``fixed_penalty``
    ///
    /// Success guarantees membership only, not Adapter applicability. This
    /// operation is not transactional, so an error may leave the instance changed.
    /// {class}`~ommx.PreparationTargetNotReachedError` exposes the final membership
    /// report when the selections do not reach ``input_class``.
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
