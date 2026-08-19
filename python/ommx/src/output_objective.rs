use crate::{Function, Sense};
use pyo3::prelude::*;

/// Objective semantics used when an Instance produces a Solution or SampleSet.
///
/// Preparation may rewrite an instance's active objective and sense to satisfy
/// an adapter's input requirements.  This value preserves the objective
/// semantics exposed by {meth}`Instance.evaluate` and
/// {meth}`Instance.evaluate_samples` while the rewritten active objective
/// remains available through {attr}`Instance.objective` and
/// {attr}`Instance.sense`.
#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass(frozen)]
#[derive(Clone)]
pub struct OutputObjective(pub(crate) ommx::OutputObjective);

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl OutputObjective {
    /// Optimization sense exposed on Solution and SampleSet outputs.
    #[getter]
    pub fn sense(&self) -> Sense {
        self.0.sense().into()
    }

    /// Objective function evaluated for Solution and SampleSet outputs.
    #[getter]
    pub fn function(&self) -> Function {
        Function(self.0.function().clone())
    }
}
