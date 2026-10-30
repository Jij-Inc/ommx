use pyo3::prelude::*;

crate::enums::open_kind! {
    /// Source constraint family. Additional provenance families may be added.
    ProvenanceKind, ommx::SpecialConstraintKind, |kind| kind as i32;
    IndicatorConstraint => Indicator, OneHotConstraint => OneHot, Sos1Constraint => Sos1
}

/// One step in a regular constraint's transformation history.
///
/// When a special constraint (indicator / one-hot / SOS1) is converted into a
/// regular {class}`~ommx.Constraint` — for example via
/// {meth}`~ommx.Instance.convert_one_hot_to_constraint` or
/// {meth}`~ommx.Instance.lower_special_constraints` —
/// the generated constraint records a {class}`Provenance` entry naming the
/// original special constraint. This lets callers trace a regular constraint
/// back to the special constraint it was derived from.
#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass(eq, hash, frozen)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Provenance {
    kind: ProvenanceKind,
    original_id: u64,
}

impl From<&ommx::Provenance> for Provenance {
    fn from(p: &ommx::Provenance) -> Self {
        Self {
            kind: p.kind().into(),
            original_id: p.original_id(),
        }
    }
}

pub fn provenance_list(context: &ommx::ConstraintContext) -> Vec<Provenance> {
    context.provenance.iter().map(Provenance::from).collect()
}

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl Provenance {
    /// The kind of special constraint this regular constraint was generated from.
    #[getter]
    pub fn kind(&self) -> ProvenanceKind {
        self.kind
    }

    /// The ID of the original special constraint (before transformation).
    #[getter]
    pub fn original_id(&self) -> PyResult<crate::constraint_id::AnyConstraintID> {
        use crate::constraint_id::AnyConstraintID;
        Ok(match self.kind {
            ProvenanceKind::IndicatorConstraint => AnyConstraintID::Indicator(
                ommx::IndicatorConstraintID::from(self.original_id).into(),
            ),
            ProvenanceKind::OneHotConstraint => {
                AnyConstraintID::OneHot(ommx::OneHotConstraintID::from(self.original_id).into())
            }
            ProvenanceKind::Sos1Constraint => {
                AnyConstraintID::Sos1(ommx::Sos1ConstraintID::from(self.original_id).into())
            }
            _ => {
                return Err(pyo3::exceptions::PyNotImplementedError::new_err(
                    "Unsupported provenance family",
                ))
            }
        })
    }

    pub fn __repr__(&self) -> String {
        let kind_name = format!(
            "{:?}Constraint",
            ommx::SpecialConstraintKind::from(self.kind)
        );
        format!("Provenance({}({}))", kind_name, self.original_id)
    }
}
