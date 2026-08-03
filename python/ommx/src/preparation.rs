use crate::{
    error::OmmxPyResult, Instance, InstanceClass, InstanceClassMembershipReport, Samples,
    SpecialConstraintKind, State,
};
use pyo3::{
    exceptions::{PyTypeError, PyValueError},
    prelude::*,
    types::{PyAny, PyBool, PyInt, PyMapping, PyMappingMethods, PyTuple, PyTupleMethods},
    Bound,
};
use std::collections::{BTreeMap, HashSet};

const MAX_U64: u64 = u64::MAX;

struct IntegerSlackMaxRangeInput(u64);

impl<'py> FromPyObject<'_, 'py> for IntegerSlackMaxRangeInput {
    type Error = PyErr;

    fn extract(ob: pyo3::Borrowed<'_, 'py, PyAny>) -> PyResult<Self> {
        let value = ob.to_owned();
        if value.is_instance_of::<PyBool>() || !value.is_instance_of::<PyInt>() {
            return Err(invalid_integer_slack_max_range());
        }
        let value = value
            .extract::<u64>()
            .map_err(|_| invalid_integer_slack_max_range())?;
        if value == 0 {
            return Err(invalid_integer_slack_max_range());
        }
        Ok(Self(value))
    }
}

impl pyo3_stub_gen::PyStubType for IntegerSlackMaxRangeInput {
    fn type_output() -> pyo3_stub_gen::TypeInfo {
        <u64 as pyo3_stub_gen::PyStubType>::type_output()
    }
}

fn invalid_integer_slack_max_range() -> PyErr {
    PyValueError::new_err(format!(
        "inequality_integer_slack_max_range must be an integer in [1, {MAX_U64}]"
    ))
}

struct ResourceLimitInput(usize);

impl<'py> FromPyObject<'_, 'py> for ResourceLimitInput {
    type Error = PyErr;

    fn extract(ob: pyo3::Borrowed<'_, 'py, PyAny>) -> PyResult<Self> {
        let value = ob.to_owned();
        if value.is_instance_of::<PyBool>() || !value.is_instance_of::<PyInt>() {
            return Err(invalid_resource_limit());
        }
        value
            .extract::<usize>()
            .map(Self)
            .map_err(|_| invalid_resource_limit())
    }
}

impl pyo3_stub_gen::PyStubType for ResourceLimitInput {
    fn type_output() -> pyo3_stub_gen::TypeInfo {
        <usize as pyo3_stub_gen::PyStubType>::type_output()
    }
}

fn invalid_resource_limit() -> PyErr {
    PyValueError::new_err("Preparation resource limits must be non-negative integers")
}

struct PositiveFinitePenaltyWeightInput(f64);

impl<'py> FromPyObject<'_, 'py> for PositiveFinitePenaltyWeightInput {
    type Error = PyErr;

    fn extract(ob: pyo3::Borrowed<'_, 'py, PyAny>) -> PyResult<Self> {
        Ok(Self(extract_penalty_weight(
            &ob.to_owned(),
            "uniform_penalty_weight",
        )?))
    }
}

impl pyo3_stub_gen::PyStubType for PositiveFinitePenaltyWeightInput {
    fn type_output() -> pyo3_stub_gen::TypeInfo {
        <f64 as pyo3_stub_gen::PyStubType>::type_output()
    }
}

struct PenaltyWeightsInput(BTreeMap<u64, f64>);

impl<'py> FromPyObject<'_, 'py> for PenaltyWeightsInput {
    type Error = PyErr;

    fn extract(ob: pyo3::Borrowed<'_, 'py, PyAny>) -> PyResult<Self> {
        let mapping = ob
            .cast::<PyMapping>()
            .map_err(|_| PyTypeError::new_err("penalty_weights must be a Mapping[int, float]"))?;
        let mut snapshot = BTreeMap::new();
        for item in mapping.items()?.iter() {
            let pair = item.cast::<PyTuple>().map_err(|_| {
                PyTypeError::new_err("penalty_weights must be a Mapping[int, float]")
            })?;
            let key = extract_penalty_weight_key(&pair.get_item(0)?)?;
            let value =
                extract_penalty_weight(&pair.get_item(1)?, &format!("penalty_weights[{key}]"))?;
            snapshot.insert(key, value);
        }
        Ok(Self(snapshot))
    }
}

