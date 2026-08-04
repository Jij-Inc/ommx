use crate::{error::OmmxPyResult, SpecialConstraintKind};
use pyo3::{
    exceptions::{PyTypeError, PyValueError},
    prelude::*,
    types::{PyAny, PyBool, PyInt, PyMapping, PyMappingMethods, PyTuple, PyTupleMethods},
    Bound,
};
use std::collections::{BTreeMap, HashSet};

const MAX_U64: u64 = u64::MAX;

struct PositiveFinitePenaltyWeightInput(f64);

impl<'py> FromPyObject<'_, 'py> for PositiveFinitePenaltyWeightInput {
    type Error = PyErr;

    fn extract(ob: pyo3::Borrowed<'_, 'py, PyAny>) -> PyResult<Self> {
        Ok(Self(extract_positive_finite_number(
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
            let value = extract_positive_finite_number(
                &pair.get_item(1)?,
                &format!("penalty_weights[{key}]"),
            )?;
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

fn extract_positive_finite_number(value: &Bound<'_, PyAny>, field: &str) -> PyResult<f64> {
    if value.is_instance_of::<PyBool>() {
        return Err(invalid_positive_finite_number(field));
    }
    let value = value
        .extract::<f64>()
        .map_err(|_| invalid_positive_finite_number(field))?;
    if !value.is_finite() || value <= 0.0 {
        return Err(invalid_positive_finite_number(field));
    }
    Ok(value)
}

fn invalid_positive_finite_number(field: &str) -> PyErr {
    PyValueError::new_err(format!("{field} must be a positive finite number"))
}

/// Caller-owned permissions and parameters for :meth:`Instance.prepare`.
///
/// This Policy controls which transformations preparation may apply and the
/// parameters used by those transformations. Pass the target
/// :class:`InstanceClass` separately to :meth:`Instance.prepare`. Solver
/// Adapters may recommend a Policy; the caller independently supplies the
/// target, often the Adapter's ``INPUT_CLASS``.
/// Positive finite penalty weights require a minimization candidate; permit
/// sense normalization when preparing a maximization source.
/// The absolute tolerance is resolved while constructing this immutable Policy,
/// so later changes to the SDK default do not affect its interpretation.
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
    #[pyo3(signature = (*, allowed_special_constraint_lowerings=HashSet::new(), allow_integer_log_encoding=false, allow_sense_normalization=false, uniform_penalty_weight=None, penalty_weights=None, atol=None))]
    fn new(
        allowed_special_constraint_lowerings: HashSet<SpecialConstraintKind>,
        allow_integer_log_encoding: bool,
        allow_sense_normalization: bool,
        uniform_penalty_weight: Option<PositiveFinitePenaltyWeightInput>,
        penalty_weights: Option<PenaltyWeightsInput>,
        atol: Option<f64>,
    ) -> OmmxPyResult<Self> {
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

        // Resolve the mutable SDK default while constructing the immutable
        // Policy. Instance.prepare() reads only the value captured here.
        let atol = match atol {
            Some(value) => ommx::ATol::new(value)?,
            None => ommx::ATol::default(),
        };

        Ok(Self(ommx::PreparationPolicy::new(
            allowed_special_constraint_lowerings
                .into_iter()
                .map(Into::into)
                .collect(),
            allow_integer_log_encoding,
            allow_sense_normalization,
            uniform_penalty_weight,
            penalty_weights,
            atol,
        )?))
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
    /// Absolute tolerance captured by this Policy for result-affecting
    /// preparation operations.
    pub fn atol(&self) -> f64 {
        self.0.atol().into_inner()
    }

    /// Uniform finite penalty weight, or ``None`` when another/no penalty mode
    /// is configured.
    #[getter]
    pub fn uniform_penalty_weight(&self) -> Option<f64> {
        self.0.uniform_penalty_weight()
    }

    /// Per-constraint finite penalty weights.
    ///
    /// Keys identify regular constraints by their stable IDs. The map must
    /// cover every regular constraint active at the penalty step; entries for
    /// regular constraints already moved to ``removed_constraints`` are
    /// ignored. Special-constraint lowering adds fresh regular IDs; a
    /// per-constraint map must cover those IDs too. Use a uniform penalty when
    /// their weights are not configured explicitly.
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
