use pyo3::prelude::*;

/// Kind of constraint from which a regular {class}`~ommx.v1.Constraint` was generated.
///
/// See {class}`~ommx.v1.Provenance` for details.
#[pyo3_stub_gen::derive::gen_stub_pyclass_enum]
#[pyclass(eq, eq_int, hash, frozen)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProvenanceKind {
    /// The regular constraint was generated from an indicator constraint.
    IndicatorConstraint = 1,
    /// The regular constraint was generated from a one-hot constraint.
    OneHotConstraint = 2,
    /// The regular constraint was generated from a SOS1 constraint.
    Sos1Constraint = 3,
}

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl ProvenanceKind {
    fn __repr__(&self) -> &'static str {
        match self {
            ProvenanceKind::IndicatorConstraint => "ProvenanceKind.IndicatorConstraint",
            ProvenanceKind::OneHotConstraint => "ProvenanceKind.OneHotConstraint",
            ProvenanceKind::Sos1Constraint => "ProvenanceKind.Sos1Constraint",
        }
    }
}

/// One step in a regular constraint's transformation history.
///
/// When a special constraint (indicator / one-hot / SOS1) is converted into a
/// regular {class}`~ommx.v1.Constraint` — for example via
/// `Instance.convert_one_hot_to_constraint` or `Instance.reduce_capabilities` —
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
        match p {
            ommx::Provenance::IndicatorConstraint(id) => Self {
                kind: ProvenanceKind::IndicatorConstraint,
                original_id: (*id).into_inner(),
            },
            ommx::Provenance::OneHotConstraint(id) => Self {
                kind: ProvenanceKind::OneHotConstraint,
                original_id: (*id).into_inner(),
            },
            ommx::Provenance::Sos1Constraint(id) => Self {
                kind: ProvenanceKind::Sos1Constraint,
                original_id: (*id).into_inner(),
            },
        }
    }
}

pub fn provenance_list(metadata: &ommx::ConstraintMetadata) -> Vec<Provenance> {
    metadata.provenance.iter().map(Provenance::from).collect()
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
    pub fn original_id(&self) -> u64 {
        self.original_id
    }

    pub fn __repr__(&self) -> String {
        let kind_name = match self.kind {
            ProvenanceKind::IndicatorConstraint => "IndicatorConstraint",
            ProvenanceKind::OneHotConstraint => "OneHotConstraint",
            ProvenanceKind::Sos1Constraint => "Sos1Constraint",
        };
        format!("Provenance({}({}))", kind_name, self.original_id)
    }
}
