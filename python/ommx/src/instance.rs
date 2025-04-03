use anyhow::Result;
use ommx::{Evaluate, Message};
use pyo3::{
    prelude::*,
    types::{PyBytes, PyDict, PyList},
};
use std::collections::{BTreeSet, HashMap, HashSet};

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pyclass)]
#[pyclass]
pub struct OneHot(ommx::v1::OneHot);

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pymethods)]
#[pymethods]
impl OneHot {
    #[staticmethod]
    pub fn from_bytes(bytes: &Bound<PyBytes>) -> Result<Self> {
        let inner = ommx::v1::OneHot::decode(bytes.as_bytes())?;
        Ok(Self(inner))
    }

    pub fn to_bytes<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        Ok(PyBytes::new(py, &self.0.encode_to_vec()))
    }
}

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pyclass)]
#[pyclass]
pub struct KHot(ommx::v1::KHot);

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pymethods)]
#[pymethods]
impl KHot {
    #[staticmethod]
    pub fn from_bytes(bytes: &Bound<PyBytes>) -> Result<Self> {
        let inner = ommx::v1::KHot::decode(bytes.as_bytes())?;
        Ok(Self(inner))
    }

    pub fn to_bytes<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        Ok(PyBytes::new(py, &self.0.encode_to_vec()))
    }
}

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pyclass)]
#[pyclass]
pub struct Instance(ommx::v1::Instance);

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pymethods)]
#[pymethods]
impl Instance {
    #[staticmethod]
    pub fn from_bytes(bytes: &Bound<PyBytes>) -> Result<Self> {
        let inner = ommx::v1::Instance::decode(bytes.as_bytes())?;
        inner.validate()?;
        Ok(Self(inner))
    }

    pub fn to_bytes<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        Ok(PyBytes::new(py, &self.0.encode_to_vec()))
    }

    pub fn validate(&self) -> Result<()> {
        self.0.validate()
    }

    pub fn constraint_hints<'py>(&'py self, py: Python<'py>) -> Bound<'py, ConstraintHints> {
        let hints = self.0.constraint_hints.clone().unwrap_or_default();
        Bound::new(py, ConstraintHints(hints)).unwrap()
    }

    pub fn as_pubo_format<'py>(&self, py: Python<'py>) -> Result<Bound<'py, PyDict>> {
        let pubo = self.0.as_pubo_format()?;
        Ok(serde_pyobject::to_pyobject(py, &pubo)?.extract()?)
    }

    pub fn as_qubo_format<'py>(&self, py: Python<'py>) -> Result<(Bound<'py, PyDict>, f64)> {
        let (qubo, constant) = self.0.as_qubo_format()?;
        Ok((serde_pyobject::to_pyobject(py, &qubo)?.extract()?, constant))
    }

    pub fn as_parametric_instance(&self) -> ParametricInstance {
        ParametricInstance(self.0.clone().into())
    }

    pub fn penalty_method(&self) -> Result<ParametricInstance> {
        Ok(ParametricInstance(self.0.clone().penalty_method()?))
    }

    pub fn uniform_penalty_method(&self) -> Result<ParametricInstance> {
        Ok(ParametricInstance(self.0.clone().uniform_penalty_method()?))
    }

    pub fn evaluate_samples(&self, samples: &Samples) -> Result<SampleSet> {
        Ok(SampleSet(self.0.evaluate_samples(&samples.0)?.0))
    }

    pub fn relax_constraint(
        &mut self,
        constraint_id: u64,
        removed_reason: String,
        removed_reason_parameters: HashMap<String, String>,
    ) -> Result<()> {
        self.0
            .relax_constraint(constraint_id, removed_reason, removed_reason_parameters)
    }

    pub fn restore_constraint(&mut self, constraint_id: u64) -> Result<()> {
        self.0.restore_constraint(constraint_id)
    }

    pub fn log_encode(&mut self, integer_variable_ids: BTreeSet<u64>) -> Result<()> {
        let replacements = integer_variable_ids
            .iter()
            .map(|&id| Ok((id, self.0.log_encode(id)?.into())))
            .collect::<Result<_>>()?;
        self.0.substitute(replacements)?;
        Ok(())
    }

    pub fn convert_inequality_to_equality_with_integer_slack(
        &mut self,
        constraint_id: u64,
        max_integer_range: u64,
    ) -> Result<()> {
        self.0
            .convert_inequality_to_equality_with_integer_slack(constraint_id, max_integer_range)
    }

    pub fn add_integer_slack_to_inequality(
        &mut self,
        constraint_id: u64,
        slack_upper_bound: u64,
    ) -> Result<Option<f64>> {
        self.0
            .add_integer_slack_to_inequality(constraint_id, slack_upper_bound)
    }
}

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pyclass)]
#[pyclass]
pub struct ParametricInstance(ommx::v1::ParametricInstance);

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pymethods)]
#[pymethods]
impl ParametricInstance {
    #[staticmethod]
    pub fn from_bytes(bytes: &Bound<PyBytes>) -> Result<Self> {
        let inner = ommx::v1::ParametricInstance::decode(bytes.as_bytes())?;
        inner.validate()?;
        Ok(Self(inner))
    }

