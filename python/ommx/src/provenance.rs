use pyo3::prelude::*;

pyo3_stub_gen::inventory::submit! {
    pyo3_stub_gen::derive::gen_methods_from_python! { r#"
class ProvenanceKind:
    def __new__(cls, _unconstructible: typing.NoReturn, /) -> ProvenanceKind: ...
"# }
}

crate::enums::open_kind! {
    /// Source constraint family. Additional provenance families may be added.
    ProvenanceKind, ommx::ConstraintKind, |kind| kind as i32;
    Constraint => Regular, IndicatorConstraint => Indicator, OneHotConstraint => OneHot, Sos1Constraint => Sos1
}

/// One step in a constraint's transformation history.
///
/// When a special constraint (indicator / one-hot / SOS1) is converted into a
/// regular {class}`~ommx.Constraint` — for example via
/// {meth}`~ommx.Instance.convert_one_hot_to_constraint` or
/// {meth}`~ommx.Instance.lower_special_constraints` —
/// the generated constraint records a {class}`Provenance` entry naming the
/// original special constraint. This lets callers trace a regular constraint
/// back to the special constraint it was derived from. Integer-slack operations
/// similarly record the original regular constraint using ``ProvenanceKind.Constraint``.
#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass(from_py_object, eq, hash, frozen)]
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
    /// The kind of source constraint this constraint was generated from.
    #[getter]
    pub fn kind(&self) -> ProvenanceKind {
        self.kind
    }

    /// The ID of the original source constraint (before transformation).
    #[getter]
    pub fn original_id(&self) -> PyResult<crate::constraint_id::AnyConstraintID> {
        use crate::constraint_id::AnyConstraintID;
        Ok(match self.kind {
            ProvenanceKind::Constraint => {
                AnyConstraintID::Regular(ommx::ConstraintID::from(self.original_id).into())
            }
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
        let kind_name = match ommx::ConstraintKind::from(self.kind) {
            ommx::ConstraintKind::Regular => "Constraint".to_owned(),
            kind => format!("{kind:?}Constraint"),
        };
        format!("Provenance({}({}))", kind_name, self.original_id)
    }
}
