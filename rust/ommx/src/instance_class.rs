//! Instance classes for adapter boundaries.
//!
//! An [`InstanceClass`] is a set of [`crate::Instance`] values. Adapters use
//! one instance class to describe, in OMMX-defined structural terms, the
//! instances they can translate directly. An [`InstanceClassClause`] is one
//! conjunctive clause in the representation; an instance class is the finite
//! union of its clauses.
//!
//! Membership does not include preparation or lowering, adapter-specific
//! preconditions, wire-format `ommx.v2.Feature` handling, or successful
//! backend execution. Preparation produces another instance whose membership
//! must be checked again.

mod instance_facts;

use crate::{
    ConstraintID, Degree, Equality, IndicatorConstraintID, Instance, Kind, OneHotConstraintID,
    Sense, Sos1ConstraintID, VariableIDSet,
};
use instance_facts::{ConstraintFacts, InstanceFacts};
use std::collections::{BTreeMap, BTreeSet};

/// Cumulative polynomial-degree bound in an [`InstanceClassClause`].
///
/// `AtMost(n)` includes every polynomial degree up to and including `n`.
/// `Unbounded` includes every degree representable by the current
/// [`crate::Function`] domain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DegreeBound {
    AtMost(Degree),
    Unbounded,
}

impl DegreeBound {
    /// Construct an inclusive upper bound from a polynomial degree.
    pub fn at_most(degree: u32) -> Self {
        Self::AtMost(degree.into())
    }

    /// Return whether `actual` satisfies this bound.
    pub fn includes(self, actual: Degree) -> bool {
        match self {
            Self::AtMost(maximum) => actual <= maximum,
            Self::Unbounded => true,
        }
    }

    /// Return the inclusive upper bound, or `None` when unbounded.
    pub fn maximum(self) -> Option<Degree> {
        match self {
            Self::AtMost(maximum) => Some(maximum),
            Self::Unbounded => None,
        }
    }
}

impl std::fmt::Display for DegreeBound {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AtMost(maximum) => write!(f, "degree <= {maximum}"),
            Self::Unbounded => f.write_str("unbounded polynomial degree"),
        }
    }
}

/// Degree bounds for the allowed relations of one constraint family.
///
/// An empty value excludes every constraint in that family. Absence of one
/// [`Equality`] excludes that relation.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct RelationDegreeBounds(BTreeMap<Equality, DegreeBound>);

impl RelationDegreeBounds {
    fn new() -> Self {
        Self::default()
    }

    fn with(mut self, relation: Equality, bound: DegreeBound) -> Self {
        self.0.insert(relation, bound);
        self
    }

    fn bound_for(&self, relation: Equality) -> Option<DegreeBound> {
        self.0.get(&relation).copied()
    }

    fn relations(&self) -> BTreeSet<Equality> {
        self.0.keys().copied().collect()
    }

    fn iter(&self) -> impl Iterator<Item = (Equality, DegreeBound)> + '_ {
        self.0.iter().map(|(relation, bound)| (*relation, *bound))
    }

    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl FromIterator<(Equality, DegreeBound)> for RelationDegreeBounds {
    fn from_iter<T: IntoIterator<Item = (Equality, DegreeBound)>>(iter: T) -> Self {
        Self(iter.into_iter().collect())
    }
}

/// One conjunctive clause in an [`InstanceClass`].
///
/// Every condition in a clause must hold for an instance to belong to that
/// clause. The containing [`InstanceClass`] is the union of its clauses, so a
/// continuous-QP clause and a MILP clause do not imply MIQP membership.
///
/// `allowed_variable_kinds` is a set rather than one field per known [`Kind`],
/// allowing future kinds such as Spin to participate without changing the
/// clause shape.
#[derive(Debug, Clone)]
pub struct InstanceClassClause {
    label: String,
    allowed_variable_kinds: BTreeSet<Kind>,
    objective_degree_bound: DegreeBound,
    regular_constraints: RelationDegreeBounds,
    indicator_constraints: RelationDegreeBounds,
    allows_one_hot: bool,
    allows_sos1: bool,
    allowed_senses: BTreeSet<Sense>,
}