    pub fn to_bytes<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        Ok(PyBytes::new(py, &self.0.encode_to_vec()))
    }

    pub fn validate(&self) -> Result<()> {
        self.0.validate()
    }

    pub fn constraint_hints<'py>(&'py self, py: Python<'py>) -> Bound<'py, ConstraintHints> {
        let hints = self.0.constraint_hints.clone().unwrap_or_default();
        Bound::new(py, ConstraintHints(hints)).unwrap()
    }

    pub fn with_parameters(&self, parameters: &Parameters) -> Result<Instance> {
        let instance = self.0.clone().with_parameters(parameters.0.clone())?;
        Ok(Instance(instance))
    }
}

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pyclass)]
#[pyclass]
pub struct Parameters(ommx::v1::Parameters);

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pymethods)]
#[pymethods]
impl Parameters {
    #[staticmethod]
    pub fn from_bytes(bytes: &Bound<PyBytes>) -> Result<Self> {
        let inner = ommx::v1::Parameters::decode(bytes.as_bytes())?;
        Ok(Self(inner))
    }

    pub fn to_bytes<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        Ok(PyBytes::new(py, &self.0.encode_to_vec()))
    }
}

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pyclass)]
#[pyclass]
pub struct Solution(ommx::v1::Solution);

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pymethods)]
#[pymethods]
impl Solution {
    #[staticmethod]
    pub fn from_bytes(bytes: &Bound<PyBytes>) -> Result<Self> {
        let inner = ommx::v1::Solution::decode(bytes.as_bytes())?;
        Ok(Self(inner))
    }

    pub fn to_bytes<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        Ok(PyBytes::new(py, &self.0.encode_to_vec()))
    }
}

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pyclass)]
#[pyclass]
pub struct Samples(ommx::v1::Samples);

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pymethods)]
#[pymethods]
impl Samples {
    #[staticmethod]
    pub fn from_bytes(bytes: &Bound<PyBytes>) -> Result<Self> {
        let inner = ommx::v1::Samples::decode(bytes.as_bytes())?;
        Ok(Self(inner))
    }

    pub fn to_bytes<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        Ok(PyBytes::new(py, &self.0.encode_to_vec()))
    }
}

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pyclass)]
#[pyclass]
pub struct SampleSet(ommx::v1::SampleSet);

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pymethods)]
#[pymethods]
impl SampleSet {
    #[staticmethod]
    pub fn from_bytes(bytes: &Bound<PyBytes>) -> Result<Self> {
        let inner = ommx::v1::SampleSet::decode(bytes.as_bytes())?;
        Ok(Self(inner))
    }

