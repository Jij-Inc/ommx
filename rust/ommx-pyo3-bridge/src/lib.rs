#![doc = include_str!("../README.md")]

use ommx::Message as _;
use pyo3::{
    exceptions::PyImportError,
    prelude::*,
    types::{PyAny, PyBytes},
};
use pyo3_stub_gen::{PyStubType, TypeInfo};

const FUNCTION_ENDPOINT: &str = "_pyo3_bridge_function_from_bytes";
const CONSTRAINT_ENDPOINT: &str = "_pyo3_bridge_constraint_from_bytes";
const DECISION_VARIABLE_ENDPOINT: &str = "_pyo3_bridge_decision_variable_from_bytes";

fn incompatible_python_ommx(capability: &str, source: PyErr) -> PyErr {
    PyImportError::new_err(format!(
        "the installed Python OMMX package does not provide the required bridge capability \
         `{capability}`; install a Python OMMX release compatible with this \
         `ommx-pyo3-bridge` crate ({source})"
    ))
}

fn bridge_endpoint<'py>(py: Python<'py>, endpoint: &str) -> PyResult<Bound<'py, PyAny>> {
    let module = py
        .import("ommx._ommx_rust")
        .map_err(|error| incompatible_python_ommx(endpoint, error))?;
    module
        .getattr(endpoint)
        .map_err(|error| incompatible_python_ommx(endpoint, error))
}

fn instance_from_v2_bytes<'py>(py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
    let capability = "ommx.Instance.from_v2_bytes";
    let module = py
        .import("ommx")
        .map_err(|error| incompatible_python_ommx(capability, error))?;
    let instance = module
        .getattr("Instance")
        .map_err(|error| incompatible_python_ommx(capability, error))?;
    instance
        .getattr("from_v2_bytes")
        .map_err(|error| incompatible_python_ommx(capability, error))
}

/// Output wrapper converting a Rust [`ommx::Function`] into `ommx.Function`.
///
/// The function is intrinsic data and needs no owner-side context.
#[derive(Debug, Clone)]
pub struct PyFunction(ommx::Function);

impl PyFunction {
    /// Create a Python output wrapper for `function`.
    pub fn new(function: ommx::Function) -> Self {
        Self(function)
    }
}

impl From<ommx::Function> for PyFunction {
    fn from(function: ommx::Function) -> Self {
        Self::new(function)
    }
}

impl<'py> IntoPyObject<'py> for PyFunction {
    type Target = PyAny;
    type Output = Bound<'py, PyAny>;
    type Error = PyErr;

    fn into_pyobject(self, py: Python<'py>) -> PyResult<Self::Output> {
        let bytes = self.0.to_bytes();
        bridge_endpoint(py, FUNCTION_ENDPOINT)?.call1((PyBytes::new(py, &bytes),))
    }
}

impl PyStubType for PyFunction {
    fn type_output() -> TypeInfo {
        TypeInfo::with_module("ommx.Function", "ommx".into())
    }
}

/// Output wrapper converting a detached Rust constraint into `ommx.Constraint`.
///
/// A detached constraint consists of its intrinsic row and its complete
/// [`ommx::ConstraintContext`], including its modeling label and provenance.
/// Its collection-owned constraint ID is intentionally not part of this type.
#[derive(Debug, Clone)]
pub struct PyConstraint {
    constraint: ommx::Constraint,
    context: ommx::ConstraintContext,
}

impl PyConstraint {
    /// Create a Python output wrapper from the complete detached constraint.
    pub fn new(constraint: ommx::Constraint, context: ommx::ConstraintContext) -> Self {
        Self {
            constraint,
            context,
        }
    }
}

impl From<(ommx::Constraint, ommx::ConstraintContext)> for PyConstraint {
    fn from((constraint, context): (ommx::Constraint, ommx::ConstraintContext)) -> Self {
        Self::new(constraint, context)
    }
}

impl<'py> IntoPyObject<'py> for PyConstraint {
    type Target = PyAny;
    type Output = Bound<'py, PyAny>;
    type Error = PyErr;

