use pyo3::prelude::*;
use std::collections::{BTreeMap, BTreeSet};

/// A OneHot violation at the result's feasibility_atol. Each variant retains
/// all member values, including zeros. Feasible rows are absent from the diagnostic
/// dictionary. Future versions may add variants or fields.
/// Minor releases may add variants or fields; removing or changing existing
/// information requires a major release.
#[pyo3_stub_gen::derive::gen_stub_pyclass_complex_enum]
#[pyclass(from_py_object, eq, frozen)]
#[derive(Debug, Clone, PartialEq)]
pub enum OneHotViolation {
    /// All members are within tolerance of zero, but the row is infeasible.
    MissingSelection { values: BTreeMap<u64, f64> },
    /// The row is infeasible and at least two members are within tolerance of one.
    /// Other value errors can coexist and remain visible in values.
    MultipleSelections {
        selected_variables: BTreeSet<u64>,
        values: BTreeMap<u64, f64>,
    },
    /// Other value deviations from a closest OneHot assignment.
    /// The target is the largest member value, breaking ties by largest ID.
    /// Errors are absolute changes exceeding feasibility_atol: distance to one
    /// for the target, distance to zero for the other members. The target is one
    /// possible repair choice, not a uniquely intended selection.
    ValueErrors {
        target_variable: u64,
        errors: BTreeMap<u64, f64>,
        values: BTreeMap<u64, f64>,
    },
}

fn values_to_python(values: BTreeMap<ommx::VariableID, f64>) -> BTreeMap<u64, f64> {
    values
        .into_iter()
        .map(|(id, value)| (id.into_inner(), value))
        .collect()
}

impl TryFrom<ommx::OneHotViolation> for OneHotViolation {
    type Error = ommx::Error;
    fn try_from(value: ommx::OneHotViolation) -> ommx::Result<Self> {
        Ok(match value {
            ommx::OneHotViolation::MissingSelection { values, .. } => Self::MissingSelection {
                values: values_to_python(values),
            },
            ommx::OneHotViolation::MultipleSelections {
                selected_variables,
                values,
                ..
            } => Self::MultipleSelections {
                selected_variables: selected_variables
                    .into_iter()
                    .map(|id| id.into_inner())
                    .collect(),
                values: values_to_python(values),
            },
            ommx::OneHotViolation::ValueErrors {
                target_variable,
                errors,
                values,
                ..
            } => Self::ValueErrors {
                target_variable: target_variable.into_inner(),
                errors: values_to_python(errors),
                values: values_to_python(values),
            },
            _ => return Err(ommx::Error::msg("Unsupported OneHot diagnostic variant")),
        })
    }
}
