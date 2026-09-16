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
    (ProtobufV1ReceiverConfig, $value:expr) => {
        ProtobufV1ReceiverConfig {
            function: |py, _| $value.into_py_any(py),
            constraint: |py, _| $value.into_py_any(py),
            decision_variable: |py, _| $value.into_py_any(py),
            instance: |py, _| $value.into_py_any(py),
            parametric_instance: |py, _| $value.into_py_any(py),
            solution: |py, _| $value.into_py_any(py),
            sample_set: |py, _| $value.into_py_any(py),
        }
    };
    (ProtobufV2ReceiverConfig, $value:expr) => {
        ProtobufV2ReceiverConfig {
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

        // Payload decoding is the configured factory's responsibility.
        let function = b"function";
        let instance_v1 = b"instance v1";
        let instance_v2 = b"instance v2";
        for (module, v1_value, v2_value) in [(&first, 1, 2), (&second, 3, 4)] {
            for (endpoint, bytes, expected) in [
                (protocol::V1_FUNCTION, function.as_slice(), v1_value),
                (protocol::V2_FUNCTION, function.as_slice(), v2_value),
                (protocol::V1_INSTANCE, instance_v1.as_slice(), v1_value),
                (protocol::V2_INSTANCE, instance_v2.as_slice(), v2_value),
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
fn wire_arguments_reach_factories_unchanged() {
    Python::initialize();
    Python::attach(|py| {
        fn bytes(py: Python<'_>, value: &[u8]) -> PyResult<Py<PyAny>> {
            Ok(PyBytes::new(py, value).into_any().unbind())
        }
        let first = ProtobufV1ReceiverConfig {
            function: bytes,
            constraint: bytes,
            decision_variable: bytes,
            instance: bytes,
            parametric_instance: bytes,
            solution: bytes,
            sample_set: bytes,
        };
        let second = ProtobufV2ReceiverConfig {
            function: bytes,
            constraint: |py, value, context| {
                (PyBytes::new(py, value), PyBytes::new(py, context)).into_py_any(py)
            },
            decision_variable: |py, id, value, label| {
                (id, PyBytes::new(py, value), PyBytes::new(py, label)).into_py_any(py)
            },
            instance: bytes,
            parametric_instance: bytes,
            solution: bytes,
            sample_set: bytes,
        };
        let module = PyModule::new(py, "sdk").unwrap();
        register(&module, [first.into(), second.into()]).unwrap();
        // Include unknown/invalid bytes: the bridge must not parse, normalize,
        // or discard any part of a payload before the SDK sees it.
        let payload = PyBytes::new(py, b"\x00\xfforiginal payload");
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
                let endpoint = format!("_bridge_protobuf_v{version}_{kind}_from_bytes");
                let args = match (version, kind) {
                    (2, "constraint") => (&payload, PyBytes::new(py, b"context\xff"))
                        .into_pyobject(py)
                        .unwrap()
                        .into_any(),
                    (2, "decision_variable") => {
                        (u64::MAX, &payload, PyBytes::new(py, b"label\xff"))
                            .into_pyobject(py)
                            .unwrap()
                            .into_any()
                    }
                    _ => (&payload,).into_pyobject(py).unwrap().into_any(),
                };
                let actual = module
                    .getattr(endpoint)
                    .unwrap()
                    .call(args.cast::<pyo3::types::PyTuple>().unwrap(), None)
                    .unwrap();
                let expected = if args.len().unwrap() == 1 {
                    payload.clone().into_any()
                } else {
                    args
                };
                assert!(actual.eq(expected).unwrap());
            }
        }
    });
}

#[test]
fn payload_validation_belongs_to_each_sdk_factory() {
    Python::initialize();
    Python::attach(|py| {
        let first = v1();
        let mut second = v2();
        second.instance = |_, bytes| {
            assert_eq!(bytes, b"\xff");
            Err(TestBridgeError::new_err("unsupported payload"))
        };
        let module = PyModule::new(py, "sdk").unwrap();
        register(&module, [first.into(), second.into()]).unwrap();
        let bytes = PyBytes::new(py, b"\xff");
        assert_eq!(
            module
                .getattr(protocol::V1_INSTANCE)
                .unwrap()
                .call1((&bytes,))
                .unwrap()
                .extract::<i32>()
                .unwrap(),
            1
        );
        let error = module
            .getattr(protocol::V2_INSTANCE)
            .unwrap()
            .call1((&bytes,))
            .unwrap_err();
        assert!(error.is_instance_of::<TestBridgeError>(py));
        assert!(error.to_string().contains("unsupported payload"));
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

        for (endpoint, bytes) in [
            (protocol::V1_FUNCTION, b"function".as_slice()),
            (protocol::V2_FUNCTION, b"function".as_slice()),
            (protocol::V1_INSTANCE, b"instance v1".as_slice()),
            (protocol::V2_INSTANCE, b"instance v2".as_slice()),
        ] {
            let error = module
                .getattr(endpoint)
                .unwrap()
                .call1((PyBytes::new(py, bytes),))
                .unwrap_err();
            assert!(error.is_instance_of::<PyValueError>(py));
            assert_eq!(error.to_string(), "ValueError: constructor failed");
        }
    });
}