impl InstanceClassClause {
    /// Construct a clause with no allowed constraint families.
    ///
    /// An empty `allowed_senses` set makes the clause contain no instances.
    /// `label` is diagnostic metadata and need not be unique.
    pub fn new(
        label: impl Into<String>,
        allowed_variable_kinds: BTreeSet<Kind>,
        objective_degree_bound: DegreeBound,
        allowed_senses: BTreeSet<Sense>,
    ) -> Self {
        Self {
            label: label.into(),
            allowed_variable_kinds,
            objective_degree_bound,
            regular_constraints: RelationDegreeBounds::new(),
            indicator_constraints: RelationDegreeBounds::new(),
            allows_one_hot: false,
            allows_sos1: false,
            allowed_senses,
        }
    }

    /// Include one regular-constraint relation up to `degree_bound`.
    pub fn with_regular_constraint(
        mut self,
        relation: Equality,
        degree_bound: DegreeBound,
    ) -> Self {
        self.regular_constraints = self.regular_constraints.with(relation, degree_bound);
        self
    }

    /// Include one Indicator body relation up to `degree_bound`.
    pub fn with_indicator_constraint(
        mut self,
        relation: Equality,
        degree_bound: DegreeBound,
    ) -> Self {
        self.indicator_constraints = self.indicator_constraints.with(relation, degree_bound);
        self
    }

    /// Include instances with active OneHot constraints.
    pub fn with_one_hot(mut self) -> Self {
        self.allows_one_hot = true;
        self
    }

    /// Include instances with active SOS1 constraints.
    pub fn with_sos1(mut self) -> Self {
        self.allows_sos1 = true;
        self
    }

    /// Human-readable diagnostic label. Labels do not affect membership and
    /// need not be unique.
    /// Return the diagnostic label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Return the variable kinds admitted by this clause.
    pub fn allowed_variable_kinds(&self) -> &BTreeSet<Kind> {
        &self.allowed_variable_kinds
    }

    /// Return the objective's cumulative degree bound.
    pub fn objective_degree_bound(&self) -> DegreeBound {
        self.objective_degree_bound
    }

    /// Iterate over admitted regular-constraint relations and degree bounds.
    pub fn regular_constraint_degree_bounds(
        &self,
    ) -> impl Iterator<Item = (Equality, DegreeBound)> + '_ {
        self.regular_constraints.iter()
    }

    /// Iterate over admitted Indicator-body relations and degree bounds.
    pub fn indicator_constraint_degree_bounds(
        &self,
    ) -> impl Iterator<Item = (Equality, DegreeBound)> + '_ {
        self.indicator_constraints.iter()
    }

    /// Return whether active OneHot constraints are admitted.
    pub fn allows_one_hot(&self) -> bool {
        self.allows_one_hot
    }

    /// Return whether active SOS1 constraints are admitted.
    pub fn allows_sos1(&self) -> bool {
        self.allows_sos1
    }

    /// Return the optimization senses admitted by this clause.
    pub fn allowed_senses(&self) -> &BTreeSet<Sense> {
        &self.allowed_senses
    }

    fn check(&self, clause_index: usize, facts: &InstanceFacts) -> InstanceClassClauseReport {
        let mut mismatches = Vec::new();

        for (kind, variable_ids) in facts.used_variables_by_kind() {
            if !self.allowed_variable_kinds.contains(kind) {
                mismatches.push(InstanceClassMismatch::VariableKindNotAllowed {
                    kind: *kind,
                    variable_ids: variable_ids.clone(),
                    allowed_kinds: self.allowed_variable_kinds.clone(),
                });
            }
        }

        if !self
            .objective_degree_bound
            .includes(facts.objective_degree())
        {
            mismatches.push(InstanceClassMismatch::ObjectiveDegreeExceedsBound {
                actual_degree: facts.objective_degree(),
                bound: self.objective_degree_bound,
            });
        }

        check_regular_constraints(
            &mut mismatches,
            facts.regular_constraints(),
            &self.regular_constraints,
        );
        check_indicator_constraints(
            &mut mismatches,
            facts.indicator_constraints(),
            &self.indicator_constraints,
        );

        if !self.allows_one_hot && !facts.one_hot_constraint_ids().is_empty() {
            mismatches.push(InstanceClassMismatch::OneHotConstraintsNotAllowed {
                constraint_ids: facts.one_hot_constraint_ids().clone(),
            });
        }
        if !self.allows_sos1 && !facts.sos1_constraint_ids().is_empty() {
            mismatches.push(InstanceClassMismatch::Sos1ConstraintsNotAllowed {
                constraint_ids: facts.sos1_constraint_ids().clone(),
            });
        }
        if !self.allowed_senses.contains(&facts.sense()) {
            mismatches.push(InstanceClassMismatch::SenseNotAllowed {
                sense: facts.sense(),
                allowed_senses: self.allowed_senses.clone(),
            });
        }

        InstanceClassClauseReport {
            clause_index,
            clause_label: self.label.clone(),
            mismatches,
        }
    }
}

