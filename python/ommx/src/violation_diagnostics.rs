//! Python projection of the Rust result owners' typed diagnostic maps.
use crate::constraint_id::AnyConstraintID;
use pyo3::{prelude::*, types::PyDict};
use pyo3_stub_gen::{PyStubType, TypeInfo};
use std::collections::BTreeMap;

/// Diagnostics for a violated regular constraint, currently without extra details.
/// Obtained from Solution.violation_diagnostics or SampleSet.violation_diagnostics.
/// Minor releases may add attributes; removing or changing existing information
/// requires a major release.
#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass(from_py_object, eq, frozen)]
#[derive(Debug, Clone, PartialEq, derive_more::From)]
pub struct RegularViolation {
    inner: ommx::RegularViolation,
}

/// Diagnostics for a violated indicator constraint, currently without extra details.
/// Obtained from Solution.violation_diagnostics or SampleSet.violation_diagnostics.
/// Minor releases may add attributes; removing or changing existing information
/// requires a major release.
#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass(from_py_object, eq, frozen)]
#[derive(Debug, Clone, PartialEq, derive_more::From)]
pub struct IndicatorViolation {
    inner: ommx::IndicatorViolation,
}

/// Diagnostics for a violated SOS1 constraint at the result's saved tolerance.
/// Obtained from Solution.violation_diagnostics or SampleSet.violation_diagnostics.
/// Minor releases may add attributes; removing or changing existing information
/// requires a major release.
#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass(from_py_object, eq, frozen)]
#[derive(Debug, Clone, PartialEq, derive_more::From)]
pub struct Sos1Violation {
    inner: ommx::Sos1Violation,
}

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl Sos1Violation {
    /// Original signed values of excess members, keyed by variable ID.
    /// Retains one largest absolute member (ties retain the largest ID), then
    /// reports remaining values exceeding feasibility_atol in absolute value.
    /// This nonempty map describes one possible repair, not a uniquely intended
    /// selection. Mutating the returned dictionary does not change the diagnostic
    /// or the result.
    #[getter]
    fn values(&self) -> BTreeMap<u64, f64> {
        self.inner
            .values
            .iter()
            .map(|(id, value)| (id.into_inner(), *value))
            .collect()
    }
}

#[derive(IntoPyObject)]
enum DiagnosticValue {
    Regular(RegularViolation),
    Indicator(IndicatorViolation),
    OneHot(crate::OneHotViolation),
    Sos1(Sos1Violation),
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
            |value| Ok(DiagnosticValue::Regular(value.into())),
        )?;
        output.extend(
            solution.violation_diagnostics::<ommx::IndicatorConstraint>(min_role),
            |value| Ok(DiagnosticValue::Indicator(value.into())),
        )?;
        output.extend(
            solution.violation_diagnostics::<ommx::OneHotConstraint>(min_role),
            |value| Ok(DiagnosticValue::OneHot(value.try_into()?)),
        )?;
        output.extend(
            solution.violation_diagnostics::<ommx::Sos1Constraint>(min_role),
            |value| Ok(DiagnosticValue::Sos1(value.into())),
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
            |value| Ok(DiagnosticValue::Regular(value.into())),
        )?;
        output.extend(
            samples.violation_diagnostics::<ommx::IndicatorConstraint>(sample_id, min_role)?,
            |value| Ok(DiagnosticValue::Indicator(value.into())),
        )?;
        output.extend(
            samples.violation_diagnostics::<ommx::OneHotConstraint>(sample_id, min_role)?,
            |value| Ok(DiagnosticValue::OneHot(value.try_into()?)),
        )?;
        output.extend(
            samples.violation_diagnostics::<ommx::Sos1Constraint>(sample_id, min_role)?,
            |value| Ok(DiagnosticValue::Sos1(value.into())),
        )?;
        Ok(output)
    }
}
