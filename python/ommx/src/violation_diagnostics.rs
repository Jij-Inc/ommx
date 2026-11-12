//! Python projection of the Rust result owners' typed diagnostic maps.
use crate::constraint_id::AnyConstraintID;
use pyo3::{prelude::*, types::PyDict};
use pyo3_stub_gen::{PyStubType, TypeInfo};
use std::collections::BTreeMap;

#[derive(IntoPyObject)]
enum DiagnosticValue {
    // PyO3 converts Rust () to an empty tuple; absent detail is Python None.
    Unit(Option<()>),
    OneHot(crate::OneHotViolation),
    Sos1(BTreeMap<u64, f64>),
}

/// The public dictionary is extensible: consumers narrow values by typed ID,
/// rather than relying on a closed union of today's diagnostic families.
/// This wrapper owns Python conversion only; filtering and computation stay in Rust.
pub(crate) struct ViolationDiagnostics(Vec<(AnyConstraintID, DiagnosticValue)>);

impl PyStubType for ViolationDiagnostics {
    fn type_output() -> TypeInfo {
        let mut info = AnyConstraintID::type_output();
        info.name = format!("builtins.dict[{}, builtins.object]", info.name);
        info
    }
}
impl<'py> IntoPyObject<'py> for ViolationDiagnostics {
    type Target = PyDict;
    type Output = Bound<'py, PyDict>;
    type Error = PyErr;
    fn into_pyobject(self, py: Python<'py>) -> PyResult<Self::Output> {
        let dict = PyDict::new(py);
        for (id, value) in self.0 {
            dict.set_item(id, value)?;
        }
        Ok(dict)
    }
}

impl ViolationDiagnostics {
    fn extend<ID: Into<AnyConstraintID>, D>(
        &mut self,
        entries: BTreeMap<ID, D>,
        mut convert: impl FnMut(D) -> ommx::Result<DiagnosticValue>,
    ) -> ommx::Result<()> {
        for (id, value) in entries {
            self.0.push((id.into(), convert(value)?));
        }
        Ok(())
    }

    pub fn solution(
        solution: &ommx::Solution,
        min_role: ommx::ConstraintRole,
    ) -> ommx::Result<Self> {
        let mut output = Self(Vec::new());
        output.extend(
            solution.violation_diagnostics::<ommx::Constraint>(min_role),
            |()| Ok(DiagnosticValue::Unit(None)),
        )?;
        output.extend(
            solution.violation_diagnostics::<ommx::IndicatorConstraint>(min_role),
            |()| Ok(DiagnosticValue::Unit(None)),
        )?;
        output.extend(
            solution.violation_diagnostics::<ommx::OneHotConstraint>(min_role),
            |value| Ok(DiagnosticValue::OneHot(value.try_into()?)),
        )?;
        output.extend(
            solution.violation_diagnostics::<ommx::Sos1Constraint>(min_role),
            |values| Ok(sos1(values)),
        )?;
        Ok(output)
    }

    pub fn sample_set(
        samples: &ommx::SampleSet,
        sample_id: ommx::SampleID,
        min_role: ommx::ConstraintRole,
    ) -> ommx::Result<Self> {
        let mut output = Self(Vec::new());
        output.extend(
            samples.violation_diagnostics::<ommx::Constraint>(sample_id, min_role)?,
            |()| Ok(DiagnosticValue::Unit(None)),
        )?;
        output.extend(
            samples.violation_diagnostics::<ommx::IndicatorConstraint>(sample_id, min_role)?,
            |()| Ok(DiagnosticValue::Unit(None)),
        )?;
        output.extend(
            samples.violation_diagnostics::<ommx::OneHotConstraint>(sample_id, min_role)?,
            |value| Ok(DiagnosticValue::OneHot(value.try_into()?)),
        )?;
        output.extend(
            samples.violation_diagnostics::<ommx::Sos1Constraint>(sample_id, min_role)?,
            |values| Ok(sos1(values)),
        )?;
        Ok(output)
    }
}

fn sos1(values: BTreeMap<ommx::VariableID, f64>) -> DiagnosticValue {
    DiagnosticValue::Sos1(
        values
            .into_iter()
            .map(|(id, value)| (id.into_inner(), value))
            .collect(),
    )
}