fn group_constraint_facts<ID: Copy + Ord>(
    facts: &BTreeMap<ID, ConstraintFacts>,
) -> BTreeMap<Equality, BTreeMap<ID, Degree>> {
    let mut grouped = BTreeMap::<Equality, BTreeMap<ID, Degree>>::new();
    for (id, fact) in facts {
        grouped
            .entry(fact.relation())
            .or_default()
            .insert(*id, fact.degree());
    }
    grouped
}

fn check_regular_constraints(
    mismatches: &mut Vec<InstanceClassMismatch>,
    facts: &BTreeMap<ConstraintID, ConstraintFacts>,
    allowed: &RelationDegreeBounds,
) {
    for (relation, constraints) in group_constraint_facts(facts) {
        let Some(bound) = allowed.bound_for(relation) else {
            mismatches.push(InstanceClassMismatch::RegularConstraintRelationNotAllowed {
                relation,
                constraint_ids: constraints.keys().copied().collect(),
                allowed_relations: allowed.relations(),
            });
            continue;
        };
        let actual_degrees = constraints
            .into_iter()
            .filter(|(_, degree)| !bound.includes(*degree))
            .collect::<BTreeMap<_, _>>();
        if !actual_degrees.is_empty() {
            mismatches.push(InstanceClassMismatch::RegularConstraintDegreeExceedsBound {
                relation,
                actual_degrees,
                bound,
            });
        }
    }
}

fn check_indicator_constraints(
    mismatches: &mut Vec<InstanceClassMismatch>,
    facts: &BTreeMap<IndicatorConstraintID, ConstraintFacts>,
    allowed: &RelationDegreeBounds,
) {
    if facts.is_empty() {
        return;
    }
    if allowed.is_empty() {
        mismatches.push(InstanceClassMismatch::IndicatorConstraintsNotAllowed {
            constraint_ids: facts.keys().copied().collect(),
        });
        return;
    }
    for (relation, constraints) in group_constraint_facts(facts) {
        let Some(bound) = allowed.bound_for(relation) else {
            mismatches.push(
                InstanceClassMismatch::IndicatorConstraintRelationNotAllowed {
                    relation,
                    constraint_ids: constraints.keys().copied().collect(),
                    allowed_relations: allowed.relations(),
                },
            );
            continue;
        };
        let actual_degrees = constraints
            .into_iter()
            .filter(|(_, degree)| !bound.includes(*degree))
            .collect::<BTreeMap<_, _>>();
        if !actual_degrees.is_empty() {
            mismatches.push(InstanceClassMismatch::IndicatorBodyDegreeExceedsBound {
                relation,
                actual_degrees,
                bound,
            });
        }
    }
}

/// A set of [`Instance`] values represented as a finite union of
/// [`InstanceClassClause`] values.
///
/// The empty clause list represents the empty class. Clause order, duplicate
/// clauses, and clause labels are representation details rather than
/// extensional class identity, so this type deliberately does not implement
/// [`PartialEq`] or [`Eq`].
#[derive(Debug, Clone, Default)]
pub struct InstanceClass {
    clauses: Vec<InstanceClassClause>,
}

impl InstanceClass {
    /// Construct the finite union of `clauses`.
    pub fn new(clauses: Vec<InstanceClassClause>) -> Self {
        Self { clauses }
    }

    /// Return the clauses representing this class.
    pub fn clauses(&self) -> &[InstanceClassClause] {
        &self.clauses
    }

    /// Return the finite union of `self` and `other`.
    pub fn union(mut self, other: Self) -> Self {
        self.clauses.extend(other.clauses);
        self
    }

