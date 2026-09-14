#![doc = include_str!("../README.md")]

mod protocol;
mod transfer;
pub use transfer::{
    resolve_target, Export, ProtobufV1, ProtobufV2, Target, TransferProtocol, TransferProtocolId,
    TransferVia,
};

use pyo3::{prelude::*, types::PyAny};
use pyo3_stub_gen::{PyStubType, TypeInfo};
use std::sync::Arc;

/// Legacy returns defer conversion; negotiated returns already own the Python
/// object. Cloning a completed output shares that object without acquiring the
/// GIL or repeating a transfer.
#[derive(Debug, Clone)]
enum Output<T> {
    Rust(T),
    Python(Arc<Py<PyAny>>),
}

impl<T> Output<T> {
    fn python(object: Bound<'_, PyAny>) -> Self {
        Self::Python(Arc::new(object.unbind()))
    }

    fn into_pyobject<'py>(
        self,
        py: Python<'py>,
        convert: impl FnOnce(T, Python<'py>) -> PyResult<Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        match self {
            Self::Rust(value) => convert(value, py),
            Self::Python(object) => Ok(object.bind(py).clone()),
        }
    }
}

/// Output wrapper converting a Rust [`ommx::Function`] into `ommx.Function`.
///
/// The function is intrinsic data and needs no owner-side context.
/// Values returned by [`Target::transfer`] already own the reconstructed Python
/// object and return it without another conversion.
#[derive(Debug, Clone)]
pub struct PyFunction(Output<ommx::Function>);

impl PyFunction {
    /// Create a Python output wrapper for `function`.
    pub fn new(function: ommx::Function) -> Self {
        Self(Output::Rust(function))
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
        self.0.into_pyobject(py, protocol::v0::function_into_py)
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
/// Values returned by [`Target::transfer`] already own the reconstructed Python
/// object and return it without another conversion.
#[derive(Debug, Clone)]
pub struct PyConstraint(Output<(ommx::Constraint, ommx::ConstraintContext)>);

impl PyConstraint {
    /// Create a Python output wrapper from the complete detached constraint.
    pub fn new(constraint: ommx::Constraint, context: ommx::ConstraintContext) -> Self {
        Self(Output::Rust((constraint, context)))
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
        self.0.into_pyobject(py, |(constraint, context), py| {
            protocol::v0::constraint_into_py(constraint, context, py)
        })
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
/// Values returned by [`Target::transfer`] already own the reconstructed Python
/// object and return it without another conversion.
#[derive(Debug, Clone)]
pub struct PyDecisionVariable(
    Output<(
        ommx::VariableID,
        ommx::DecisionVariable,
        ommx::ModelingLabel,
    )>,
);

impl PyDecisionVariable {
    /// Create a Python output wrapper from the complete detached variable.
    pub fn new(
        id: ommx::VariableID,
        decision_variable: ommx::DecisionVariable,
        label: ommx::ModelingLabel,
    ) -> Self {
        Self(Output::Rust((id, decision_variable, label)))
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
        self.0.into_pyobject(py, |(id, variable, label), py| {
            protocol::v0::decision_variable_into_py(id, variable, label, py)
        })
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
        ///
        /// Values returned by [`Target::transfer`] already own the reconstructed
        /// Python object and return it without another conversion.
        #[derive(Debug, Clone)]
        pub struct $wrapper(Output<$rust_type>);

        impl $wrapper {
            /// Create a Python output wrapper for the value.
            pub fn new(value: $rust_type) -> Self {
                Self(Output::Rust(value))
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
                self.0.into_pyobject(py, |value, py| {
                    protocol::v0::root_into_py(value.to_v2_bytes(), $python_name, py)
                })
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
