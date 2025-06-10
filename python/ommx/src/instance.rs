use crate::{Constraint, DecisionVariable, Function, RemovedConstraint, VariableBound};
use anyhow::Result;
use ommx::{ConstraintID, Evaluate, Message, Parse, VariableID};
use pyo3::{
    prelude::*,
    types::{PyBytes, PyDict},
};
use std::collections::{BTreeMap, BTreeSet, HashMap};

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pyclass)]
#[pyclass]
pub struct Instance(ommx::Instance);

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pymethods)]
#[pymethods]
impl Instance {
    #[staticmethod]
    pub fn from_bytes(bytes: &Bound<PyBytes>) -> Result<Self> {
        let inner = ommx::v1::Instance::decode(bytes.as_bytes())?;
        let parsed = Parse::parse(inner.clone(), &())?;
        Ok(Self(parsed))
    }

    #[staticmethod]
    #[pyo3(signature = (sense, objective, decision_variables, constraints))]
    pub fn from_components(
        sense: u32,
        objective: Function,
        decision_variables: HashMap<u64, DecisionVariable>,
        constraints: HashMap<u64, Constraint>,
    ) -> Result<Self> {
        let rust_sense = match sense {
            1 => ommx::Sense::Minimize,
            2 => ommx::Sense::Maximize,
            _ => return Err(anyhow::anyhow!("Invalid sense: {}", sense).into()),
        };

        let rust_decision_variables: BTreeMap<VariableID, ommx::DecisionVariable> =
            decision_variables
                .into_iter()
                .map(|(id, var)| (VariableID::from(id), var.0))
                .collect();

        let rust_constraints: BTreeMap<ConstraintID, ommx::Constraint> = constraints
            .into_iter()
            .map(|(id, constraint)| (ConstraintID::from(id), constraint.0))
            .collect();

        let instance = ommx::Instance::new(
            rust_sense,
            objective.0,
            rust_decision_variables,
            rust_constraints,
            ommx::ConstraintHints::default(),
        )?;

        Ok(Self(instance))
    }

    pub fn get_sense(&self) -> u32 {
        match self.0.sense() {
            ommx::Sense::Minimize => 1,
            ommx::Sense::Maximize => 2,
        }
    }

    pub fn get_objective(&self) -> Function {
        Function(self.0.objective().clone())
    }

    pub fn get_decision_variables(&self) -> HashMap<u64, DecisionVariable> {
        self.0
            .decision_variables()
            .iter()
            .map(|(id, var)| (id.into_inner(), DecisionVariable(var.clone())))
            .collect()
    }

    pub fn get_constraints(&self) -> HashMap<u64, Constraint> {
        self.0
            .constraints()
            .iter()
            .map(|(id, constraint)| (id.into_inner(), Constraint(constraint.clone())))
            .collect()
    }

    pub fn get_removed_constraints(&self) -> HashMap<u64, RemovedConstraint> {
        self.0
            .removed_constraints()
            .iter()
            .map(|(id, removed_constraint)| {
                (
                    id.into_inner(),
                    RemovedConstraint(removed_constraint.clone()),
                )
            })
            .collect()
    }