    pub fn to_bytes<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        Ok(PyBytes::new(py, &self.0.encode_to_vec()))
    }

    pub fn get(&self, sample_id: u64) -> PyResult<Solution> {
        Ok(self.0.get(sample_id).map(Solution)?)
    }

    pub fn num_samples(&self) -> PyResult<usize> {
        Ok(self.0.num_samples()?)
    }

    pub fn sample_ids(&self) -> BTreeSet<u64> {
        self.0.sample_ids()
    }

    pub fn feasible_ids(&self) -> BTreeSet<u64> {
        self.0.feasible_ids()
    }

    pub fn feasible_unrelaxed_ids(&self) -> BTreeSet<u64> {
        self.0.feasible_unrelaxed_ids()
    }

    pub fn best_feasible(&self) -> PyResult<Solution> {
        Ok(self.0.best_feasible().map(Solution)?)
    }

    pub fn best_feasible_unrelaxed(&self) -> PyResult<Solution> {
        Ok(self.0.best_feasible_unrelaxed().map(Solution)?)
    }
}

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pyclass)]
#[pyclass]
pub struct ConstraintHints(ommx::v1::ConstraintHints);

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pymethods)]
#[pymethods]
impl ConstraintHints {
    #[staticmethod]
    pub fn from_bytes(bytes: &Bound<PyBytes>) -> Result<Self> {
        let inner = ommx::v1::ConstraintHints::decode(bytes.as_bytes())?;
        Ok(Self(inner))
    }

    pub fn to_bytes<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        Ok(PyBytes::new(py, &self.0.encode_to_vec()))
    }

    pub fn one_hot_constraints(&self) -> Vec<OneHot> {
        let mut result = Vec::new();
        let mut constraint_ids = HashSet::<u64>::new();

        #[allow(deprecated)]
        for constraint in &self.0.one_hot_constraints {
            constraint_ids.insert(constraint.constraint_id);
            result.push(OneHot(constraint.clone()));
        }

        if let Some(k_hot_list) = self.0.k_hot_constraints.get(&1) {
            for k_hot in &k_hot_list.constraints {
                if !constraint_ids.contains(&k_hot.constraint_id) {
                    let mut one_hot = ommx::v1::OneHot::default();
                    one_hot.constraint_id = k_hot.constraint_id;
                    one_hot.decision_variables = k_hot.decision_variables.clone();
                    result.push(OneHot(one_hot));
                }
            }
        }

        result
    }

    pub fn k_hot_constraints<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let result = PyDict::new(py);

        for (k, k_hot_list) in &self.0.k_hot_constraints {
            let constraints_list = PyList::empty(py);
            for constraint in &k_hot_list.constraints {
                constraints_list.append(constraint.encode_to_vec().as_slice())?;
            }
            result.set_item(k, constraints_list)?;
        }

        let k1_constraint_ids: HashSet<u64> = match self.0.k_hot_constraints.get(&1) {
            Some(k_hot_list) => k_hot_list
                .constraints
                .iter()
                .map(|c| c.constraint_id)
                .collect(),
            None => HashSet::new(),
        };

        #[allow(deprecated)]
        if !self.0.one_hot_constraints.is_empty() {
            let k1_list = PyList::empty(py);

            if let Some(existing_list) = result.get_item(1)? {
                let existing_list = existing_list.downcast::<PyList>()?;
                for item in existing_list.iter() {
                    k1_list.append(item)?;
                }
            }

            #[allow(deprecated)]
            for one_hot in &self.0.one_hot_constraints {
                if !k1_constraint_ids.contains(&one_hot.constraint_id) {
                    let mut k_hot = ommx::v1::KHot::default();
                    k_hot.constraint_id = one_hot.constraint_id;
                    k_hot.decision_variables = one_hot.decision_variables.clone();
                    k_hot.num_hot_vars = 1;
                    k1_list.append(k_hot.encode_to_vec().as_slice())?;
                }
            }

            if !k1_list.is_empty() {
                result.set_item(1, k1_list)?;
            }
        }

        Ok(result)
    }
}