impl pyo3_stub_gen::PyStubType for PenaltyWeightsInput {
    fn type_output() -> pyo3_stub_gen::TypeInfo {
        <BTreeMap<u64, f64> as pyo3_stub_gen::PyStubType>::type_output()
    }

    fn type_input() -> pyo3_stub_gen::TypeInfo {
        pyo3_stub_gen::TypeInfo {
            import: ["collections.abc".into()].into(),
            name: "collections.abc.Mapping[int, float]".into(),
            source_module: None,
            type_refs: Default::default(),
        }
    }
}

fn extract_penalty_weight_key(value: &Bound<'_, PyAny>) -> PyResult<u64> {
    if value.is_instance_of::<PyBool>() || !value.is_instance_of::<PyInt>() {
        return Err(invalid_penalty_weight_key());
    }
    value
        .extract::<u64>()
        .map_err(|_| invalid_penalty_weight_key())
}

fn invalid_penalty_weight_key() -> PyErr {
    PyValueError::new_err(format!(
        "penalty_weights keys must be integer constraint IDs in [0, {MAX_U64}]"
    ))
}

fn extract_penalty_weight(value: &Bound<'_, PyAny>, field: &str) -> PyResult<f64> {
    if value.is_instance_of::<PyBool>() {
        return Err(invalid_penalty_weight(field));
    }
    let value = value
        .extract::<f64>()
        .map_err(|_| invalid_penalty_weight(field))?;
    if !value.is_finite() || value <= 0.0 {
        return Err(invalid_penalty_weight(field));
    }
    Ok(value)
}

fn invalid_penalty_weight(field: &str) -> PyErr {
    PyValueError::new_err(format!("{field} must be a positive finite number"))
}

