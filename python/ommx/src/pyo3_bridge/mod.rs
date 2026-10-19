//! Parse bridge payloads with this SDK and construct its canonical Python classes.

mod protobuf;

use crate::{
    Constraint, DecisionVariable, Function, Instance, ParametricInstance, SampleSet, Solution,
};
use ommx_pyo3_bridge::{register_receivers, ProtobufV1ReceiverConfig, ProtobufV2ReceiverConfig};
use pyo3::{prelude::*, IntoPyObjectExt};

pyo3_stub_gen::create_exception!(
    ommx._ommx_rust,
    BridgeError,
    pyo3::exceptions::PyRuntimeError,
    "An OMMX bridge protocol, registration, or transfer failure after loading the SDK."
);

// Payload validity is owned by this SDK's parsers. The private bridge boundary
// classifies their failures as BridgeError; Python constructor errors pass through.
fn parse<T>(value: ommx::Result<T>, protocol: &str) -> PyResult<T> {
    value.map_err(|error| {
        BridgeError::new_err(format!("invalid OMMX {protocol} bridge payload: {error:#}"))
    })
}

fn v1() -> ProtobufV1ReceiverConfig {
    ProtobufV1ReceiverConfig {
        function: |py, bytes| {
            Function(parse(protobuf::function(bytes), "ProtobufV1")?).into_py_any(py)
        },
        constraint: |py, bytes| {
            let (value, context) = parse(protobuf::constraint_v1(bytes), "ProtobufV1")?;
            Constraint::from_parts(value, context).into_py_any(py)
        },
        decision_variable: |py, bytes| {
            let (id, value, label) = parse(protobuf::decision_variable_v1(bytes), "ProtobufV1")?;
            DecisionVariable::from_parts(id, value, label).into_py_any(py)
        },
        instance: |py, bytes| {
            Instance {
                inner: parse(ommx::Instance::from_v1_bytes(bytes), "ProtobufV1")?,
            }
            .into_py_any(py)
        },
        parametric_instance: |py, bytes| {
            ParametricInstance {
                inner: parse(ommx::ParametricInstance::from_v1_bytes(bytes), "ProtobufV1")?,
            }
            .into_py_any(py)
        },
        solution: |py, bytes| {
            Solution {
                inner: parse(ommx::Solution::from_v1_bytes(bytes), "ProtobufV1")?,
            }
            .into_py_any(py)
        },
        sample_set: |py, bytes| {
            SampleSet {
                inner: parse(ommx::SampleSet::from_v1_bytes(bytes), "ProtobufV1")?,
            }
            .into_py_any(py)
        },
    }
}

fn v2() -> ProtobufV2ReceiverConfig {
    ProtobufV2ReceiverConfig {
        function: |py, bytes| {
            Function(parse(protobuf::function(bytes), "ProtobufV2")?).into_py_any(py)
        },
        constraint: |py, constraint, context| {
            let (value, context) =
                parse(protobuf::constraint_v2(constraint, context), "ProtobufV2")?;
            Constraint::from_parts(value, context).into_py_any(py)
        },
        decision_variable: |py, id, variable, label| {
            let (id, value, label) = parse(
                protobuf::decision_variable_v2(id, variable, label),
                "ProtobufV2",
            )?;
            DecisionVariable::from_parts(id, value, label).into_py_any(py)
        },
        instance: |py, bytes| {
            Instance {
                inner: parse(ommx::Instance::from_v2_bytes(bytes), "ProtobufV2")?,
            }
            .into_py_any(py)
        },
        parametric_instance: |py, bytes| {
            ParametricInstance {
                inner: parse(ommx::ParametricInstance::from_v2_bytes(bytes), "ProtobufV2")?,
            }
            .into_py_any(py)
        },
        solution: |py, bytes| {
            Solution {
                inner: parse(ommx::Solution::from_v2_bytes(bytes), "ProtobufV2")?,
            }
            .into_py_any(py)
        },
        sample_set: |py, bytes| {
            SampleSet {
                inner: parse(ommx::SampleSet::from_v2_bytes(bytes), "ProtobufV2")?,
            }
            .into_py_any(py)
        },
    }
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    register_receivers(
        module,
        &module.py().get_type::<BridgeError>(),
        [v1().into(), v2().into()],
    )
}