    /// Return whether `instance` belongs to this class.
    pub fn contains(&self, instance: &Instance) -> bool {
        self.check_membership(instance).is_member()
    }

    /// Evaluate membership without mutating or preparing `instance`.
    ///
    /// Facts are derived from the exact instance on every call. Callers that
    /// prepare an instance must check membership again on the prepared value.
    pub fn check_membership(&self, instance: &Instance) -> InstanceClassMembershipReport {
        let facts = InstanceFacts::from(instance);
        InstanceClassMembershipReport {
            clause_reports: self
                .clauses
                .iter()
                .enumerate()
                .map(|(index, clause)| clause.check(index, &facts))
                .collect(),
        }
    }
}

impl From<InstanceClassClause> for InstanceClass {
    fn from(clause: InstanceClassClause) -> Self {
        Self::new(vec![clause])
    }
}

impl FromIterator<InstanceClassClause> for InstanceClass {
    fn from_iter<T: IntoIterator<Item = InstanceClassClause>>(iter: T) -> Self {
        Self::new(iter.into_iter().collect())
    }
}

/// One reason an [`Instance`] does not belong to an
/// [`InstanceClassClause`].
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstanceClassMismatch {
    VariableKindNotAllowed {
        kind: Kind,
        variable_ids: VariableIDSet,
        allowed_kinds: BTreeSet<Kind>,
    },
    ObjectiveDegreeExceedsBound {
        actual_degree: Degree,
        bound: DegreeBound,
    },
    RegularConstraintRelationNotAllowed {
        relation: Equality,
        constraint_ids: BTreeSet<ConstraintID>,
        allowed_relations: BTreeSet<Equality>,
    },
    RegularConstraintDegreeExceedsBound {
        relation: Equality,
        actual_degrees: BTreeMap<ConstraintID, Degree>,
        bound: DegreeBound,
    },
    IndicatorConstraintsNotAllowed {
        constraint_ids: BTreeSet<IndicatorConstraintID>,
    },
    IndicatorConstraintRelationNotAllowed {
        relation: Equality,
        constraint_ids: BTreeSet<IndicatorConstraintID>,
        allowed_relations: BTreeSet<Equality>,
    },
    IndicatorBodyDegreeExceedsBound {
        relation: Equality,
        actual_degrees: BTreeMap<IndicatorConstraintID, Degree>,
        bound: DegreeBound,
    },
    OneHotConstraintsNotAllowed {
        constraint_ids: BTreeSet<OneHotConstraintID>,
    },
    Sos1ConstraintsNotAllowed {
        constraint_ids: BTreeSet<Sos1ConstraintID>,
    },
    SenseNotAllowed {
        sense: Sense,
        allowed_senses: BTreeSet<Sense>,
    },
}

impl std::fmt::Display for InstanceClassMismatch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::VariableKindNotAllowed {
                kind,
                variable_ids,
                allowed_kinds,
            } => write!(
                f,
                "variable kind {kind:?} for IDs {variable_ids:?} is not allowed; allowed kinds are {allowed_kinds:?}"
            ),
            Self::ObjectiveDegreeExceedsBound {
                actual_degree,
                bound,
            } => write!(f, "objective degree {actual_degree} exceeds {bound}"),
            Self::RegularConstraintRelationNotAllowed {
                relation,
                constraint_ids,
                allowed_relations,
            } => write!(
                f,
                "regular-constraint relation {relation:?} for IDs {constraint_ids:?} is not allowed; allowed relations are {allowed_relations:?}"
            ),
            Self::RegularConstraintDegreeExceedsBound {
                relation,
                actual_degrees,
                bound,
            } => write!(
                f,
                "regular {relation:?} constraint degrees {actual_degrees:?} exceed {bound}"
            ),
            Self::IndicatorConstraintsNotAllowed { constraint_ids } => {
                write!(f, "indicator constraints {constraint_ids:?} are not allowed")
            }
            Self::IndicatorConstraintRelationNotAllowed {
                relation,
                constraint_ids,
                allowed_relations,
            } => write!(
                f,
                "indicator relation {relation:?} for IDs {constraint_ids:?} is not allowed; allowed relations are {allowed_relations:?}"
            ),
            Self::IndicatorBodyDegreeExceedsBound {
                relation,
                actual_degrees,
                bound,
            } => write!(
                f,
                "indicator {relation:?} body degrees {actual_degrees:?} exceed {bound}"
            ),
            Self::OneHotConstraintsNotAllowed { constraint_ids } => {
                write!(f, "one-hot constraints {constraint_ids:?} are not allowed")
            }
            Self::Sos1ConstraintsNotAllowed { constraint_ids } => {
                write!(f, "SOS1 constraints {constraint_ids:?} are not allowed")
            }
            Self::SenseNotAllowed {
                sense,
                allowed_senses,
            } => write!(
                f,
                "optimization sense {sense:?} is not allowed; allowed senses are {allowed_senses:?}"
            ),
        }
    }
}

