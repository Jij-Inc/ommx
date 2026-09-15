//! Receiver registration owned by the bridge; SDKs supply Python constructors.

mod protobuf;

use crate::{protocol, TransferProtocolId};
use pyo3::{
    exceptions::{PyImportError, PyRuntimeError},
    prelude::*,
    types::PyBytes,
};

/// Configure the installed Python SDK's bridge receivers.
///
/// Factories run inside the receiving extension and must construct that SDK's
/// canonical Python classes, preserving all supplied domain data. They receive
/// parsed Rust values; protocol names, byte decoding, call signatures, and
/// bridge error conversion are owned by this crate. Python objects are opaque
/// here because their concrete classes belong to the receiving SDK.
///
/// [`Self::register`] checks the root decoding methods required by the selected
/// protocols before registering their component receivers and advertising
/// support. The module must already contain the SDK's four root classes.
pub struct ReceiverConfig {
    /// Protocols this SDK supports, in declaration order. Duplicates are removed.
    pub protocols: Vec<TransferProtocolId>,
    /// Construct the SDK's `ommx.Function` from a parsed function.
    pub function: fn(Python<'_>, ommx::Function) -> PyResult<Py<PyAny>>,
    /// Construct its detached `ommx.Constraint` with the complete context.
    pub constraint:
        fn(Python<'_>, ommx::Constraint, ommx::ConstraintContext) -> PyResult<Py<PyAny>>,
    /// Construct its detached `ommx.DecisionVariable` with its ID and label.
    pub decision_variable: fn(
        Python<'_>,
        ommx::VariableID,
        ommx::DecisionVariable,
        ommx::ModelingLabel,
    ) -> PyResult<Py<PyAny>>,
}

impl ReceiverConfig {
    /// Register private receivers on the SDK's extension module.
    ///
    /// Missing or non-callable root decoders raise `ImportError` before any
    /// bridge attributes are added. Factories and support declarations are local
    /// to this registration; no process-global receiver state is used.
    pub fn register(mut self, module: &Bound<'_, PyModule>) -> PyResult<()> {
        let mut protocols = Vec::new();
        for protocol in self.protocols {
            if !protocols.contains(&protocol) {
                protocols.push(protocol);
            }
        }
        self.protocols = protocols;
        let v1 = self.protocols.contains(&TransferProtocolId::ProtobufV1);
        let v2 = self.protocols.contains(&TransferProtocolId::ProtobufV2);
        for (required, method) in [(v1, protocol::FROM_V1_BYTES), (v2, protocol::FROM_V2_BYTES)] {
            if !required {
                continue;
            }
            for class in protocol::ROOT_CLASSES {
                let decoder = module
                    .getattr(class)
                    .and_then(|class| class.getattr(method))
                    .map_err(|source| {
                        let error = PyImportError::new_err(format!(
                            "OMMX bridge receiver requires {class}.{method}"
                        ));
                        error.set_cause(module.py(), Some(source));
                        error
                    })?;
                if !decoder.is_callable() {
                    return Err(PyImportError::new_err(format!(
                        "OMMX bridge receiver requires callable {class}.{method}"
                    )));
                }
            }
        }
        let receiver = Py::new(module.py(), Receiver { config: self })?.into_bound(module.py());
        // Prepare all bindings before publishing the declaration. Only the
        // bound methods are exported; the implementation class stays private.
        let mut bindings = Vec::new();
        macro_rules! bind {
            ($name:expr, $method:ident) => {
                bindings.push(($name, receiver.getattr(stringify!($method))?));
            };
        }
        if v2 {
            bind!(protocol::V2_FUNCTION, function_v2);
            bind!(protocol::V2_CONSTRAINT, constraint_v2);
            bind!(protocol::V2_DECISION_VARIABLE, decision_variable_v2);
        }
        if v1 {
            bind!(protocol::V1_FUNCTION, function_v1);
            bind!(protocol::V1_CONSTRAINT, constraint_v1);
            bind!(protocol::V1_DECISION_VARIABLE, decision_variable_v1);
        }
        bind!(protocol::SUPPORTED_PROTOCOLS, supported_protocols);
        for (name, method) in bindings {
            module.add(name, method)?;
        }
        Ok(())
    }
}

// This object and its factories belong to the receiving shared library. A
// sender only calls Python methods; it never extracts this Rust/PyO3 type.
#[pyclass(frozen, module = "ommx._ommx_rust")]
struct Receiver {
    config: ReceiverConfig,
}

fn v2_error(error: ommx::Error) -> PyErr {
    PyRuntimeError::new_err(format!("invalid OMMX ProtobufV2 bridge payload: {error:#}"))
}

fn v1_error(error: ommx::Error) -> PyErr {
    PyRuntimeError::new_err(format!("invalid OMMX ProtobufV1 bridge payload: {error:#}"))
}

#[pymethods]
impl Receiver {
    fn supported_protocols(&self) -> Vec<u32> {
        self.config.protocols.iter().map(|id| *id as u32).collect()
    }

    fn function_v2(&self, bytes: &Bound<'_, PyBytes>) -> PyResult<Py<PyAny>> {
        let function = protobuf::function(bytes.as_bytes()).map_err(v2_error)?;
        (self.config.function)(bytes.py(), function)
    }

    fn constraint_v2(
        &self,
        constraint: &Bound<'_, PyBytes>,
        context: &Bound<'_, PyBytes>,
    ) -> PyResult<Py<PyAny>> {
        let (value, context) =
            protobuf::constraint_v2(constraint.as_bytes(), context.as_bytes()).map_err(v2_error)?;
        (self.config.constraint)(constraint.py(), value, context)
    }

    fn decision_variable_v2(
        &self,
        id: u64,
        decision_variable: &Bound<'_, PyBytes>,
        label: &Bound<'_, PyBytes>,
    ) -> PyResult<Py<PyAny>> {
        let (id, variable, label) =
            protobuf::decision_variable_v2(id, decision_variable.as_bytes(), label.as_bytes())
                .map_err(v2_error)?;
        (self.config.decision_variable)(decision_variable.py(), id, variable, label)
    }

    fn function_v1(&self, bytes: &Bound<'_, PyBytes>) -> PyResult<Py<PyAny>> {
        let function = protobuf::function(bytes.as_bytes()).map_err(v1_error)?;
        (self.config.function)(bytes.py(), function)
    }

    fn constraint_v1(&self, bytes: &Bound<'_, PyBytes>) -> PyResult<Py<PyAny>> {
        let (value, context) = protobuf::constraint_v1(bytes.as_bytes()).map_err(v1_error)?;
        (self.config.constraint)(bytes.py(), value, context)
    }

    fn decision_variable_v1(&self, bytes: &Bound<'_, PyBytes>) -> PyResult<Py<PyAny>> {
        let (id, variable, label) =
            protobuf::decision_variable_v1(bytes.as_bytes()).map_err(v1_error)?;
        (self.config.decision_variable)(bytes.py(), id, variable, label)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pyo3::{exceptions::PyValueError, IntoPyObjectExt};
    use TransferProtocolId::{ProtobufV1, ProtobufV2};

    #[pyfunction]
    fn root_decoder(_bytes: &Bound<'_, PyBytes>) {}

    fn sdk_module(py: Python<'_>) -> Bound<'_, PyModule> {
        let module = PyModule::new(py, "sdk").unwrap();
        for class in protocol::ROOT_CLASSES {
            let root = PyModule::new(py, class).unwrap();
            root.add("from_v1_bytes", wrap_pyfunction!(root_decoder, py).unwrap())
                .unwrap();
            root.add("from_v2_bytes", wrap_pyfunction!(root_decoder, py).unwrap())
                .unwrap();
            module.add(class, root).unwrap();
        }
        module
    }

    fn config(protocols: Vec<TransferProtocolId>) -> ReceiverConfig {
        ReceiverConfig {
            protocols,
            function: |py, _| 1.into_py_any(py),
            constraint: |py, _, _| Ok(py.None()),
            decision_variable: |py, _, _, _| Ok(py.None()),
        }
    }

    #[test]
    fn advertisement_matches_registered_endpoints() {
        Python::initialize();
        Python::attach(|py| {
            for (ids, expected, v1, v2) in [
                (vec![], vec![], false, false),
                (vec![ProtobufV1], vec![1], true, false),
                (vec![ProtobufV2], vec![2], false, true),
                (
                    vec![ProtobufV2, ProtobufV1, ProtobufV2],
                    vec![2, 1],
                    true,
                    true,
                ),
            ] {
                let module = sdk_module(py);
                config(ids).register(&module).unwrap();
                let advertised: Vec<u32> = module
                    .getattr("_bridge_supported_protocols")
                    .unwrap()
                    .call0()
                    .unwrap()
                    .extract()
                    .unwrap();
                assert_eq!(advertised, expected);
                for kind in ["function", "constraint", "decision_variable"] {
                    assert_eq!(
                        module
                            .hasattr(format!("_bridge_protobuf_v1_{kind}_from_bytes"))
                            .unwrap(),
                        v1
                    );
                    assert_eq!(
                        module
                            .hasattr(format!("_bridge_protobuf_v2_{kind}_from_bytes"))
                            .unwrap(),
                        v2
                    );
                }
                assert!(!module.hasattr("Receiver").unwrap());
            }
        });
    }

    #[test]
    fn invalid_root_decoder_does_not_publish_partial_support() {
        Python::initialize();
        Python::attach(|py| {
            for (id, method) in [(ProtobufV1, "from_v1_bytes"), (ProtobufV2, "from_v2_bytes")] {
                for class in protocol::ROOT_CLASSES {
                    for missing in [false, true] {
                        let module = sdk_module(py);
                        let root = module.getattr(class).unwrap();
                        if missing {
                            root.delattr(method).unwrap();
                        } else {
                            root.setattr(method, py.None()).unwrap();
                        }
                        let error = config(vec![id]).register(&module).unwrap_err();
                        assert!(error.is_instance_of::<PyImportError>(py));
                        assert!(error.to_string().contains(&format!("{class}.{method}")));
                        assert!(!module.hasattr("_bridge_supported_protocols").unwrap());
                        assert!(!module
                            .hasattr("_bridge_protobuf_v1_function_from_bytes")
                            .unwrap());
                        assert!(!module
                            .hasattr("_bridge_protobuf_v2_function_from_bytes")
                            .unwrap());
                    }
                }
            }
        });
    }

    #[test]
    fn receiver_configuration_is_local_to_each_module() {
        Python::initialize();
        Python::attach(|py| {
            let first = sdk_module(py);
            config(vec![ProtobufV1]).register(&first).unwrap();
            let second = sdk_module(py);
            let mut other = config(vec![ProtobufV2]);
            other.function = |py, _| 2.into_py_any(py);
            other.register(&second).unwrap();
            let bytes = ommx::Function::default().to_bytes();
            let bytes = PyBytes::new(py, &bytes);
            let first_value: i32 = first
                .getattr(protocol::V1_FUNCTION)
                .unwrap()
                .call1((&bytes,))
                .unwrap()
                .extract()
                .unwrap();
            let second_value: i32 = second
                .getattr(protocol::V2_FUNCTION)
                .unwrap()
                .call1((&bytes,))
                .unwrap()
                .extract()
                .unwrap();
            assert_eq!((first_value, second_value), (1, 2));
            assert_eq!(
                first
                    .getattr(protocol::SUPPORTED_PROTOCOLS)
                    .unwrap()
                    .call0()
                    .unwrap()
                    .extract::<Vec<u32>>()
                    .unwrap(),
                vec![1]
            );
        });
    }

    #[test]
    fn invalid_payload_is_a_bridge_runtime_error() {
        Python::initialize();
        Python::attach(|py| {
            let module = sdk_module(py);
            config(vec![ProtobufV2]).register(&module).unwrap();
            let error = module
                .getattr(protocol::V2_FUNCTION)
                .unwrap()
                .call1((PyBytes::new(py, b"\xff"),))
                .unwrap_err();
            assert!(error.is_instance_of::<PyRuntimeError>(py));
            assert!(error
                .to_string()
                .contains("invalid OMMX ProtobufV2 bridge payload"));
        });
    }

    #[test]
    fn constructor_errors_keep_their_python_classification() {
        Python::initialize();
        Python::attach(|py| {
            let module = sdk_module(py);
            let mut receiver = config(vec![ProtobufV1]);
            receiver.function = |_, _| Err(PyValueError::new_err("constructor failed"));
            receiver.register(&module).unwrap();
            let bytes = ommx::Function::default().to_bytes();
            let error = module
                .getattr(protocol::V1_FUNCTION)
                .unwrap()
                .call1((PyBytes::new(py, &bytes),))
                .unwrap_err();
            assert!(error.is_instance_of::<PyValueError>(py));
            assert_eq!(error.to_string(), "ValueError: constructor failed");
        });
    }
}
