//! Codecs for the two registered protobuf transfer contracts.

use super::*;
use crate::protocol;
use crate::{
    PyConstraint, PyDecisionVariable, PyFunction, PyInstance, PyParametricInstance, PySampleSet,
    PySolution,
};
use ommx::Message;
use pyo3::types::PyBytes;

fn private_receiver(module: &Bound<'_, PyModule>, endpoint: &str) -> PyResult<Py<PyAny>> {
    Ok(module.getattr("_ommx_rust")?.getattr(endpoint)?.unbind())
}

fn import_bytes<'py>(bytes: Vec<u8>, receiver: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    receiver.call1((PyBytes::new(receiver.py(), &bytes),))
}

macro_rules! root {
    ($wrapper:ident, $domain:ident, $name:literal, $v1:expr, $v2:expr) => {
        impl super::sealed::Type<ProtobufV1> for $wrapper {}
        impl super::sealed::Type<ProtobufV2> for $wrapper {}
        impl TransferVia<ProtobufV1> for $wrapper {
            type Payload = Vec<u8>;
            const PYTHON_NAME: &'static str = concat!("ommx.", $name);
            fn receiver(module: &Bound<'_, PyModule>) -> PyResult<Py<PyAny>> {
                private_receiver(module, $v1)
            }
            fn import(payload: Vec<u8>, receiver: &Bound<'_, PyAny>) -> PyResult<Self> {
                import_bytes(payload, receiver).map(|object| Self(object.unbind()))
            }
        }
        impl TransferVia<ProtobufV2> for $wrapper {
            type Payload = Vec<u8>;
            const PYTHON_NAME: &'static str = concat!("ommx.", $name);
            fn receiver(module: &Bound<'_, PyModule>) -> PyResult<Py<PyAny>> {
                private_receiver(module, $v2)
            }
            fn import(payload: Vec<u8>, receiver: &Bound<'_, PyAny>) -> PyResult<Self> {
                import_bytes(payload, receiver).map(|object| Self(object.unbind()))
            }
        }
        impl Export<ProtobufV1, $wrapper> for ommx::v1::$domain {
            fn export(self) -> ommx::Result<Vec<u8>> {
                Ok(self.encode_to_vec())
            }
        }
        impl Export<ProtobufV2, $wrapper> for ommx::$domain {
            fn export(self) -> ommx::Result<Vec<u8>> {
                Ok(self.to_v2_bytes())
            }
        }
    };
}
root!(
    PyInstance,
    Instance,
    "Instance",
    protocol::V1_INSTANCE,
    protocol::V2_INSTANCE
);
root!(
    PyParametricInstance,
    ParametricInstance,
    "ParametricInstance",
    protocol::V1_PARAMETRIC_INSTANCE,
    protocol::V2_PARAMETRIC_INSTANCE
);
root!(
    PySolution,
    Solution,
    "Solution",
    protocol::V1_SOLUTION,
    protocol::V2_SOLUTION
);
root!(
    PySampleSet,
    SampleSet,
    "SampleSet",
    protocol::V1_SAMPLE_SET,
    protocol::V2_SAMPLE_SET
);

// Only use checked v1 domain conversions. Solution/SampleSet's legacy `From`
// conversions intentionally discard special-constraint results; those roots
// require an explicitly constructed v1 source when selecting ProtobufV1.
macro_rules! checked_v1_root {
    ($wrapper:ident, $domain:ident) => {
        impl Export<ProtobufV1, $wrapper> for ommx::$domain {
            fn export(self) -> ommx::Result<Vec<u8>> {
                self.to_v1_bytes()
            }
        }
    };
}
checked_v1_root!(PyInstance, Instance);
checked_v1_root!(PyParametricInstance, ParametricInstance);

macro_rules! v1_component {
    ($wrapper:ident, $name:literal, $endpoint:expr, $wire:ident) => {
        impl super::sealed::Type<ProtobufV1> for $wrapper {}
        impl super::sealed::Type<ProtobufV2> for $wrapper {}
        impl TransferVia<ProtobufV1> for $wrapper {
            type Payload = Vec<u8>;
            const PYTHON_NAME: &'static str = concat!("ommx.", $name);
            fn receiver(module: &Bound<'_, PyModule>) -> PyResult<Py<PyAny>> {
                private_receiver(module, $endpoint)
            }
            fn import(payload: Vec<u8>, receiver: &Bound<'_, PyAny>) -> PyResult<Self> {
                import_bytes(payload, receiver).map(|object| Self(object.unbind()))
            }
        }
        impl Export<ProtobufV1, $wrapper> for ommx::v1::$wire {
            fn export(self) -> ommx::Result<Vec<u8>> {
                Ok(self.encode_to_vec())
            }
        }
    };
}
v1_component!(PyFunction, "Function", protocol::V1_FUNCTION, Function);
v1_component!(
    PyConstraint,
    "Constraint",
    protocol::V1_CONSTRAINT,
    Constraint
);
v1_component!(
    PyDecisionVariable,
    "DecisionVariable",
    protocol::V1_DECISION_VARIABLE,
    DecisionVariable
);

