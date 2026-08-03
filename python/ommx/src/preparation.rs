use crate::{error::OmmxPyResult, InstanceClass, SpecialConstraintKind};
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
/// Positive finite penalty weights require a minimization candidate; permit
/// sense normalization when preparing a maximization source.
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
    #[pyo3(signature = (*, acceptable_instance_class, allowed_special_constraint_lowerings=HashSet::new(), allow_integer_log_encoding=false, allow_sense_normalization=false, inequality_integer_slack_max_range=None, allow_approximate_integer_slack=false, uniform_penalty_weight=None, penalty_weights=None))]
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
    ) -> OmmxPyResult<Self> {
        let inequality_integer_slack_max_range = match inequality_integer_slack_max_range {
            Some(IntegerSlackMaxRangeInput(max_range)) => Some(max_range),
            None if allow_approximate_integer_slack => {
                return Err(PyValueError::new_err(
                    "allow_approximate_integer_slack requires inequality_integer_slack_max_range",
                )
                .into());
            }
            None => None,
        };

        let (uniform_penalty_weight, penalty_weights) =
            match (uniform_penalty_weight, penalty_weights) {
                (Some(_), Some(_)) => {
                    return Err(PyValueError::new_err(
                        "uniform_penalty_weight and penalty_weights are mutually exclusive",
                    )
                    .into());
                }
                (Some(PositiveFinitePenaltyWeightInput(weight)), None) => (Some(weight), None),
                (None, Some(PenaltyWeightsInput(weights))) => (
                    None,
                    Some(
                        weights
                            .into_iter()
                            .map(|(id, weight)| (ommx::ConstraintID::from(id), weight))
                            .collect(),
                    ),
                ),
                (None, None) => (None, None),
            };

        Ok(Self(ommx::PreparationPolicy::new(
            acceptable_instance_class.0.clone(),
            allowed_special_constraint_lowerings
                .into_iter()
                .map(Into::into)
                .collect(),
            allow_integer_log_encoding,
            allow_sense_normalization,
            inequality_integer_slack_max_range,
            allow_approximate_integer_slack,
            uniform_penalty_weight,
            penalty_weights,
        )?))
    }

    #[getter]
    /// InstanceClass that the Instance must belong to after preparation.
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
        self.0.inequality_integer_slack_max_range()
    }

    #[getter]
    /// Whether approximate integer slack may be used after exact slack is unavailable.
    pub fn allow_approximate_integer_slack(&self) -> bool {
        self.0.allow_approximate_integer_slack()
    }

    /// Uniform finite penalty weight, or ``None`` when another/no penalty mode
    /// is configured.
    #[getter]
    pub fn uniform_penalty_weight(&self) -> Option<f64> {
        self.0.uniform_penalty_weight()
    }

    /// Per-constraint finite penalty weights.
    ///
    /// Keys identify source regular constraints by their stable IDs. The map
    /// must cover every source regular constraint still active at the penalty
    /// step, while an entry for one removed as trivial is allowed.
    /// Special-constraint lowering adds new regular rows outside this
    /// source-map domain, so a uniform penalty is required when such rows
    /// remain active.
    #[getter]
    pub fn penalty_weights(&self) -> Option<BTreeMap<u64, f64>> {
        self.0.penalty_weights().map(|weights| {
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
