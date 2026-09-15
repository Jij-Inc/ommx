use super::*;
use pyo3::{exceptions::PyValueError, types::PyBytes, IntoPyObjectExt};

pyo3::create_exception!(sdk, TestBridgeError, pyo3::exceptions::PyRuntimeError);

fn register(
    module: &Bound<'_, PyModule>,
    configs: impl IntoIterator<Item = ReceiverConfig>,
) -> PyResult<()> {
    super::register_receivers(module, &module.py().get_type::<TestBridgeError>(), configs)
}

macro_rules! config {
    ($config:ident, $value:expr) => {
        $config {
            function: |py, _| $value.into_py_any(py),
            constraint: |py, _, _| $value.into_py_any(py),
            decision_variable: |py, _, _, _| $value.into_py_any(py),
            instance: |py, _| $value.into_py_any(py),
            parametric_instance: |py, _| $value.into_py_any(py),
            solution: |py, _| $value.into_py_any(py),
            sample_set: |py, _| $value.into_py_any(py),
        }
    };
}

fn v1() -> ProtobufV1ReceiverConfig {
    config!(ProtobufV1ReceiverConfig, 1)
}

fn v2() -> ProtobufV2ReceiverConfig {
    config!(ProtobufV2ReceiverConfig, 2)
}

fn advertised(module: &Bound<'_, PyModule>) -> Vec<u32> {
    module
        .getattr(protocol::SUPPORTED_PROTOCOLS)
        .unwrap()
        .call0()
        .unwrap()
        .extract()
        .unwrap()
}

#[test]
fn advertisement_matches_registered_endpoints_without_public_classes() {
    Python::initialize();
    Python::attach(|py| {
        for (configs, expected) in [
            (vec![], vec![]),
            (vec![v1().into()], vec![1]),
            (vec![v2().into()], vec![2]),
            (vec![v2().into(), v1().into()], vec![2, 1]),
            (vec![v1().into(), v2().into()], vec![1, 2]),
        ] {
            // Registration needs no Python classes or public byte decoders.
            let module = PyModule::new(py, "sdk").unwrap();
            register(&module, configs).unwrap();
            assert_eq!(advertised(&module), expected);
            for version in [1, 2] {
                for kind in [
                    "function",
                    "constraint",
                    "decision_variable",
                    "instance",
                    "parametric_instance",
                    "solution",
                    "sample_set",
                ] {
                    let name = format!("_bridge_protobuf_v{version}_{kind}_from_bytes");
                    if expected.contains(&version) {
                        assert!(module.getattr(name).unwrap().is_callable());
                    } else {
                        assert!(!module.hasattr(name).unwrap());
                    }
                }
            }
            assert!(!module.hasattr("Instance").unwrap());
            assert!(!module.hasattr("Receiver").unwrap());
            assert!(!module.hasattr("ProtocolDeclaration").unwrap());
        }
    });
}

#[test]
fn duplicate_protocols_are_rejected_before_registration() {
    Python::initialize();
    Python::attach(|py| {
        for configs in [
            vec![v1().into(), v2().into(), v1().into()],
            vec![v2().into(), v1().into(), v2().into()],
        ] {
            let module = PyModule::new(py, "sdk").unwrap();
            let before = module.dict().copy().unwrap();
            let error = register(&module, configs).unwrap_err();
            assert!(error.get_type(py).is(py.get_type::<TestBridgeError>()));
            assert!(error.to_string().contains("repeats ProtobufV"));
            assert!(module.dict().eq(before).unwrap());
        }
    });
}

#[test]
fn invalid_exception_type_does_not_publish_receivers() {
    Python::initialize();
    Python::attach(|py| {
        let module = PyModule::new(py, "sdk").unwrap();
        let before = module.dict().copy().unwrap();
        let error =
            super::register_receivers(&module, &py.get_type::<pyo3::types::PyInt>(), [v1().into()])
                .unwrap_err();
        assert!(error.is_instance_of::<pyo3::exceptions::PyTypeError>(py));
        assert!(module.dict().eq(before).unwrap());
    });
}

#[test]
fn occupied_endpoint_does_not_publish_partial_support() {
    Python::initialize();
    Python::attach(|py| {
        let module = PyModule::new(py, "sdk").unwrap();
        module.add(protocol::V2_SAMPLE_SET, 123).unwrap();
        let before = module.dict().copy().unwrap();
        let error = register(&module, [v1().into(), v2().into()]).unwrap_err();
        assert!(error.get_type(py).is(py.get_type::<TestBridgeError>()));
        assert!(error.to_string().contains(protocol::V2_SAMPLE_SET));
        assert!(module.dict().eq(before).unwrap());
    });
}

