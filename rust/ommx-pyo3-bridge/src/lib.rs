#![doc = include_str!("../README.md")]

mod protocol;
mod receiver;
mod transfer;
pub use receiver::{
    register_receivers, ProtobufV1ReceiverConfig, ProtobufV2ReceiverConfig, ReceiverConfig,
};
pub use transfer::{
    resolve_target, Export, ProtobufV1, ProtobufV2, Target, TransferProtocol, TransferProtocolId,
    TransferVia,
};

use pyo3::{prelude::*, types::PyAny};
use pyo3_stub_gen::{PyStubType, TypeInfo};
use std::convert::Infallible;

macro_rules! output_wrapper {
    ($wrapper:ident, $python_name:literal) => {
        #[doc = concat!("A completed transfer of `ommx.", $python_name, "`.")]
        ///
        /// Construct through [`Target::transfer`]. The wrapper owns the Python
        /// object produced by the receiving SDK; returning it through PyO3
        /// performs no serialization or protocol selection.
        #[derive(Debug)]
        pub struct $wrapper(Py<PyAny>);

        impl<'py> IntoPyObject<'py> for $wrapper {
            type Target = PyAny;
            type Output = Bound<'py, PyAny>;
            type Error = Infallible;

            fn into_pyobject(self, py: Python<'py>) -> Result<Self::Output, Infallible> {
                Ok(self.0.into_bound(py))
            }
        }

        impl PyStubType for $wrapper {
            fn type_output() -> TypeInfo {
                TypeInfo::with_module(concat!("ommx.", $python_name), "ommx".into())
            }
        }
    };
}

output_wrapper!(PyFunction, "Function");
output_wrapper!(PyConstraint, "Constraint");
output_wrapper!(PyDecisionVariable, "DecisionVariable");
output_wrapper!(PyInstance, "Instance");
output_wrapper!(PyParametricInstance, "ParametricInstance");
output_wrapper!(PySolution, "Solution");
output_wrapper!(PySampleSet, "SampleSet");

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
