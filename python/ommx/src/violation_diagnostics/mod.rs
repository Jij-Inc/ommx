//! Python projection of the Rust result owners' typed diagnostic maps.
use crate::constraint_id::AnyConstraintID;
use pyo3::prelude::*;
use pyo3_stub_gen::{PyStubType, TypeInfo};
use std::collections::BTreeMap;
use std::sync::Arc;
mod mapping;
mod one_hot;
pub use mapping::ViolationDiagnosticsFamilyView;
use one_hot::OneHotDiagnosticValue;
pub use one_hot::{
    OneHotMissingSelection, OneHotMultipleSelections, OneHotValueErrors, OneHotViolationDiagnostics,
};

/// Common interface for diagnostics from any current or future constraint family.
/// Inspect the concrete type before using family-specific information.
#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass(subclass, frozen)]
#[derive(Default)]
pub struct ViolationDiagnosticsBase;

/// Diagnostics for a violated regular constraint, currently without extra details.
/// Obtained from Solution.violation_diagnostics or SampleSet.violation_diagnostics.
/// Minor releases may add attributes; removing or changing existing information
/// requires a major release.
#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass(eq, frozen, extends = ViolationDiagnosticsBase)]
#[derive(Debug, Clone, PartialEq, derive_more::From)]
pub struct RegularViolationDiagnostics {
    inner: ommx::RegularViolationDiagnostics,
}
crate::constraint_base::constraint_subclass!(RegularViolationDiagnostics, ViolationDiagnosticsBase);

/// Diagnostics for a violated indicator constraint, currently without extra details.
/// Obtained from Solution.violation_diagnostics or SampleSet.violation_diagnostics.
/// Minor releases may add attributes; removing or changing existing information
/// requires a major release.
#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass(eq, frozen, extends = ViolationDiagnosticsBase)]
#[derive(Debug, Clone, PartialEq, derive_more::From)]
pub struct IndicatorViolationDiagnostics {
    inner: ommx::IndicatorViolationDiagnostics,
}
crate::constraint_base::constraint_subclass!(
    IndicatorViolationDiagnostics,
    ViolationDiagnosticsBase
);

/// Diagnostics for a violated SOS1 constraint at the result's saved tolerance.
/// Obtained from Solution.violation_diagnostics or SampleSet.violation_diagnostics.
/// Minor releases may add attributes; removing or changing existing information
/// requires a major release.
#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass(eq, frozen, extends = ViolationDiagnosticsBase)]
#[derive(Debug, Clone, PartialEq, derive_more::From)]
pub struct Sos1ViolationDiagnostics {
    inner: ommx::Sos1ViolationDiagnostics,
}
crate::constraint_base::constraint_subclass!(Sos1ViolationDiagnostics, ViolationDiagnosticsBase);

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl Sos1ViolationDiagnostics {
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

#[derive(Clone, IntoPyObject)]
enum DiagnosticValue {
    Regular(RegularViolationDiagnostics),
    Indicator(IndicatorViolationDiagnostics),
    OneHot(OneHotDiagnosticValue),
    Sos1(Sos1ViolationDiagnostics),
}

impl PyStubType for DiagnosticValue {
    fn type_output() -> TypeInfo {
        ViolationDiagnosticsBase::type_output()
    }
}

#[doc = include_str!("../docstrings/ommx/ViolationDiagnostics.md")]
#[pyclass(frozen)]
pub struct ViolationDiagnostics {
    entries: Arc<BTreeMap<AnyConstraintID, DiagnosticValue>>,
}

impl ViolationDiagnostics {
    fn extend<ID: Into<AnyConstraintID>, D>(
        &mut self,
        entries: BTreeMap<ID, D>,
        mut convert: impl FnMut(D) -> ommx::Result<DiagnosticValue>,
    ) -> ommx::Result<()> {
        for (id, value) in entries {
            Arc::make_mut(&mut self.entries).insert(id.into(), convert(value)?);
        }
        Ok(())
    }

    pub fn solution(
        solution: &ommx::Solution,
        min_role: ommx::ConstraintRole,
    ) -> ommx::Result<Self> {
        let mut output = Self {
            entries: Arc::new(BTreeMap::new()),
        };
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
        let mut output = Self {
            entries: Arc::new(BTreeMap::new()),
        };
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

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<ViolationDiagnosticsBase>()?;
    m.add_class::<RegularViolationDiagnostics>()?;
    m.add_class::<IndicatorViolationDiagnostics>()?;
    m.add_class::<Sos1ViolationDiagnostics>()?;
    m.add_class::<OneHotViolationDiagnostics>()?;
    m.add_class::<OneHotMissingSelection>()?;
    m.add_class::<OneHotMultipleSelections>()?;
    m.add_class::<OneHotValueErrors>()?;
    mapping::register(m)
}