    fn into_pyobject(self, py: Python<'py>) -> PyResult<Self::Output> {
        let constraint = ommx::v2::RegularConstraint::from(self.constraint).encode_to_vec();
        let context = ommx::v2::ConstraintContext::from(self.context).encode_to_vec();
        bridge_endpoint(py, CONSTRAINT_ENDPOINT)?
            .call1((PyBytes::new(py, &constraint), PyBytes::new(py, &context)))
    }
}

impl PyStubType for PyConstraint {
    fn type_output() -> TypeInfo {
        TypeInfo::with_module("ommx.Constraint", "ommx".into())
    }
}

/// Output wrapper converting a detached Rust decision variable into
/// `ommx.DecisionVariable`.
///
/// The variable ID and modeling label are supplied explicitly because they
/// are owned by an enclosing decision-variable table in the Rust SDK. Fixed
/// values remain instance-owned and are intentionally not transferred.
#[derive(Debug, Clone)]
pub struct PyDecisionVariable {
    id: ommx::VariableID,
    decision_variable: ommx::DecisionVariable,
    label: ommx::ModelingLabel,
}

impl PyDecisionVariable {
    /// Create a Python output wrapper from the complete detached variable.
    pub fn new(
        id: ommx::VariableID,
        decision_variable: ommx::DecisionVariable,
        label: ommx::ModelingLabel,
    ) -> Self {
        Self {
            id,
            decision_variable,
            label,
        }
    }
}

impl
    From<(
        ommx::VariableID,
        ommx::DecisionVariable,
        ommx::ModelingLabel,
    )> for PyDecisionVariable
{
    fn from(
        (id, decision_variable, label): (
            ommx::VariableID,
            ommx::DecisionVariable,
            ommx::ModelingLabel,
        ),
    ) -> Self {
        Self::new(id, decision_variable, label)
    }
}

impl<'py> IntoPyObject<'py> for PyDecisionVariable {
    type Target = PyAny;
    type Output = Bound<'py, PyAny>;
    type Error = PyErr;

    fn into_pyobject(self, py: Python<'py>) -> PyResult<Self::Output> {
        let id: u64 = self.id.into();
        let decision_variable =
            ommx::v2::DecisionVariable::from(self.decision_variable).encode_to_vec();
        let label = ommx::v2::ModelingLabel::from(self.label).encode_to_vec();
        bridge_endpoint(py, DECISION_VARIABLE_ENDPOINT)?.call1((
            id,
            PyBytes::new(py, &decision_variable),
            PyBytes::new(py, &label),
        ))
    }
}

impl PyStubType for PyDecisionVariable {
    fn type_output() -> TypeInfo {
        TypeInfo::with_module("ommx.DecisionVariable", "ommx".into())
    }
}

/// Output wrapper converting a Rust [`ommx::Instance`] into `ommx.Instance`.
///
/// Instances already have a public, owner-complete v2 root serialization, so
/// this wrapper uses `ommx.Instance.from_v2_bytes` rather than a component-only
/// reconstruction endpoint.
#[derive(Debug, Clone)]
pub struct PyInstance(ommx::Instance);

impl PyInstance {
    /// Create a Python output wrapper for `instance`.
    pub fn new(instance: ommx::Instance) -> Self {
        Self(instance)
    }
}

impl From<ommx::Instance> for PyInstance {
    fn from(instance: ommx::Instance) -> Self {
        Self::new(instance)
    }
}

impl<'py> IntoPyObject<'py> for PyInstance {
    type Target = PyAny;
    type Output = Bound<'py, PyAny>;
    type Error = PyErr;

    fn into_pyobject(self, py: Python<'py>) -> PyResult<Self::Output> {
        let bytes = self.0.to_v2_bytes();
        instance_from_v2_bytes(py)?.call1((PyBytes::new(py, &bytes),))
    }
}

impl PyStubType for PyInstance {
    fn type_output() -> TypeInfo {
        TypeInfo::with_module("ommx.Instance", "ommx".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stub_types_are_canonical_top_level_ommx_classes() {
        assert_eq!(PyFunction::type_output().name, "ommx.Function");
        assert_eq!(PyConstraint::type_output().name, "ommx.Constraint");
        assert_eq!(
            PyDecisionVariable::type_output().name,
            "ommx.DecisionVariable"
        );
        assert_eq!(PyInstance::type_output().name, "ommx.Instance");
    }
}