impl Export<ProtobufV1, PyFunction> for ommx::Function {
    fn export(self) -> ommx::Result<Vec<u8>> {
        Ok(self.to_bytes())
    }
}
impl Export<ProtobufV1, PyDecisionVariable>
    for (
        ommx::VariableID,
        ommx::DecisionVariable,
        ommx::ModelingLabel,
    )
{
    fn export(self) -> ommx::Result<Vec<u8>> {
        let (id, variable, label) = self;
        let mut message = ommx::v1::DecisionVariable::default();
        message.id = id.into();
        message.kind = variable.kind().into();
        message.bound = Some(variable.bound().into());
        message.name = label.name;
        message.subscripts = label.subscripts;
        message.parameters = label.parameters.into_iter().collect();
        message.description = label.description;
        Ok(message.encode_to_vec())
    }
}

impl TransferVia<ProtobufV2> for PyFunction {
    type Payload = Vec<u8>;
    const PYTHON_NAME: &'static str = "ommx.Function";
    fn receiver(module: &Bound<'_, PyModule>) -> PyResult<Py<PyAny>> {
        private_receiver(module, protocol::V2_FUNCTION)
    }
    fn import(payload: Vec<u8>, receiver: &Bound<'_, PyAny>) -> PyResult<Self> {
        import_bytes(payload, receiver).map(|object| Self(object.unbind()))
    }
}
impl Export<ProtobufV2, PyFunction> for ommx::Function {
    fn export(self) -> ommx::Result<Vec<u8>> {
        Ok(self.to_bytes())
    }
}

impl TransferVia<ProtobufV2> for PyConstraint {
    type Payload = (Vec<u8>, Vec<u8>);
    const PYTHON_NAME: &'static str = "ommx.Constraint";
    fn receiver(module: &Bound<'_, PyModule>) -> PyResult<Py<PyAny>> {
        private_receiver(module, protocol::V2_CONSTRAINT)
    }
    fn import((constraint, context): Self::Payload, receiver: &Bound<'_, PyAny>) -> PyResult<Self> {
        let py = receiver.py();
        let object = receiver.call1((PyBytes::new(py, &constraint), PyBytes::new(py, &context)))?;
        Ok(Self(object.unbind()))
    }
}
impl Export<ProtobufV2, PyConstraint> for (ommx::Constraint, ommx::ConstraintContext) {
    fn export(self) -> ommx::Result<(Vec<u8>, Vec<u8>)> {
        Ok((
            ommx::v2::RegularConstraint::from(self.0).encode_to_vec(),
            ommx::v2::ConstraintContext::from(self.1).encode_to_vec(),
        ))
    }
}