/// Membership result for one conjunctive clause.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstanceClassClauseReport {
    clause_index: usize,
    clause_label: String,
    mismatches: Vec<InstanceClassMismatch>,
}

impl InstanceClassClauseReport {
    /// Return the clause's position in its containing class.
    pub fn clause_index(&self) -> usize {
        self.clause_index
    }

    /// Return the clause's diagnostic label.
    pub fn clause_label(&self) -> &str {
        &self.clause_label
    }

    /// Return every reason the instance is outside this clause.
    pub fn mismatches(&self) -> &[InstanceClassMismatch] {
        &self.mismatches
    }

    /// Return whether the instance belongs to this clause.
    pub fn is_member(&self) -> bool {
        self.mismatches.is_empty()
    }
}

/// Side-effect-free [`InstanceClass`] membership report.
///
/// An instance is a member when at least one complete clause contains it.
/// Adapter identity and adapter-specific preconditions belong to an adapter
/// applicability report layered on top of this result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstanceClassMembershipReport {
    clause_reports: Vec<InstanceClassClauseReport>,
}

impl InstanceClassMembershipReport {
    /// Return one membership report per class clause, in clause order.
    pub fn clause_reports(&self) -> &[InstanceClassClauseReport] {
        &self.clause_reports
    }

    /// Return whether at least one clause contains the instance.
    pub fn is_member(&self) -> bool {
        self.clause_reports
            .iter()
            .any(InstanceClassClauseReport::is_member)
    }

    /// Iterate over the indices and labels of clauses containing the instance.
    pub fn matching_clauses(&self) -> impl Iterator<Item = (usize, &str)> {
        self.clause_reports
            .iter()
            .filter(|clause| clause.is_member())
            .map(|clause| (clause.clause_index(), clause.clause_label()))
    }
}