/// Caller-owned permissions and parameters for :meth:`Instance.prepare`.
///
/// This is an OMMX core value. Solver Adapters may recommend a policy, but the
/// policy neither retains an Adapter nor calls Adapter-owned Python code.
#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass(frozen)]
#[derive(Clone)]
pub struct PreparationPolicy(pub(crate) ommx::PreparationPolicy);

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl PreparationPolicy {
    #[new]
    #[pyo3(signature = (*, acceptable_instance_class, allowed_special_constraint_lowerings=HashSet::new(), allow_integer_log_encoding=false, allow_sense_normalization=false, inequality_integer_slack_max_range=None, allow_approximate_integer_slack=false, uniform_penalty_weight=None, penalty_weights=None, atol=None, max_added_decision_variables=None, max_added_regular_constraints=None))]
    #[allow(clippy::too_many_arguments)]
    fn new(
        acceptable_instance_class: &InstanceClass,
        allowed_special_constraint_lowerings: HashSet<SpecialConstraintKind>,
        allow_integer_log_encoding: bool,
        allow_sense_normalization: bool,
        inequality_integer_slack_max_range: Option<IntegerSlackMaxRangeInput>,
        allow_approximate_integer_slack: bool,
        uniform_penalty_weight: Option<PositiveFinitePenaltyWeightInput>,
        penalty_weights: Option<PenaltyWeightsInput>,
        atol: Option<f64>,
        max_added_decision_variables: Option<ResourceLimitInput>,
        max_added_regular_constraints: Option<ResourceLimitInput>,
    ) -> OmmxPyResult<Self> {
        let integer_slack_policy = match inequality_integer_slack_max_range {
            Some(IntegerSlackMaxRangeInput(max_range)) if allow_approximate_integer_slack => {
                Some(ommx::IntegerSlackPolicy::exact_or_approximate(max_range)?)
            }
            Some(IntegerSlackMaxRangeInput(max_range)) => {
                Some(ommx::IntegerSlackPolicy::exact(max_range)?)
            }
            None if allow_approximate_integer_slack => {
                return Err(PyValueError::new_err(
                    "allow_approximate_integer_slack requires inequality_integer_slack_max_range",
                )
                .into());
            }
            None => None,
        };

        let penalty_weights = match (uniform_penalty_weight, penalty_weights) {
            (Some(_), Some(_)) => {
                return Err(PyValueError::new_err(
                    "uniform_penalty_weight and penalty_weights are mutually exclusive",
                )
                .into());
            }
            (Some(PositiveFinitePenaltyWeightInput(weight)), None) => {
                Some(ommx::PenaltyWeights::Uniform { weight })
            }
            (None, Some(PenaltyWeightsInput(weights))) => {
                Some(ommx::PenaltyWeights::PerConstraint {
                    weights: weights
                        .into_iter()
                        .map(|(id, weight)| (ommx::ConstraintID::from(id), weight))
                        .collect(),
                })
            }
            (None, None) => None,
        };

        let atol = match atol {
            Some(value) => ommx::ATol::new(value)?,
            None => ommx::ATol::default(),
        };
        Ok(Self(ommx::PreparationPolicy::new(
            acceptable_instance_class.0.clone(),
            allowed_special_constraint_lowerings
                .into_iter()
                .map(Into::into)
                .collect(),
            allow_integer_log_encoding,
            allow_sense_normalization,
            integer_slack_policy,
            penalty_weights,
            atol,
            max_added_decision_variables.map(|limit| limit.0),
            max_added_regular_constraints.map(|limit| limit.0),
        )?))
    }

    #[getter]
    /// InstanceClass that the prepared input must belong to.
    pub fn acceptable_instance_class(&self) -> InstanceClass {
        InstanceClass(self.0.acceptable_instance_class().clone())
    }

    #[getter]
    /// Special-constraint families whose existing OMMX lowering may be used.
    pub fn allowed_special_constraint_lowerings(&self) -> HashSet<SpecialConstraintKind> {
        self.0
            .allowed_special_constraint_lowerings()
            .iter()
            .copied()
            .map(Into::into)
            .collect()
    }

    #[getter]
    /// Whether bounded Integer variables may be log encoded.
    pub fn allow_integer_log_encoding(&self) -> bool {
        self.0.allow_integer_log_encoding()
    }

    #[getter]
    /// Whether a maximization objective may be normalized to minimization.
    pub fn allow_sense_normalization(&self) -> bool {
        self.0.allow_sense_normalization()
    }

    #[getter]
    /// Configured positive slack range, or ``None`` when integer slack is forbidden.
    pub fn inequality_integer_slack_max_range(&self) -> Option<u64> {
        self.0
            .integer_slack_policy()
            .map(|policy| policy.max_range())
    }

    #[getter]
    /// Whether approximate integer slack may be used after exact slack is unavailable.
    pub fn allow_approximate_integer_slack(&self) -> bool {
        self.0
            .integer_slack_policy()
            .is_some_and(|policy| policy.allow_approximate())
    }

    /// Uniform finite penalty weight, or ``None`` when another/no penalty mode
    /// is configured.
    #[getter]
    pub fn uniform_penalty_weight(&self) -> Option<f64> {
        match self.0.penalty_weights() {
            Some(ommx::PenaltyWeights::Uniform { weight }) => Some(*weight),
            _ => None,
        }
    }

    /// Per-constraint finite penalty weights.
    ///
    /// Keys identify source regular constraints by their stable IDs; those IDs
    /// are preserved by Preparation Transforms. The map must cover every
    /// source regular constraint still active at the penalty step, while an
    /// entry for one removed as trivial is allowed. Special-constraint
    /// lowering adds new regular rows outside this source-map domain, so a
    /// uniform penalty is required when such rows remain active.
    #[getter]
    pub fn penalty_weights(&self) -> Option<BTreeMap<u64, f64>> {
        match self.0.penalty_weights() {
            Some(ommx::PenaltyWeights::PerConstraint { weights }) => Some(
                weights
                    .iter()
                    .map(|(id, weight)| (id.into_inner(), *weight))
                    .collect(),
            ),
            _ => None,
        }
    }

    #[getter]
    /// Absolute tolerance snapshotted into every Transform that uses it.
    pub fn atol(&self) -> f64 {
        self.0.atol().into_inner()
    }

    #[getter]
    /// Maximum number of decision variables introduced relative to the source.
    pub fn max_added_decision_variables(&self) -> Option<usize> {
        self.0.max_added_decision_variables()
    }

    #[getter]
    /// Maximum number of active regular constraints introduced relative to the source.
    pub fn max_added_regular_constraints(&self) -> Option<usize> {
        self.0.max_added_regular_constraints()
    }

    pub fn __repr__(&self) -> String {
        format!("PreparationPolicy({:?})", self.0)
    }
}

