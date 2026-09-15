//! Configure the SDK-owned Python constructors for the bridge's receivers.

use crate::{
    Constraint, DecisionVariable, Function, Instance, ParametricInstance, SampleSet, Solution,
};
use ommx_pyo3_bridge::{register_receivers, ProtobufV1ReceiverConfig, ProtobufV2ReceiverConfig};
use pyo3::{prelude::*, IntoPyObjectExt};

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    // Both protobuf protocols construct the same SDK classes after parsing.
    // The configuration types independently define their factory signatures.
    macro_rules! config {
        ($config:ident) => {
            $config {
                function: |py, value| Function(value).into_py_any(py),
                constraint: |py, value, context| {
                    Constraint::from_parts(value, context).into_py_any(py)
                },
                decision_variable: |py, id, value, label| {
                    DecisionVariable::from_parts(id, value, label).into_py_any(py)
                },
                instance: |py, inner| Instance { inner }.into_py_any(py),
                parametric_instance: |py, inner| ParametricInstance { inner }.into_py_any(py),
                solution: |py, inner| Solution { inner }.into_py_any(py),
                sample_set: |py, inner| SampleSet { inner }.into_py_any(py),
            }
        };
    }
    register_receivers(
        module,
        [
            config!(ProtobufV1ReceiverConfig).into(),
            config!(ProtobufV2ReceiverConfig).into(),
        ],
    )
}
