use crate::{Equality, Instance, Kind, Sense};
use pyo3::prelude::*;
use std::collections::{BTreeMap, BTreeSet};

/// Cumulative polynomial-degree bound in an :class:`InstanceClassClause`.
#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass(eq, frozen)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DegreeBound(pub ommx::DegreeBound);

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl DegreeBound {
    /// Include every degree up to and including ``maximum``.
    #[staticmethod]
    pub fn at_most(maximum: u32) -> Self {
        Self(ommx::DegreeBound::at_most(maximum))
    }

    /// Include every polynomial degree representable by OMMX.
    #[staticmethod]
    pub fn unbounded() -> Self {
        Self(ommx::DegreeBound::Unbounded)
    }

    /// Inclusive maximum degree, or ``None`` when unbounded.
    #[getter]
    pub fn maximum(&self) -> Option<u32> {
        self.0.maximum().map(|degree| degree.into_inner())
    }

    /// Return whether ``actual_degree`` satisfies this bound.
    pub fn includes(&self, actual_degree: u32) -> bool {
        self.0.includes(actual_degree.into())
    }

    pub fn __repr__(&self) -> String {
        match self.maximum() {
            Some(maximum) => format!("DegreeBound.at_most({maximum})"),
            None => "DegreeBound.unbounded()".to_string(),
        }
    }
}

/// One conjunctive clause in an :class:`InstanceClass`.
///
/// Every condition in a clause must hold. The containing instance class is
/// the finite union of its clauses, so alternatives are not combined across
/// clause boundaries.
#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass(frozen)]
#[derive(Debug, Clone)]
pub struct InstanceClassClause(pub ommx::InstanceClassClause);

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl InstanceClassClause {
    #[new]
    #[pyo3(signature = (*, label, allowed_variable_kinds, objective_degree_bound, allowed_senses, regular_constraint_degree_bounds=None, indicator_constraint_degree_bounds=None, allows_one_hot=false, allows_sos1=false))]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        label: String,
        allowed_variable_kinds: BTreeSet<Kind>,
        objective_degree_bound: DegreeBound,
        allowed_senses: BTreeSet<Sense>,
        regular_constraint_degree_bounds: Option<BTreeMap<Equality, DegreeBound>>,
        indicator_constraint_degree_bounds: Option<BTreeMap<Equality, DegreeBound>>,
        allows_one_hot: bool,
        allows_sos1: bool,
    ) -> Self {
        let mut clause = ommx::InstanceClassClause::new(
            label,
            allowed_variable_kinds.into_iter().map(Into::into).collect(),
            objective_degree_bound.0,
            allowed_senses.into_iter().map(Into::into).collect(),
        );
        for (relation, bound) in regular_constraint_degree_bounds.unwrap_or_default() {
            clause = clause.with_regular_constraint(relation.into(), bound.0);
        }
        for (relation, bound) in indicator_constraint_degree_bounds.unwrap_or_default() {
            clause = clause.with_indicator_constraint(relation.into(), bound.0);
        }
        if allows_one_hot {
            clause = clause.with_one_hot();
        }
        if allows_sos1 {
            clause = clause.with_sos1();
        }
        Self(clause)
    }

    /// Human-readable diagnostic label. It does not affect membership.
    #[getter]
    pub fn label(&self) -> &str {
        self.0.label()
    }

    #[getter]
    pub fn allowed_variable_kinds(&self) -> BTreeSet<Kind> {
        self.0
            .allowed_variable_kinds()
            .iter()
            .copied()
            .map(Into::into)
            .collect()
    }

    #[getter]
    pub fn objective_degree_bound(&self) -> DegreeBound {
        DegreeBound(self.0.objective_degree_bound())
    }

    #[getter]
    pub fn regular_constraint_degree_bounds(&self) -> BTreeMap<Equality, DegreeBound> {
        self.0
            .regular_constraint_degree_bounds()
            .map(|(relation, bound)| (relation.into(), DegreeBound(bound)))
            .collect()
    }

    #[getter]
    pub fn indicator_constraint_degree_bounds(&self) -> BTreeMap<Equality, DegreeBound> {
        self.0
            .indicator_constraint_degree_bounds()
            .map(|(relation, bound)| (relation.into(), DegreeBound(bound)))
            .collect()
    }

    #[getter]
    pub fn allows_one_hot(&self) -> bool {
        self.0.allows_one_hot()
    }

    #[getter]
    pub fn allows_sos1(&self) -> bool {
        self.0.allows_sos1()
    }

    #[getter]
    pub fn allowed_senses(&self) -> BTreeSet<Sense> {
        self.0
            .allowed_senses()
            .iter()
            .copied()
            .map(Into::into)
            .collect()
    }

    pub fn __repr__(&self) -> String {
        format!("InstanceClassClause(label={:?})", self.label())
    }
}