/// One OMMX Transform applied by :meth:`Instance.prepare`.
///
/// Values are created while OMMX applies an Instance operation. They are
/// immutable, source-specific receipts, not caller-supplied instructions and
/// not reusable ``apply`` objects.
#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass(frozen)]
#[derive(Clone)]
pub struct Transform(pub(crate) ommx::Transform);

/// Structured evidence from an unsuccessful canonical Preparation attempt.
///
/// The corresponding Python exception exposes this value as
/// :attr:`PreparationError.failure`.
///
/// ``PolicyMismatch`` carries ``source``, ``policy``, ``current_membership``,
/// committed ``transforms``, and ``detail``. ``ResourceLimitExceeded`` carries
/// the same snapshots plus ``operation``, ``resource``, ``observed``, and
/// ``limit``. ``TargetInstanceClassNotReached`` carries the common snapshots
/// plus ``detail``. Errors owned by a mathematical Transform are not converted
/// to these variants.
#[pyo3_stub_gen::derive::gen_stub_pyclass_complex_enum]
#[pyclass(frozen)]
#[derive(Clone)]
pub enum PreparationFailure {
    PolicyMismatch {
        source: Instance,
        policy: PreparationPolicy,
        current_membership: InstanceClassMembershipReport,
        transforms: Vec<Transform>,
        detail: String,
    },
    ResourceLimitExceeded {
        source: Instance,
        policy: PreparationPolicy,
        current_membership: InstanceClassMembershipReport,
        transforms: Vec<Transform>,
        operation: String,
        resource: String,
        observed: usize,
        limit: usize,
    },
    TargetInstanceClassNotReached {
        source: Instance,
        policy: PreparationPolicy,
        current_membership: InstanceClassMembershipReport,
        transforms: Vec<Transform>,
        detail: String,
    },
}

impl PreparationFailure {
    pub(crate) fn from_core(failure: &ommx::PreparationFailure) -> Self {
        match failure {
            ommx::PreparationFailure::PolicyMismatch {
                source,
                policy,
                current_membership,
                transforms,
                detail,
            } => Self::PolicyMismatch {
                source: Instance {
                    inner: source.clone(),
                },
                policy: PreparationPolicy(policy.clone()),
                current_membership: InstanceClassMembershipReport(current_membership.clone()),
                transforms: transforms.iter().cloned().map(Transform).collect(),
                detail: detail.clone(),
            },
            ommx::PreparationFailure::ResourceLimitExceeded {
                source,
                policy,
                current_membership,
                transforms,
                operation,
                resource,
                observed,
                limit,
            } => Self::ResourceLimitExceeded {
                source: Instance {
                    inner: source.clone(),
                },
                policy: PreparationPolicy(policy.clone()),
                current_membership: InstanceClassMembershipReport(current_membership.clone()),
                transforms: transforms.iter().cloned().map(Transform).collect(),
                operation: operation.clone(),
                resource: resource.clone(),
                observed: *observed,
                limit: *limit,
            },
            ommx::PreparationFailure::TargetInstanceClassNotReached {
                source,
                policy,
                current_membership,
                transforms,
                detail,
            } => Self::TargetInstanceClassNotReached {
                source: Instance {
                    inner: source.clone(),
                },
                policy: PreparationPolicy(policy.clone()),
                current_membership: InstanceClassMembershipReport(current_membership.clone()),
                transforms: transforms.iter().cloned().map(Transform).collect(),
                detail: detail.clone(),
            },
            _ => unreachable!(
                "the Python binding must project every PreparationFailure variant from its pinned core version"
            ),
        }
    }
}