impl std::fmt::Display for InstanceClassMembershipReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_member() {
            let clauses = self
                .matching_clauses()
                .map(|(index, label)| format!("{index}: `{label}`"))
                .collect::<Vec<_>>()
                .join(", ");
            return write!(f, "Instance belongs via clause(s) {clauses}");
        }
        writeln!(f, "Instance does not belong to any clause:")?;
        for clause in &self.clause_reports {
            writeln!(
                f,
                "- clause {} (`{}`):",
                clause.clause_index, clause.clause_label
            )?;
            for mismatch in &clause.mismatches {
                writeln!(f, "  - {mismatch}")?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        linear, quadratic, Constraint, DecisionVariable, Function, IndicatorConstraint,
        InstanceParameters, OneHotConstraint, OneHotConstraintID, Sos1Constraint, Sos1ConstraintID,
        VariableID,
    };
    use proptest::prelude::*;

    fn clause(
        label: &str,
        allowed_variable_kinds: &[Kind],
        objective_degree_bound: DegreeBound,
    ) -> InstanceClassClause {
        InstanceClassClause::new(
            label,
            allowed_variable_kinds.iter().copied().collect(),
            objective_degree_bound,
            BTreeSet::from([Sense::Minimize, Sense::Maximize]),
        )
    }

    fn covering_clause(facts: &InstanceFacts) -> InstanceClassClause {
        let mut clause = InstanceClassClause::new(
            "covering",
            facts.used_variables_by_kind().keys().copied().collect(),
            DegreeBound::AtMost(facts.objective_degree()),
            BTreeSet::from([facts.sense()]),
        );

        let mut regular = BTreeMap::<Equality, Degree>::new();
        for fact in facts.regular_constraints().values() {
            regular
                .entry(fact.relation())
                .and_modify(|degree| *degree = (*degree).max(fact.degree()))
                .or_insert(fact.degree());
        }
        for (relation, degree) in regular {
            clause = clause.with_regular_constraint(relation, DegreeBound::AtMost(degree));
        }

        let mut indicator = BTreeMap::<Equality, Degree>::new();
        for fact in facts.indicator_constraints().values() {
            indicator
                .entry(fact.relation())
                .and_modify(|degree| *degree = (*degree).max(fact.degree()))
                .or_insert(fact.degree());
        }
        for (relation, degree) in indicator {
            clause = clause.with_indicator_constraint(relation, DegreeBound::AtMost(degree));
        }
        if !facts.one_hot_constraint_ids().is_empty() {
            clause = clause.with_one_hot();
        }
        if !facts.sos1_constraint_ids().is_empty() {
            clause = clause.with_sos1();
        }
        clause
    }

    #[test]
    fn degree_bound_is_cumulative_and_inclusive() {
        let linear = DegreeBound::at_most(1);
        assert!(linear.includes(0.into()));
        assert!(linear.includes(1.into()));
        assert!(!linear.includes(2.into()));
        assert_eq!(linear.maximum(), Some(1.into()));
        assert!(DegreeBound::Unbounded.includes(10_000.into()));
        assert_eq!(DegreeBound::Unbounded.maximum(), None);
    }

    #[test]
    fn union_does_not_combine_conditions_across_clauses() {
        let x = VariableID::from(1);
        let instance = crate::Instance::new(
            Sense::Minimize,
            Function::Quadratic(quadratic!(x, x).into()),
            BTreeMap::from([(x, DecisionVariable::binary())]),
            BTreeMap::new(),
        )
        .unwrap();

        let milp = clause(
            "milp",
            &[Kind::Binary, Kind::Integer, Kind::Continuous],
            DegreeBound::at_most(1),
        )
        .with_regular_constraint(Equality::EqualToZero, DegreeBound::at_most(1))
        .with_regular_constraint(Equality::LessThanOrEqualToZero, DegreeBound::at_most(1));
        let continuous_qp = clause(
            "continuous-qp",
            &[Kind::Continuous],
            DegreeBound::at_most(2),
        )
        .with_regular_constraint(Equality::EqualToZero, DegreeBound::at_most(1))
        .with_regular_constraint(Equality::LessThanOrEqualToZero, DegreeBound::at_most(1));
        let instance_class = InstanceClass::new(vec![milp, continuous_qp]);

        let report = instance_class.check_membership(&instance);
        assert!(!report.is_member());
        assert!(matches!(
            report.clause_reports()[0].mismatches(),
            [InstanceClassMismatch::ObjectiveDegreeExceedsBound { .. }]
        ));
        assert!(matches!(
            report.clause_reports()[1].mismatches(),
            [InstanceClassMismatch::VariableKindNotAllowed { .. }]
        ));
        assert!(!instance_class.contains(&instance));
    }

    #[test]
    fn structured_mismatches_preserve_relations_degrees_and_ids() {
        let x = VariableID::from(1);
        let y = VariableID::from(2);
        let regular_eq = ConstraintID::from(10);
        let regular_le = ConstraintID::from(11);
        let indicator_eq = IndicatorConstraintID::from(20);
        let indicator_le = IndicatorConstraintID::from(21);
        let one_hot = OneHotConstraintID::from(30);
        let sos1 = Sos1ConstraintID::from(40);
        let quadratic_y = || Function::Quadratic(quadratic!(y, y).into());
        let instance = crate::Instance::builder()
            .sense(Sense::Maximize)
            .objective(Function::Quadratic(quadratic!(x, y).into()))
            .decision_variables(BTreeMap::from([
                (x, DecisionVariable::binary()),
                (y, DecisionVariable::continuous()),
            ]))
            .constraints(BTreeMap::from([
                (regular_eq, Constraint::equal_to_zero(quadratic_y())),
                (
                    regular_le,
                    Constraint::less_than_or_equal_to_zero(Function::from(linear!(y))),
                ),
            ]))
            .indicator_constraints(BTreeMap::from([
                (
                    indicator_eq,
                    IndicatorConstraint::new(x, Equality::EqualToZero, quadratic_y()),
                ),
                (
                    indicator_le,
                    IndicatorConstraint::new(
                        x,
                        Equality::LessThanOrEqualToZero,
                        Function::from(linear!(y)),
                    ),
                ),
            ]))
            .one_hot_constraints(BTreeMap::from([(
                one_hot,
                OneHotConstraint::new(BTreeSet::from([x])).unwrap(),
            )]))
            .sos1_constraints(BTreeMap::from([(
                sos1,
                Sos1Constraint::new(BTreeSet::from([y])).unwrap(),
            )]))
            .build()
            .unwrap();

        // Compare serialized bytes to make the side-effect-free contract
        // observable across the complete instance.
        let before = instance.to_v2_bytes();
        let limited = InstanceClassClause::new(
            "limited",
            BTreeSet::from([Kind::Binary]),
            DegreeBound::at_most(1),
            BTreeSet::from([Sense::Minimize]),
        )
        .with_regular_constraint(Equality::EqualToZero, DegreeBound::at_most(1))
        .with_indicator_constraint(Equality::EqualToZero, DegreeBound::at_most(1));
        let report = InstanceClass::new(vec![limited]).check_membership(&instance);

        assert!(!report.is_member());
        assert_eq!(
            report.clause_reports()[0].mismatches(),
            &[
                InstanceClassMismatch::VariableKindNotAllowed {
                    kind: Kind::Continuous,
                    variable_ids: BTreeSet::from([y]),
                    allowed_kinds: BTreeSet::from([Kind::Binary]),
                },
                InstanceClassMismatch::ObjectiveDegreeExceedsBound {
                    actual_degree: 2.into(),
                    bound: DegreeBound::at_most(1),
                },
                InstanceClassMismatch::RegularConstraintDegreeExceedsBound {
                    relation: Equality::EqualToZero,
                    actual_degrees: BTreeMap::from([(regular_eq, 2.into())]),
                    bound: DegreeBound::at_most(1),
                },
                InstanceClassMismatch::RegularConstraintRelationNotAllowed {
                    relation: Equality::LessThanOrEqualToZero,
                    constraint_ids: BTreeSet::from([regular_le]),
                    allowed_relations: BTreeSet::from([Equality::EqualToZero]),
                },
                InstanceClassMismatch::IndicatorBodyDegreeExceedsBound {
                    relation: Equality::EqualToZero,
                    actual_degrees: BTreeMap::from([(indicator_eq, 2.into())]),
                    bound: DegreeBound::at_most(1),
                },
                InstanceClassMismatch::IndicatorConstraintRelationNotAllowed {
                    relation: Equality::LessThanOrEqualToZero,
                    constraint_ids: BTreeSet::from([indicator_le]),
                    allowed_relations: BTreeSet::from([Equality::EqualToZero]),
                },
                InstanceClassMismatch::OneHotConstraintsNotAllowed {
                    constraint_ids: BTreeSet::from([one_hot]),
                },
                InstanceClassMismatch::Sos1ConstraintsNotAllowed {
                    constraint_ids: BTreeSet::from([sos1]),
                },
                InstanceClassMismatch::SenseNotAllowed {
                    sense: Sense::Maximize,
                    allowed_senses: BTreeSet::from([Sense::Minimize]),
                },
            ]
        );
        assert_eq!(instance.to_v2_bytes(), before);
    }

    #[test]
    fn omitted_constraint_relations_include_unconstrained_instances() {
        let x = VariableID::from(1);
        let instance = crate::Instance::new(
            Sense::Maximize,
            Function::Quadratic(quadratic!(x, x).into()),
            BTreeMap::from([(x, DecisionVariable::binary())]),
            BTreeMap::new(),
        )
        .unwrap();
        let qubo = clause("qubo", &[Kind::Binary], DegreeBound::at_most(2));
        let report = InstanceClass::from(qubo).check_membership(&instance);
        assert!(report.is_member());
        assert_eq!(report.matching_clauses().collect::<Vec<_>>(), [(0, "qubo")]);
    }

    #[test]
    fn membership_is_recomputed_after_explicit_lowering() {
        let x = VariableID::from(1);
        let y = VariableID::from(2);
        let one_hot_id = OneHotConstraintID::from(7);
        let mut instance = crate::Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::from((linear!(x) + linear!(y)).unwrap()))
            .decision_variables(BTreeMap::from([
                (x, DecisionVariable::binary()),
                (y, DecisionVariable::binary()),
            ]))
            .constraints(BTreeMap::new())
            .one_hot_constraints(BTreeMap::from([(
                one_hot_id,
                OneHotConstraint::new(BTreeSet::from([x, y])).unwrap(),
            )]))
            .build()
            .unwrap();
        let linear_binary = InstanceClass::from(
            clause("linear-binary", &[Kind::Binary], DegreeBound::at_most(1))
                .with_regular_constraint(Equality::EqualToZero, DegreeBound::at_most(1)),
        );

        assert!(!linear_binary.contains(&instance));
        instance.convert_one_hot_to_constraint(one_hot_id).unwrap();
        assert!(linear_binary.contains(&instance));
    }

    #[test]
    fn empty_class_and_empty_clause_represent_empty_sets() {
        let instance = crate::Instance::new(
            Sense::Minimize,
            Function::zero(),
            BTreeMap::new(),
            BTreeMap::new(),
        )
        .unwrap();
        assert!(!InstanceClass::default().contains(&instance));

        let empty_clause = InstanceClassClause::new(
            "empty",
            BTreeSet::new(),
            DegreeBound::Unbounded,
            BTreeSet::new(),
        );
        assert!(!InstanceClass::from(empty_clause).contains(&instance));
    }

    #[test]
    fn union_is_disjunction_and_duplicate_labels_are_diagnostic_only() {
        let x = VariableID::from(1);
        let binary = crate::Instance::new(
            Sense::Minimize,
            Function::from(linear!(x)),
            BTreeMap::from([(x, DecisionVariable::binary())]),
            BTreeMap::new(),
        )
        .unwrap();
        let continuous = crate::Instance::new(
            Sense::Minimize,
            Function::from(linear!(x)),
            BTreeMap::from([(x, DecisionVariable::continuous())]),
            BTreeMap::new(),
        )
        .unwrap();
        let binary_class =
            InstanceClass::from(clause("linear", &[Kind::Binary], DegreeBound::at_most(1)));
        let continuous_class = InstanceClass::from(clause(
            "linear",
            &[Kind::Continuous],
            DegreeBound::at_most(1),
        ));
        let union = binary_class.clone().union(continuous_class.clone());

        assert!(union.contains(&binary));
        assert!(union.contains(&continuous));
        assert!(InstanceClass::default()
            .union(binary_class)
            .contains(&binary));
        assert!(continuous_class
            .union(InstanceClass::default())
            .contains(&continuous));
        assert_eq!(
            union
                .check_membership(&binary)
                .matching_clauses()
                .collect::<Vec<_>>(),
            [(0, "linear")]
        );
        assert_eq!(
            union
                .check_membership(&continuous)
                .matching_clauses()
                .collect::<Vec<_>>(),
            [(1, "linear")]
        );
    }

    proptest! {
        #[test]
        fn union_membership_is_disjunction(
            instance in any_with::<crate::Instance>(InstanceParameters::full_v3())
        ) {
            let binary = InstanceClass::from(clause(
                "binary-linear",
                &[Kind::Binary],
                DegreeBound::at_most(1),
            ));
            let continuous = InstanceClass::from(clause(
                "continuous-quadratic",
                &[Kind::Continuous],
                DegreeBound::at_most(2),
            ));
            let expected = binary.contains(&instance) || continuous.contains(&instance);
            let union = binary.union(continuous);

            prop_assert_eq!(union.contains(&instance), expected);
        }

        #[test]
        fn covering_clause_contains_every_current_instance(
            instance in any_with::<crate::Instance>(InstanceParameters::full_v3())
        ) {
            let facts = InstanceFacts::from(&instance);
            let instance_class = InstanceClass::from(covering_clause(&facts));
            let report = instance_class.check_membership(&instance);
            prop_assert!(report.is_member(), "{report}");
            prop_assert!(instance_class.contains(&instance));
        }
    }
}