/// A set of :class:`Instance` values represented as a finite union of clauses.
#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass(frozen)]
#[derive(Debug, Clone)]
pub struct InstanceClass(pub ommx::InstanceClass);

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl InstanceClass {
    #[new]
    pub fn new(clauses: Vec<InstanceClassClause>) -> Self {
        Self(ommx::InstanceClass::new(
            clauses.into_iter().map(|clause| clause.0).collect(),
        ))
    }

    #[getter]
    pub fn clauses(&self) -> Vec<InstanceClassClause> {
        self.0
            .clauses()
            .iter()
            .cloned()
            .map(InstanceClassClause)
            .collect()
    }

    /// Return the finite union of two instance classes.
    pub fn union(&self, other: &InstanceClass) -> Self {
        Self(self.0.clone().union(other.0.clone()))
    }

    /// Return whether ``instance`` belongs to this class.
    pub fn contains(&self, instance: &Instance) -> bool {
        self.0.contains(&instance.inner)
    }

    /// Evaluate membership without mutating or preparing ``instance``.
    pub fn check_membership(&self, instance: &Instance) -> InstanceClassMembershipReport {
        InstanceClassMembershipReport(self.0.check_membership(&instance.inner))
    }

    pub fn __repr__(&self) -> String {
        format!("InstanceClass(clauses={:?})", self.clauses())
    }
}

/// One structured reason an instance is outside a complete clause.
#[pyo3_stub_gen::derive::gen_stub_pyclass_complex_enum]
#[pyclass(eq, frozen)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstanceClassMismatch {
    VariableKindNotAllowed {
        kind: Kind,
        variable_ids: BTreeSet<u64>,
        allowed_kinds: BTreeSet<Kind>,
    },
    ObjectiveDegreeExceedsBound {
        actual_degree: u32,
        bound: DegreeBound,
    },
    RegularConstraintRelationNotAllowed {
        relation: Equality,
        constraint_ids: BTreeSet<u64>,
        allowed_relations: BTreeSet<Equality>,
    },
    RegularConstraintDegreeExceedsBound {
        relation: Equality,
        actual_degrees: BTreeMap<u64, u32>,
        bound: DegreeBound,
    },
    IndicatorConstraintsNotAllowed {
        constraint_ids: BTreeSet<u64>,
    },
    IndicatorConstraintRelationNotAllowed {
        relation: Equality,
        constraint_ids: BTreeSet<u64>,
        allowed_relations: BTreeSet<Equality>,
    },
    IndicatorBodyDegreeExceedsBound {
        relation: Equality,
        actual_degrees: BTreeMap<u64, u32>,
        bound: DegreeBound,
    },
    OneHotConstraintsNotAllowed {
        constraint_ids: BTreeSet<u64>,
    },
    Sos1ConstraintsNotAllowed {
        constraint_ids: BTreeSet<u64>,
    },
    SenseNotAllowed {
        sense: Sense,
        allowed_senses: BTreeSet<Sense>,
    },
    Unknown {
        message: String,
    },
}