impl TransferVia<ProtobufV2> for PyDecisionVariable {
    type Payload = (u64, Vec<u8>, Vec<u8>);
    const PYTHON_NAME: &'static str = "ommx.DecisionVariable";
    fn receiver(module: &Bound<'_, PyModule>) -> PyResult<Py<PyAny>> {
        private_receiver(module, protocol::V2_DECISION_VARIABLE)
    }
    fn import((id, variable, label): Self::Payload, receiver: &Bound<'_, PyAny>) -> PyResult<Self> {
        let py = receiver.py();
        let object = receiver.call1((id, PyBytes::new(py, &variable), PyBytes::new(py, &label)))?;
        Ok(Self(object.unbind()))
    }
}
impl Export<ProtobufV2, PyDecisionVariable>
    for (
        ommx::VariableID,
        ommx::DecisionVariable,
        ommx::ModelingLabel,
    )
{
    fn export(self) -> ommx::Result<(u64, Vec<u8>, Vec<u8>)> {
        Ok((
            self.0.into(),
            ommx::v2::DecisionVariable::from(self.1).encode_to_vec(),
            ommx::v2::ModelingLabel::from(self.2).encode_to_vec(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ommx::Parse as _;
    use proptest::{prelude::*, string::string_regex};

    fn export<T: TransferVia<ProtobufV2>>(value: impl Export<ProtobufV2, T>) -> T::Payload {
        value.export().unwrap()
    }

    fn deeply_composed_function(operation_count: usize) -> ommx::Function {
        (0..operation_count).fold(ommx::Function::from(ommx::linear!(1)), |function, level| {
            if level % 2 == 0 {
                function.abs()
            } else {
                function.signum()
            }
        })
    }

    fn arbitrary_function() -> impl Strategy<Value = ommx::Function> {
        ommx::Function::arbitrary()
    }

    fn arbitrary_constraint() -> impl Strategy<Value = ommx::Constraint> {
        (arbitrary_function(), any::<ommx::Equality>()).prop_map(|(function, equality)| {
            match equality {
                ommx::Equality::EqualToZero => ommx::Constraint::equal_to_zero(function),
                ommx::Equality::LessThanOrEqualToZero => {
                    ommx::Constraint::less_than_or_equal_to_zero(function)
                }
            }
        })
    }

    fn short_string() -> impl Strategy<Value = String> {
        string_regex("[a-z]{0,12}").expect("the test regex is valid")
    }

    fn arbitrary_label() -> impl Strategy<Value = ommx::ModelingLabel> {
        (
            proptest::option::of(short_string()),
            proptest::collection::vec(any::<i64>(), 0..5),
            proptest::collection::vec((short_string(), short_string()), 0..5),
            proptest::option::of(short_string()),
        )
            .prop_map(
                |(name, subscripts, parameters, description)| ommx::ModelingLabel {
                    name,
                    subscripts,
                    parameters: parameters.into_iter().collect(),
                    description,
                },
            )
    }

    fn arbitrary_provenance() -> impl Strategy<Value = ommx::Provenance> {
        prop_oneof![
            any::<u64>().prop_map(|id| ommx::Provenance::IndicatorConstraint(id.into())),
            any::<u64>().prop_map(|id| ommx::Provenance::OneHotConstraint(id.into())),
            any::<u64>().prop_map(|id| ommx::Provenance::Sos1Constraint(id.into())),
        ]
    }

    fn arbitrary_context() -> impl Strategy<Value = ommx::ConstraintContext> {
        (
            arbitrary_label(),
            proptest::collection::vec(arbitrary_provenance(), 0..5),
        )
            .prop_map(|(label, provenance)| ommx::ConstraintContext { label, provenance })
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(64))]

        #[test]
        fn function_payload_roundtrips_arbitrary_functions(function in arbitrary_function()) {
            let expected = function.clone();
            let payload = export::<PyFunction>(function);
            let actual = ommx::Function::from_bytes(&payload).unwrap();
            prop_assert_eq!(actual, expected);
        }

        #[test]
        fn constraint_payloads_preserve_intrinsic_and_owner_data(
            constraint in arbitrary_constraint(),
            context in arbitrary_context(),
        ) {
            let expected_constraint = constraint.clone();
            let expected_context = context.clone();
            let (constraint, context) = export::<PyConstraint>((constraint, context));
            let actual_constraint = ommx::v2::RegularConstraint::decode(constraint.as_slice())
                .unwrap()
                .parse(&())
                .unwrap();
            let actual_context = ommx::v2::ConstraintContext::decode(context.as_slice())
                .unwrap()
                .parse(&())
                .unwrap();
            prop_assert_eq!(actual_constraint, expected_constraint);
            prop_assert_eq!(actual_context, expected_context);
        }

        #[test]
        fn decision_variable_payloads_preserve_identity_intrinsic_and_owner_data(
            id in any::<u64>(),
            decision_variable in any::<ommx::DecisionVariable>(),
            label in arbitrary_label(),
        ) {
            let expected_decision_variable = decision_variable.clone();
            let expected_label = label.clone();
            let (actual_id, decision_variable, label) = export::<PyDecisionVariable>((
                id.into(),
                decision_variable,
                label,
            ));
            let actual_decision_variable = ommx::v2::DecisionVariable::decode(
                decision_variable.as_slice(),
            )
            .unwrap()
            .parse(&ommx::VariableID::from(id))
            .unwrap();
            let actual_label: ommx::ModelingLabel =
                ommx::v2::ModelingLabel::decode(label.as_slice()).unwrap().into();
            prop_assert_eq!(actual_id, id);
            prop_assert_eq!(actual_decision_variable, expected_decision_variable);
            prop_assert_eq!(actual_label, expected_label);
        }

        #[test]
        fn instance_payload_preserves_owner_complete_root(instance in any::<ommx::Instance>()) {
            let expected = instance.clone();
            let payload = export::<PyInstance>(instance);
            let actual = ommx::Instance::from_v2_bytes(&payload).unwrap();
            prop_assert_eq!(actual, expected);
        }
    }

    #[test]
    fn deep_function_and_constraint_payloads_roundtrip() {
        let function = deeply_composed_function(4096);
        let function_bytes = export::<PyFunction>(function.clone());
        assert_eq!(
            ommx::Function::from_bytes(&function_bytes).unwrap(),
            function
        );

        let constraint = ommx::Constraint::equal_to_zero(function);
        let (constraint_bytes, _) =
            export::<PyConstraint>((constraint.clone(), Default::default()));
        assert_eq!(
            ommx::v2::RegularConstraint::decode(constraint_bytes.as_slice())
                .unwrap()
                .parse(&())
                .unwrap(),
            constraint,
        );
    }
}