/// Typed carrier for the two state-domain inputs accepted by Transform and
/// Preparation. It is private so no ``Any``-typed surface leaks into Python.
enum PreparationData {
    State(State),
    Samples(Samples),
}

impl<'py> FromPyObject<'_, 'py> for PreparationData {
    type Error = PyErr;

    fn extract(ob: pyo3::Borrowed<'_, 'py, PyAny>) -> PyResult<Self> {
        if let Ok(state) = ob.cast::<State>() {
            return Ok(Self::State(state.borrow().clone()));
        }
        if let Ok(samples) = ob.cast::<Samples>() {
            return Ok(Self::Samples(samples.borrow().clone()));
        }
        Err(PyTypeError::new_err(
            "data must be an ommx.State or ommx.Samples",
        ))
    }
}

impl<'py> IntoPyObject<'py> for PreparationData {
    type Target = PyAny;
    type Output = Bound<'py, PyAny>;
    type Error = PyErr;

    fn into_pyobject(self, py: Python<'py>) -> PyResult<Self::Output> {
        match self {
            Self::State(state) => Ok(Py::new(py, state)?.into_bound(py).into_any()),
            Self::Samples(samples) => Ok(Py::new(py, samples)?.into_bound(py).into_any()),
        }
    }
}

impl pyo3_stub_gen::PyStubType for PreparationData {
    fn type_input() -> pyo3_stub_gen::TypeInfo {
        <State as pyo3_stub_gen::PyStubType>::type_output()
            | <Samples as pyo3_stub_gen::PyStubType>::type_output()
    }

    fn type_output() -> pyo3_stub_gen::TypeInfo {
        <State as pyo3_stub_gen::PyStubType>::type_output()
            | <Samples as pyo3_stub_gen::PyStubType>::type_output()
    }
}

// Preserve the input-dependent return types in generated stubs. Runtime
// extraction remains restricted to the two concrete SDK classes above.
pyo3_stub_gen::inventory::submit! {
    pyo3_stub_gen::derive::gen_methods_from_python! {
        r#"
        class Transform:
            @overload
            def encode(self, data: State) -> State:
                """Mechanically map State or Samples to this Transform's input side.

                This maps decision-variable state only; it does not evaluate an
                Instance or assert feasibility.
                """
            @overload
            def encode(self, data: Samples) -> Samples:
                """Map samples pointwise while preserving IDs and grouping."""
            @overload
            def decode(self, data: State) -> State:
                """Mechanically map State or Samples back to this Transform's source side.

                This does not transport objectives, feasibility, or solver status.
                """
            @overload
            def decode(self, data: Samples) -> Samples:
                """Map samples back pointwise while preserving IDs and grouping."""
        "#
    }
}

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl Transform {
    /// Stable operation name for this applied Transform receipt.
    #[getter]
    pub fn name(&self) -> &str {
        self.0.name()
    }

    #[gen_stub(skip)]
    /// Mechanically map a :class:`State` or :class:`Samples` value from this
    /// Transform's source-side variable namespace to its input-side namespace.
    ///
    /// This does not evaluate an Instance or assert feasibility. ``Samples``
    /// IDs and grouping are preserved.
    fn encode(&self, data: PreparationData) -> OmmxPyResult<PreparationData> {
        match data {
            PreparationData::State(state) => {
                Ok(PreparationData::State(State(self.0.encode(&state.0)?)))
            }
            PreparationData::Samples(samples) => Ok(PreparationData::Samples(Samples(
                self.0.encode_samples(&samples.0)?,
            ))),
        }
    }

    #[gen_stub(skip)]
    /// Mechanically map a :class:`State` or :class:`Samples` value from this
    /// Transform's input-side variable namespace back to its source side.
    ///
    /// This does not transport objectives, feasibility, or solver statuses and
    /// is not promised to invert every possible encoding.
    fn decode(&self, data: PreparationData) -> OmmxPyResult<PreparationData> {
        match data {
            PreparationData::State(state) => {
                Ok(PreparationData::State(State(self.0.decode(&state.0)?)))
            }
            PreparationData::Samples(samples) => Ok(PreparationData::Samples(Samples(
                self.0.decode_samples(&samples.0)?,
            ))),
        }
    }

    pub fn __repr__(&self) -> String {
        format!("Transform({:?})", self.0)
    }
}