impl From<ommx::InstanceClassMismatch> for InstanceClassMismatch {
    fn from(mismatch: ommx::InstanceClassMismatch) -> Self {
        match mismatch {
            ommx::InstanceClassMismatch::VariableKindNotAllowed {
                kind,
                variable_ids,
                allowed_kinds,
            } => Self::VariableKindNotAllowed {
                kind: kind.into(),
                variable_ids: variable_ids.iter().map(|id| id.into_inner()).collect(),
                allowed_kinds: allowed_kinds.into_iter().map(Into::into).collect(),
            },
            ommx::InstanceClassMismatch::ObjectiveDegreeExceedsBound {
                actual_degree,
                bound,
            } => Self::ObjectiveDegreeExceedsBound {
                actual_degree: actual_degree.into_inner(),
                bound: DegreeBound(bound),
            },
            ommx::InstanceClassMismatch::RegularConstraintRelationNotAllowed {
                relation,
                constraint_ids,
                allowed_relations,
            } => Self::RegularConstraintRelationNotAllowed {
                relation: relation.into(),
                constraint_ids: constraint_ids
                    .into_iter()
                    .map(|id| id.into_inner())
                    .collect(),
                allowed_relations: allowed_relations.into_iter().map(Into::into).collect(),
            },
            ommx::InstanceClassMismatch::RegularConstraintDegreeExceedsBound {
                relation,
                actual_degrees,
                bound,
            } => Self::RegularConstraintDegreeExceedsBound {
                relation: relation.into(),
                actual_degrees: actual_degrees
                    .into_iter()
                    .map(|(id, degree)| (id.into_inner(), degree.into_inner()))
                    .collect(),
                bound: DegreeBound(bound),
            },
            ommx::InstanceClassMismatch::IndicatorConstraintsNotAllowed { constraint_ids } => {
                Self::IndicatorConstraintsNotAllowed {
                    constraint_ids: constraint_ids
                        .into_iter()
                        .map(|id| id.into_inner())
                        .collect(),
                }
            }
            ommx::InstanceClassMismatch::IndicatorConstraintRelationNotAllowed {
                relation,
                constraint_ids,
                allowed_relations,
            } => Self::IndicatorConstraintRelationNotAllowed {
                relation: relation.into(),
                constraint_ids: constraint_ids
                    .into_iter()
                    .map(|id| id.into_inner())
                    .collect(),
                allowed_relations: allowed_relations.into_iter().map(Into::into).collect(),
            },
            ommx::InstanceClassMismatch::IndicatorBodyDegreeExceedsBound {
                relation,
                actual_degrees,
                bound,
            } => Self::IndicatorBodyDegreeExceedsBound {
                relation: relation.into(),
                actual_degrees: actual_degrees
                    .into_iter()
                    .map(|(id, degree)| (id.into_inner(), degree.into_inner()))
                    .collect(),
                bound: DegreeBound(bound),
            },
            ommx::InstanceClassMismatch::OneHotConstraintsNotAllowed { constraint_ids } => {
                Self::OneHotConstraintsNotAllowed {
                    constraint_ids: constraint_ids
                        .into_iter()
                        .map(|id| id.into_inner())
                        .collect(),
                }
            }
            ommx::InstanceClassMismatch::Sos1ConstraintsNotAllowed { constraint_ids } => {
                Self::Sos1ConstraintsNotAllowed {
                    constraint_ids: constraint_ids
                        .into_iter()
                        .map(|id| id.into_inner())
                        .collect(),
                }
            }
            ommx::InstanceClassMismatch::SenseNotAllowed {
                sense,
                allowed_senses,
            } => Self::SenseNotAllowed {
                sense: sense.into(),
                allowed_senses: allowed_senses.into_iter().map(Into::into).collect(),
            },
            other => Self::Unknown {
                message: other.to_string(),
            },
        }
    }
}

/// Membership result for one conjunctive instance-class clause.
#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass(eq, frozen)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstanceClassClauseReport(pub ommx::InstanceClassClauseReport);

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl InstanceClassClauseReport {
    #[getter]
    pub fn clause_index(&self) -> usize {
        self.0.clause_index()
    }

    #[getter]
    pub fn clause_label(&self) -> &str {
        self.0.clause_label()
    }

    #[getter]
    pub fn mismatches(&self) -> Vec<InstanceClassMismatch> {
        self.0
            .mismatches()
            .iter()
            .cloned()
            .map(Into::into)
            .collect()
    }

    #[getter]
    pub fn is_member(&self) -> bool {
        self.0.is_member()
    }

    pub fn __repr__(&self) -> String {
        format!(
            "InstanceClassClauseReport(clause_index={}, clause_label={:?}, is_member={})",
            self.clause_index(),
            self.clause_label(),
            self.is_member()
        )
    }
}

/// Side-effect-free membership report for an :class:`InstanceClass`.
#[pyo3_stub_gen::derive::gen_stub_pyclass]
#[pyclass(eq, frozen)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstanceClassMembershipReport(pub ommx::InstanceClassMembershipReport);

#[pyo3_stub_gen::derive::gen_stub_pymethods]
#[pymethods]
impl InstanceClassMembershipReport {
    #[getter]
    pub fn clause_reports(&self) -> Vec<InstanceClassClauseReport> {
        self.0
            .clause_reports()
            .iter()
            .cloned()
            .map(InstanceClassClauseReport)
            .collect()
    }

    #[getter]
    pub fn is_member(&self) -> bool {
        self.0.is_member()
    }

    #[getter]
    pub fn matching_clauses(&self) -> Vec<(usize, String)> {
        self.0
            .matching_clauses()
            .map(|(index, label)| (index, label.to_string()))
            .collect()
    }

    pub fn __str__(&self) -> String {
        self.0.to_string()
    }

    pub fn __repr__(&self) -> String {
        format!(
            "InstanceClassMembershipReport(is_member={}, matching_clauses={:?})",
            self.is_member(),
            self.matching_clauses()
        )
    }
}
