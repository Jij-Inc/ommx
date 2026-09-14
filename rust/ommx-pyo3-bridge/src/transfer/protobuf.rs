//! Codecs for the two registered protobuf transfer contracts.

use super::*;
use crate::{
    PyConstraint, PyDecisionVariable, PyFunction, PyInstance, PyParametricInstance, PySampleSet,
    PySolution,
};
use ommx::Message;
use pyo3::types::PyBytes;

fn public_receiver(module: &Bound<'_, PyModule>, class: &str, method: &str) -> PyResult<Py<PyAny>> {
    Ok(module.getattr(class)?.getattr(method)?.unbind())
}

fn private_receiver(module: &Bound<'_, PyModule>, endpoint: &str) -> PyResult<Py<PyAny>> {
    Ok(module.getattr("_ommx_rust")?.getattr(endpoint)?.unbind())
}

fn import_bytes<'py>(bytes: Vec<u8>, receiver: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    receiver.call1((PyBytes::new(receiver.py(), &bytes),))
}

macro_rules! root {
    ($wrapper:ident, $domain:ident, $name:literal) => {
        impl super::sealed::Type<ProtobufV1> for $wrapper {}
        impl super::sealed::Type<ProtobufV2> for $wrapper {}
        impl TransferVia<ProtobufV1> for $wrapper {
            type Payload = Vec<u8>;
            const PYTHON_NAME: &'static str = concat!("ommx.", $name);
            fn receiver(module: &Bound<'_, PyModule>) -> PyResult<Py<PyAny>> {
                public_receiver(module, $name, "from_v1_bytes")
            }
            fn import(payload: Vec<u8>, receiver: &Bound<'_, PyAny>) -> PyResult<Self> {
                import_bytes(payload, receiver).map(|object| Self(crate::Output::python(object)))
            }
        }
        impl TransferVia<ProtobufV2> for $wrapper {
            type Payload = Vec<u8>;
            const PYTHON_NAME: &'static str = concat!("ommx.", $name);
            fn receiver(module: &Bound<'_, PyModule>) -> PyResult<Py<PyAny>> {
                public_receiver(module, $name, "from_v2_bytes")
            }
            fn import(payload: Vec<u8>, receiver: &Bound<'_, PyAny>) -> PyResult<Self> {
                import_bytes(payload, receiver).map(|object| Self(crate::Output::python(object)))
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
root!(PyInstance, Instance, "Instance");
root!(
    PyParametricInstance,
    ParametricInstance,
    "ParametricInstance"
);
root!(PySolution, Solution, "Solution");
root!(PySampleSet, SampleSet, "SampleSet");

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
    ($wrapper:ident, $name:literal, $endpoint:literal, $wire:ident) => {
        impl super::sealed::Type<ProtobufV1> for $wrapper {}
        impl super::sealed::Type<ProtobufV2> for $wrapper {}
        impl TransferVia<ProtobufV1> for $wrapper {
            type Payload = Vec<u8>;
            const PYTHON_NAME: &'static str = concat!("ommx.", $name);
            fn receiver(module: &Bound<'_, PyModule>) -> PyResult<Py<PyAny>> {
                private_receiver(module, $endpoint)
            }
            fn import(payload: Vec<u8>, receiver: &Bound<'_, PyAny>) -> PyResult<Self> {
                import_bytes(payload, receiver).map(|object| Self(crate::Output::python(object)))
            }
        }
        impl Export<ProtobufV1, $wrapper> for ommx::v1::$wire {
            fn export(self) -> ommx::Result<Vec<u8>> {
                Ok(self.encode_to_vec())
            }
        }
    };
}
v1_component!(
    PyFunction,
    "Function",
    "_bridge_protobuf_v1_function_from_bytes",
    Function
);
v1_component!(
    PyConstraint,
    "Constraint",
    "_bridge_protobuf_v1_constraint_from_bytes",
    Constraint
);
v1_component!(
    PyDecisionVariable,
    "DecisionVariable",
    "_bridge_protobuf_v1_decision_variable_from_bytes",
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
        private_receiver(module, "_pyo3_bridge_v0_function_from_bytes")
    }
    fn import(payload: Vec<u8>, receiver: &Bound<'_, PyAny>) -> PyResult<Self> {
        import_bytes(payload, receiver).map(|object| Self(crate::Output::python(object)))
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
        private_receiver(module, "_pyo3_bridge_v0_constraint_from_bytes")
    }
    fn import((constraint, context): Self::Payload, receiver: &Bound<'_, PyAny>) -> PyResult<Self> {
        let py = receiver.py();
        let object = receiver.call1((PyBytes::new(py, &constraint), PyBytes::new(py, &context)))?;
        Ok(Self(crate::Output::python(object)))
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
        private_receiver(module, "_pyo3_bridge_v0_decision_variable_from_bytes")
    }
    fn import((id, variable, label): Self::Payload, receiver: &Bound<'_, PyAny>) -> PyResult<Self> {
        let py = receiver.py();
        let object = receiver.call1((id, PyBytes::new(py, &variable), PyBytes::new(py, &label)))?;
        Ok(Self(crate::Output::python(object)))
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