#[test]
fn repeated_registration_preserves_the_original_receivers() {
    Python::initialize();
    Python::attach(|py| {
        let module = PyModule::new(py, "sdk").unwrap();
        register(&module, [v1().into()]).unwrap();
        let before = module.dict().copy().unwrap();
        let error = register(&module, [v2().into()]).unwrap_err();
        assert!(error.get_type(py).is(py.get_type::<TestBridgeError>()));
        assert!(error.to_string().contains("already registered"));
        assert!(module.dict().eq(before).unwrap());
    });
}

#[test]
fn factories_are_local_to_each_protocol_and_module() {
    Python::initialize();
    Python::attach(|py| {
        let first = PyModule::new(py, "first").unwrap();
        register(&first, [v1().into(), v2().into()]).unwrap();
        let second = PyModule::new(py, "second").unwrap();
        register(
            &second,
            [
                config!(ProtobufV1ReceiverConfig, 3).into(),
                config!(ProtobufV2ReceiverConfig, 4).into(),
            ],
        )
        .unwrap();

        let function = ommx::Function::default().to_bytes();
        let instance = ommx::Instance::default();
        let instance_v1 = instance.to_v1_bytes().unwrap();
        let instance_v2 = instance.to_v2_bytes();
        for (module, v1_value, v2_value) in [(&first, 1, 2), (&second, 3, 4)] {
            for (endpoint, bytes, expected) in [
                (protocol::V1_FUNCTION, &function, v1_value),
                (protocol::V2_FUNCTION, &function, v2_value),
                (protocol::V1_INSTANCE, &instance_v1, v1_value),
                (protocol::V2_INSTANCE, &instance_v2, v2_value),
            ] {
                let actual: i32 = module
                    .getattr(endpoint)
                    .unwrap()
                    .call1((PyBytes::new(py, bytes),))
                    .unwrap()
                    .extract()
                    .unwrap();
                assert_eq!(actual, expected);
            }
            assert_eq!(advertised(module), vec![1, 2]);
        }
    });
}

#[test]
fn invalid_payload_is_rejected_before_calling_the_factory() {
    Python::initialize();
    Python::attach(|py| {
        // Returning ValueError would reveal an accidental factory call.
        let mut first = v1();
        let mut second = v2();
        first.function = |_, _| Err(PyValueError::new_err("factory called"));
        second.function = first.function;
        first.instance = |_, _| Err(PyValueError::new_err("factory called"));
        second.instance = first.instance;
        let module = PyModule::new(py, "sdk").unwrap();
        register(&module, [first.into(), second.into()]).unwrap();
        for version in [1, 2] {
            for kind in [
                "function",
                "instance",
                "parametric_instance",
                "solution",
                "sample_set",
            ] {
                let endpoint = format!("_bridge_protobuf_v{version}_{kind}_from_bytes");
                let error = module
                    .getattr(endpoint)
                    .unwrap()
                    .call1((PyBytes::new(py, b"\xff"),))
                    .unwrap_err();
                assert!(error.get_type(py).is(py.get_type::<TestBridgeError>()));
                assert!(error
                    .to_string()
                    .contains(&format!("invalid OMMX ProtobufV{version} bridge payload")));
            }
        }
    });
}

#[test]
fn constructor_errors_keep_their_python_classification() {
    Python::initialize();
    Python::attach(|py| {
        let mut first = v1();
        let mut second = v2();
        first.function = |_, _| Err(PyValueError::new_err("constructor failed"));
        second.function = first.function;
        first.instance = |_, _| Err(PyValueError::new_err("constructor failed"));
        second.instance = first.instance;
        let module = PyModule::new(py, "sdk").unwrap();
        register(&module, [first.into(), second.into()]).unwrap();
        let instance = ommx::Instance::default();
        for (endpoint, bytes) in [
            (protocol::V1_FUNCTION, ommx::Function::default().to_bytes()),
            (protocol::V2_FUNCTION, ommx::Function::default().to_bytes()),
            (protocol::V1_INSTANCE, instance.to_v1_bytes().unwrap()),
            (protocol::V2_INSTANCE, instance.to_v2_bytes()),
        ] {
            let error = module
                .getattr(endpoint)
                .unwrap()
                .call1((PyBytes::new(py, &bytes),))
                .unwrap_err();
            assert!(error.is_instance_of::<PyValueError>(py));
            assert_eq!(error.to_string(), "ValueError: constructor failed");
        }
    });
}
