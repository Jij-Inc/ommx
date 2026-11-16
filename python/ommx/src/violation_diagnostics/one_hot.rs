use super::ViolationDiagnosticsBase;
use pyo3::prelude::*;
use pyo3_stub_gen::{PyStubType, TypeInfo};
use std::collections::{BTreeMap, BTreeSet};

/// Base of the OneHot MissingSelection, MultipleSelections, and ValueErrors variants.
/// All variants retain every member value, including zeros. Minor releases may
/// add variants; existing variant fields retain their meaning and availability.
/// Match the named variants and retain a fallback for future versions.
#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass(subclass, frozen, extends = ViolationDiagnosticsBase)]
#[derive(Default)]
pub struct OneHotViolationDiagnostics;
crate::constraint_base::constraint_subclass!(OneHotViolationDiagnostics, ViolationDiagnosticsBase);

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl OneHotViolationDiagnostics {
    /// All original member values, including zeros, as an independent dictionary.
    #[getter]
    fn values(&self) -> PyResult<BTreeMap<u64, f64>> {
        Err(pyo3::exceptions::PyNotImplementedError::new_err(
            "OneHot diagnostic variants must provide member values",
        ))
    }
}

/// Every member is within the saved tolerance of zero; no member is selected.
#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass(eq, frozen, extends = OneHotViolationDiagnostics)]
#[derive(Clone, Debug, PartialEq)]
pub struct OneHotMissingSelection {
    /// All original member values. The returned dictionary is an independent copy.
    #[pyo3(get)]
    values: BTreeMap<u64, f64>,
}

/// At least two members are within the saved tolerance of one.
/// Other errors may coexist and remain visible in values.
#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass(eq, frozen, extends = OneHotViolationDiagnostics)]
#[derive(Clone, Debug, PartialEq)]
pub struct OneHotMultipleSelections {
    /// IDs of all members within tolerance of one.
    #[pyo3(get)]
    selected_variables: BTreeSet<u64>,
    /// All original member values. The returned dictionary is an independent copy.
    #[pyo3(get)]
    values: BTreeMap<u64, f64>,
}

/// Deviations from one closest OneHot assignment. The target is the largest
/// member value, with ties resolved by largest ID; it is one possible repair.
#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass(eq, frozen, extends = OneHotViolationDiagnostics)]
#[derive(Clone, Debug, PartialEq)]
pub struct OneHotValueErrors {
    /// Member to set to one in this repair.
    #[pyo3(get)]
    target_variable: u64,
    /// Absolute changes exceeding the saved tolerance: distance to one for
    /// the target and distance to zero for other members.
    #[pyo3(get)]
    errors: BTreeMap<u64, f64>,
    /// All original member values. The returned dictionary is an independent copy.
    #[pyo3(get)]
    values: BTreeMap<u64, f64>,
}

macro_rules! variant {
    ($variant:ident) => {
        impl From<$variant> for PyClassInitializer<$variant> {
            fn from(value: $variant) -> Self {
                PyClassInitializer::from(ViolationDiagnosticsBase)
                    .add_subclass(OneHotViolationDiagnostics)
                    .add_subclass(value)
            }
        }
        impl<'py> IntoPyObject<'py> for $variant {
            type Target = Self;
            type Output = Bound<'py, Self>;
            type Error = PyErr;
            fn into_pyobject(self, py: Python<'py>) -> PyResult<Self::Output> {
                Bound::new(py, self)
            }
        }
    };
}
variant!(OneHotMissingSelection);
variant!(OneHotMultipleSelections);
variant!(OneHotValueErrors);

// Named class aliases preserve the enum-style spelling while the native
// hierarchy makes isinstance(..., ViolationDiagnosticsBase) truthful as well.
#[allow(non_snake_case)]
#[pymethods]
impl OneHotViolationDiagnostics {
    #[classattr]
    fn MissingSelection(py: Python<'_>) -> Bound<'_, pyo3::types::PyType> {
        py.get_type::<OneHotMissingSelection>()
    }
    #[classattr]
    fn MultipleSelections(py: Python<'_>) -> Bound<'_, pyo3::types::PyType> {
        py.get_type::<OneHotMultipleSelections>()
    }
    #[classattr]
    fn ValueErrors(py: Python<'_>) -> Bound<'_, pyo3::types::PyType> {
        py.get_type::<OneHotValueErrors>()
    }
}
fn class_type<T: PyStubType>() -> TypeInfo {
    let mut info = T::type_output();
    info.name = format!("builtins.type[{}]", info.name);
    info.import.insert("builtins".into());
    info.source_module = None;
    info
}
pyo3_stub_gen::inventory::submit! {
    pyo3_stub_gen::type_info::PyMethodsInfo {
        struct_id: std::any::TypeId::of::<OneHotViolationDiagnostics>,
        attrs: &[
            pyo3_stub_gen::type_info::MemberInfo { name: "MissingSelection", r#type: class_type::<OneHotMissingSelection>, doc: "Missing-selection variant.", deprecated: None, default: None },
            pyo3_stub_gen::type_info::MemberInfo { name: "MultipleSelections", r#type: class_type::<OneHotMultipleSelections>, doc: "Multiple-selection variant.", deprecated: None, default: None },
            pyo3_stub_gen::type_info::MemberInfo { name: "ValueErrors", r#type: class_type::<OneHotValueErrors>, doc: "Value-error variant.", deprecated: None, default: None },
        ],
        methods: &[], getters: &[], setters: &[], file: file!(), line: line!(), column: column!(),
    }
}

#[derive(Clone, IntoPyObject)]
pub enum OneHotDiagnosticValue {
    MissingSelection(OneHotMissingSelection),
    MultipleSelections(OneHotMultipleSelections),
    ValueErrors(OneHotValueErrors),
}
fn values(values: BTreeMap<ommx::VariableID, f64>) -> BTreeMap<u64, f64> {
    values
        .into_iter()
        .map(|(id, x)| (id.into_inner(), x))
        .collect()
}
impl TryFrom<ommx::OneHotViolationDiagnostics> for OneHotDiagnosticValue {
    type Error = ommx::Error;
    fn try_from(value: ommx::OneHotViolationDiagnostics) -> ommx::Result<Self> {
        Ok(match value {
            ommx::OneHotViolationDiagnostics::MissingSelection { values: xs } => {
                Self::MissingSelection(OneHotMissingSelection { values: values(xs) })
            }
            ommx::OneHotViolationDiagnostics::MultipleSelections {
                selected_variables,
                values: xs,
            } => Self::MultipleSelections(OneHotMultipleSelections {
                selected_variables: selected_variables
                    .into_iter()
                    .map(|id| id.into_inner())
                    .collect(),
                values: values(xs),
            }),
            ommx::OneHotViolationDiagnostics::ValueErrors {
                target_variable,
                errors,
                values: xs,
            } => Self::ValueErrors(OneHotValueErrors {
                target_variable: target_variable.into_inner(),
                errors: values(errors),
                values: values(xs),
            }),
            _ => return Err(ommx::Error::msg("Unsupported OneHot diagnostic variant")),
        })
    }
}
