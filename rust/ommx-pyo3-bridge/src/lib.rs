#![doc = include_str!("../README.md")]

mod protocol;

use pyo3::{prelude::*, types::PyAny};
use pyo3_stub_gen::{PyStubType, TypeInfo};

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
        protocol::v0::function_into_py(self.0, py)
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
        protocol::v0::constraint_into_py(self.constraint, self.context, py)
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
        protocol::v0::decision_variable_into_py(self.id, self.decision_variable, self.label, py)
    }
}

impl PyStubType for PyDecisionVariable {
    fn type_output() -> TypeInfo {
        TypeInfo::with_module("ommx.DecisionVariable", "ommx".into())
    }
}

macro_rules! root_wrapper {
    ($wrapper:ident, $rust_type:ty, $python_name:literal) => {
        #[doc = concat!("Output wrapper converting a Rust [`", stringify!($rust_type), "`] into `ommx.", $python_name, "`.")]
        #[derive(Debug, Clone)]
        pub struct $wrapper($rust_type);

        impl $wrapper {
            /// Create a Python output wrapper for the value.
            pub fn new(value: $rust_type) -> Self {
                Self(value)
            }
        }

        impl From<$rust_type> for $wrapper {
            fn from(value: $rust_type) -> Self {
                Self::new(value)
            }
        }

        impl<'py> IntoPyObject<'py> for $wrapper {
            type Target = PyAny;
            type Output = Bound<'py, PyAny>;
            type Error = PyErr;

            fn into_pyobject(self, py: Python<'py>) -> PyResult<Self::Output> {
                protocol::v0::root_into_py(self.0.to_v2_bytes(), $python_name, py)
            }
        }

        impl PyStubType for $wrapper {
            fn type_output() -> TypeInfo {
                TypeInfo::with_module(concat!("ommx.", $python_name), "ommx".into())
            }
        }
    };
}

root_wrapper!(PyInstance, ommx::Instance, "Instance");
root_wrapper!(
    PyParametricInstance,
    ommx::ParametricInstance,
    "ParametricInstance"
);
root_wrapper!(PySolution, ommx::Solution, "Solution");
root_wrapper!(PySampleSet, ommx::SampleSet, "SampleSet");

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
        assert_eq!(
            PyParametricInstance::type_output().name,
            "ommx.ParametricInstance"
        );
        assert_eq!(PySolution::type_output().name, "ommx.Solution");
        assert_eq!(PySampleSet::type_output().name, "ommx.SampleSet");
    }
}