    pub fn to_bytes<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        let inner: ommx::v1::Instance = self.0.clone().into();
        Ok(PyBytes::new(py, &inner.encode_to_vec()))
    }

    pub fn required_ids(&self) -> BTreeSet<u64> {
        self.0
            .required_ids()
            .into_iter()
            .map(|id| id.into_inner())
            .collect()
    }

    pub fn as_pubo_format<'py>(&self, py: Python<'py>) -> Result<Bound<'py, PyDict>> {
        let inner: ommx::v1::Instance = self.0.clone().into();
        let pubo = inner.as_pubo_format()?;
        Ok(serde_pyobject::to_pyobject(py, &pubo)?.extract()?)
    }

    pub fn as_qubo_format<'py>(&self, py: Python<'py>) -> Result<(Bound<'py, PyDict>, f64)> {
        let inner: ommx::v1::Instance = self.0.clone().into();
        let (qubo, constant) = inner.as_qubo_format()?;
        Ok((serde_pyobject::to_pyobject(py, &qubo)?.extract()?, constant))
    }

    pub fn as_parametric_instance(&self) -> ParametricInstance {
        let inner: ommx::v1::Instance = self.0.clone().into();
        ParametricInstance(inner.into())
    }

    pub fn penalty_method(&self) -> Result<ParametricInstance> {
        let inner: ommx::v1::Instance = self.0.clone().into();
        Ok(ParametricInstance(inner.penalty_method()?))
    }

    pub fn uniform_penalty_method(&self) -> Result<ParametricInstance> {
        let inner: ommx::v1::Instance = self.0.clone().into();
        Ok(ParametricInstance(inner.uniform_penalty_method()?))
    }

    pub fn evaluate_samples(&self, samples: &Samples) -> Result<SampleSet> {
        Ok(SampleSet(
            self.0.evaluate_samples(&samples.0, ommx::ATol::default())?,
        ))
    }

    pub fn relax_constraint(
        &mut self,
        constraint_id: u64,
        removed_reason: String,
        removed_reason_parameters: HashMap<String, String>,
    ) -> Result<()> {
        self.0.relax_constraint(
            constraint_id.into(),
            removed_reason,
            removed_reason_parameters,
        )?;
        Ok(())
    }

    pub fn restore_constraint(&mut self, constraint_id: u64) -> Result<()> {
        self.0.restore_constraint(constraint_id.into())?;
        Ok(())
    }

    pub fn log_encode(&mut self, integer_variable_ids: BTreeSet<u64>) -> Result<()> {
        let mut inner: ommx::v1::Instance = self.0.clone().into();
        let replacements = integer_variable_ids
            .iter()
            .map(|&id| Ok((id, inner.log_encode(id)?.into())))
            .collect::<Result<_>>()?;
        inner.substitute(replacements)?;
        self.0 = Parse::parse(inner, &())?;
        Ok(())
    }

    pub fn convert_inequality_to_equality_with_integer_slack(
        &mut self,
        constraint_id: u64,
        max_integer_range: u64,
    ) -> Result<()> {
        let mut inner: ommx::v1::Instance = self.0.clone().into();
        inner.convert_inequality_to_equality_with_integer_slack(
            constraint_id,
            max_integer_range,
            ommx::ATol::default(),
        )?;
        self.0 = Parse::parse(inner, &())?;
        Ok(())
    }

    pub fn add_integer_slack_to_inequality(
        &mut self,
        constraint_id: u64,
        slack_upper_bound: u64,
    ) -> Result<Option<f64>> {
        let mut inner: ommx::v1::Instance = self.0.clone().into();
        let result = inner.add_integer_slack_to_inequality(constraint_id, slack_upper_bound)?;
        self.0 = Parse::parse(inner, &())?;
        Ok(result)
    }

    pub fn decision_variable_analysis(&self) -> DecisionVariableAnalysis {
        DecisionVariableAnalysis(self.0.analyze_decision_variables())
    }
}

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pyclass)]
#[pyclass]
pub struct DecisionVariableAnalysis(ommx::DecisionVariableAnalysis);

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pymethods)]
#[pymethods]
impl DecisionVariableAnalysis {
    pub fn used_binary(&self) -> BTreeMap<u64, VariableBound> {
        self.0
            .used_binary()
            .into_iter()
            .map(|(id, bound)| (id.into_inner(), VariableBound(bound)))
            .collect()
    }

    pub fn used_integer(&self) -> BTreeMap<u64, VariableBound> {
        self.0
            .used_integer()
            .into_iter()
            .map(|(id, bound)| (id.into_inner(), VariableBound(bound)))
            .collect()
    }

    pub fn used_continuous(&self) -> BTreeMap<u64, VariableBound> {
        self.0
            .used_continuous()
            .into_iter()
            .map(|(id, bound)| (id.into_inner(), VariableBound(bound)))
            .collect()
    }

    pub fn used_semi_integer(&self) -> BTreeMap<u64, VariableBound> {
        self.0
            .used_semi_integer()
            .into_iter()
            .map(|(id, bound)| (id.into_inner(), VariableBound(bound)))
            .collect()
    }

    pub fn used_semi_continuous(&self) -> BTreeMap<u64, VariableBound> {
        self.0
            .used_semi_continuous()
            .into_iter()
            .map(|(id, bound)| (id.into_inner(), VariableBound(bound)))
            .collect()
    }

    pub fn used_decision_variable_ids(&self) -> BTreeSet<u64> {
        self.0.used().iter().map(|id| id.into_inner()).collect()
    }

    pub fn all_decision_variable_ids(&self) -> BTreeSet<u64> {
        self.0.all().iter().map(|id| id.into_inner()).collect()
    }

    pub fn used_in_objective(&self) -> BTreeSet<u64> {
        self.0
            .used_in_objective()
            .iter()
            .map(|id| id.into_inner())
            .collect()
    }

    pub fn used_in_constraints(&self) -> BTreeMap<u64, BTreeSet<u64>> {
        self.0
            .used_in_constraints()
            .iter()
            .map(|(constraint_id, variable_ids)| {
                (
                    **constraint_id,
                    variable_ids.iter().map(|id| id.into_inner()).collect(),
                )
            })
            .collect()
    }

    pub fn fixed(&self) -> BTreeMap<u64, f64> {
        self.0
            .fixed()
            .iter()
            .map(|(id, value)| (id.into_inner(), *value))
            .collect()
    }

    pub fn irrelevant(&self) -> BTreeSet<u64> {
        self.0
            .irrelevant()
            .keys()
            .map(|id| id.into_inner())
            .collect()
    }

    pub fn dependent(&self) -> BTreeSet<u64> {
        self.0
            .dependent()
            .keys()
            .map(|id| id.into_inner())
            .collect()
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

    pub fn with_parameters(&self, parameters: &Parameters) -> Result<Instance> {
        let instance = self
            .0
            .clone()
            .with_parameters(parameters.0.clone(), ommx::ATol::default())?;
        let parsed = Parse::parse(instance, &())?;
        Ok(Instance(parsed))
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