/// An immutable, auditable relation between a source Instance and the prepared
/// input produced from it under one :class:`PreparationPolicy`.
#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass(frozen)]
#[derive(Clone)]
pub struct Preparation(pub(crate) ommx::Preparation);

// Preserve the input-dependent return types in the generated Python stub.
pyo3_stub_gen::inventory::submit! {
    pyo3_stub_gen::derive::gen_methods_from_python! {
        r#"
        class Preparation:
            @overload
            def encode(self, data: State) -> State:
                """Compose applied Transform encoders for State or Samples.

                Solution and SampleSet are not accepted, and no model is evaluated.
                """
            @overload
            def encode(self, data: Samples) -> Samples:
                """Compose encoders pointwise while preserving IDs and grouping."""
            @overload
            def decode(self, data: State) -> State:
                """Compose applied Transform decoders for State or Samples.

                This performs no source evaluation and transports no solver status.
                """
            @overload
            def decode(self, data: Samples) -> Samples:
                """Compose decoders pointwise while preserving IDs and grouping."""
        "#
    }
}

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl Preparation {
    #[getter]
    /// Isolated snapshot of the Instance passed to :meth:`Instance.prepare`.
    pub fn source(&self) -> Instance {
        Instance {
            inner: self.0.source().clone(),
        }
    }

    #[getter]
    /// Constructed Instance belonging to the Policy's acceptable class.
    pub fn input(&self) -> Instance {
        Instance {
            inner: self.0.input().clone(),
        }
    }

    #[getter]
    /// Isolated snapshot of the Policy passed to :meth:`Instance.prepare`.
    pub fn policy(&self) -> PreparationPolicy {
        PreparationPolicy(self.0.policy().clone())
    }

    #[getter]
    /// Applied Transform receipts in construction order.
    pub fn transforms(&self) -> Vec<Transform> {
        self.0.transforms().iter().cloned().map(Transform).collect()
    }

    #[gen_stub(skip)]
    /// Compose the applied Transform encoders in construction order.
    ///
    /// Accepts only :class:`State` or :class:`Samples`; it does not accept a
    /// Solution or SampleSet and performs no source evaluation.
    fn encode(&self, data: PreparationData) -> OmmxPyResult<PreparationData> {
        match data {
            PreparationData::State(state) => {
                Ok(PreparationData::State(State(self.0.encode(&state.0)?)))
            }
            PreparationData::Samples(samples) => Ok(PreparationData::Samples(Samples(
                self.0.encode_samples(&samples.0)?,
            ))),
        }
    }

    #[gen_stub(skip)]
    /// Compose the applied Transform decoders in reverse construction order.
    ///
    /// Accepts only :class:`State` or :class:`Samples`. Evaluate the returned
    /// state data against :attr:`source` explicitly when source-side results
    /// are needed.
    fn decode(&self, data: PreparationData) -> OmmxPyResult<PreparationData> {
        match data {
            PreparationData::State(state) => {
                Ok(PreparationData::State(State(self.0.decode(&state.0)?)))
            }
            PreparationData::Samples(samples) => Ok(PreparationData::Samples(Samples(
                self.0.decode_samples(&samples.0)?,
            ))),
        }
    }

    pub fn __repr__(&self) -> String {
        format!("Preparation({:?})", self.0)
    }
}
