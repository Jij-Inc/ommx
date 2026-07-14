use crate::{
    ConstraintID, ConstraintRequirement, Degree, Equality, IndicatorConstraintID,
    InstanceRequirements, Kind, OneHotConstraintID, Sense, Sos1ConstraintID, VariableIDSet,
};
use std::collections::{BTreeMap, BTreeSet};

/// Cumulative polynomial-degree support declared by a capability profile.
///
/// `AtMost(n)` accepts every polynomial degree up to and including `n`; it is
/// not an exact-degree requirement. `Any` accepts every degree representable by
/// the current [`crate::Function`] domain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DegreeLimit {
    AtMost(Degree),
    Any,
}

impl DegreeLimit {
    pub fn at_most(degree: u32) -> Self {
        Self::AtMost(degree.into())
    }

    pub fn allows(self, actual: Degree) -> bool {
        match self {
            Self::AtMost(maximum) => actual <= maximum,
            Self::Any => true,
        }
    }

    pub fn maximum(self) -> Option<Degree> {
        match self {
            Self::AtMost(maximum) => Some(maximum),
            Self::Any => None,
        }
    }
}

impl std::fmt::Display for DegreeLimit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AtMost(maximum) => write!(f, "degree <= {maximum}"),
            Self::Any => f.write_str("any polynomial degree"),
        }
    }
}

/// Degree limits for the supported relations of one constraint family.
///
/// An empty value means that the profile supports no constraint of that
/// family. Absence of one [`Equality`] means that relation is unsupported.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RelationDegreeLimits(BTreeMap<Equality, DegreeLimit>);

impl RelationDegreeLimits {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with(mut self, relation: Equality, limit: DegreeLimit) -> Self {
        self.0.insert(relation, limit);
        self
    }

    pub fn limit_for(&self, relation: Equality) -> Option<DegreeLimit> {
        self.0.get(&relation).copied()
    }

    pub fn relations(&self) -> BTreeSet<Equality> {
        self.0.keys().copied().collect()
    }

    pub fn iter(&self) -> impl Iterator<Item = (Equality, DegreeLimit)> + '_ {
        self.0.iter().map(|(relation, limit)| (*relation, *limit))
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl FromIterator<(Equality, DegreeLimit)> for RelationDegreeLimits {
    fn from_iter<T: IntoIterator<Item = (Equality, DegreeLimit)>>(iter: T) -> Self {
        Self(iter.into_iter().collect())
    }
}

/// One coherent combination of native solver capabilities.
///
/// Profiles are conjunctions of all their fields. [`AdapterCapabilities`]
/// treats multiple profiles as alternatives, so a continuous-QP profile and a
/// MILP profile do not accidentally imply MIQP support.
///
/// A profile describes the model shape accepted directly by a solver
/// translator after any explicit preparation. Acceptance through an exact
/// reformulation, relaxation, or heuristic/finite-penalty conversion is not a
/// native capability and belongs to a separate preparation layer. Derive fresh
/// [`InstanceRequirements`] and check them again after preparation.
///
/// `variable_kinds` is a set rather than one field per known [`Kind`], allowing
/// future kinds such as Spin to participate without changing this shape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityProfile {
    name: String,
    variable_kinds: BTreeSet<Kind>,
    objective_degree: DegreeLimit,
    regular_constraints: RelationDegreeLimits,
    indicator_constraints: RelationDegreeLimits,
    supports_one_hot: bool,
    supports_sos1: bool,
    senses: BTreeSet<Sense>,
}

impl CapabilityProfile {
    pub fn new(
        name: impl Into<String>,
        variable_kinds: BTreeSet<Kind>,
        objective_degree: DegreeLimit,
        senses: BTreeSet<Sense>,
    ) -> Result<Self, CapabilityDefinitionError> {
        let name = name.into();
        if name.trim().is_empty() {
            return Err(CapabilityDefinitionError::EmptyProfileName);
        }
        if senses.is_empty() {
            return Err(CapabilityDefinitionError::EmptySupportedSenses { profile_name: name });
        }
        Ok(Self {
            name,
            variable_kinds,
            objective_degree,
            regular_constraints: RelationDegreeLimits::new(),
            indicator_constraints: RelationDegreeLimits::new(),
            supports_one_hot: false,
            supports_sos1: false,
            senses,
        })
    }

    /// Declare direct translator support for one regular-constraint relation
    /// and degree limit.
    pub fn with_regular_constraint(mut self, relation: Equality, limit: DegreeLimit) -> Self {
        self.regular_constraints = self.regular_constraints.with(relation, limit);
        self
    }

    /// Declare native Indicator support for one body relation and degree limit.
    ///
    /// An adapter that first lowers Indicator constraints must not declare them
    /// here; it must recheck the lowered instance instead.
    pub fn with_indicator_constraint(mut self, relation: Equality, limit: DegreeLimit) -> Self {
        self.indicator_constraints = self.indicator_constraints.with(relation, limit);
        self
    }

    /// Declare native OneHot support in the direct translator input.
    ///
    /// An adapter that first lowers OneHot constraints must not set this flag;
    /// it must recheck the lowered instance instead.
    pub fn with_one_hot(mut self) -> Self {
        self.supports_one_hot = true;
        self
    }

    /// Declare native SOS1 support in the direct translator input.
    ///
    /// An adapter that first lowers SOS1 constraints must not set this flag;
    /// it must recheck the lowered instance instead.
    pub fn with_sos1(mut self) -> Self {
        self.supports_sos1 = true;
        self
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn variable_kinds(&self) -> &BTreeSet<Kind> {
        &self.variable_kinds
    }

    pub fn objective_degree(&self) -> DegreeLimit {
        self.objective_degree
    }

    pub fn regular_constraints(&self) -> &RelationDegreeLimits {
        &self.regular_constraints
    }

    pub fn indicator_constraints(&self) -> &RelationDegreeLimits {
        &self.indicator_constraints
    }

    pub fn supports_one_hot(&self) -> bool {
        self.supports_one_hot
    }

    pub fn supports_sos1(&self) -> bool {
        self.supports_sos1
    }

    pub fn senses(&self) -> &BTreeSet<Sense> {
        &self.senses
    }

    fn check(&self, requirements: &InstanceRequirements) -> ProfileCompatibilityReport {
        let mut mismatches = Vec::new();

        for (kind, used_variable_ids) in requirements.used_variables_by_kind() {
            if !self.variable_kinds.contains(kind) {
                mismatches.push(PortableCapabilityMismatch::UnsupportedVariableKind {
                    kind: *kind,
                    used_variable_ids: used_variable_ids.clone(),
                    supported_kinds: self.variable_kinds.clone(),
                });
            }
        }

        if !self
            .objective_degree
            .allows(requirements.objective_degree())
        {
            mismatches.push(PortableCapabilityMismatch::ObjectiveDegreeExceeded {
                actual_degree: requirements.objective_degree(),
                limit: self.objective_degree,
            });
        }

        check_regular_constraints(
            &mut mismatches,
            requirements.regular_constraints(),
            &self.regular_constraints,
        );
        check_indicator_constraints(
            &mut mismatches,
            requirements.indicator_constraints(),
            &self.indicator_constraints,
        );

        if !self.supports_one_hot && !requirements.one_hot_constraint_ids().is_empty() {
            mismatches.push(PortableCapabilityMismatch::UnsupportedOneHotConstraints {
                constraint_ids: requirements.one_hot_constraint_ids().clone(),
            });
        }
        if !self.supports_sos1 && !requirements.sos1_constraint_ids().is_empty() {
            mismatches.push(PortableCapabilityMismatch::UnsupportedSos1Constraints {
                constraint_ids: requirements.sos1_constraint_ids().clone(),
            });
        }
        if !self.senses.contains(&requirements.sense()) {
            mismatches.push(PortableCapabilityMismatch::UnsupportedSense {
                sense: requirements.sense(),
                supported_senses: self.senses.clone(),
            });
        }

        ProfileCompatibilityReport {
            profile_name: self.name.clone(),
            mismatches,
        }
    }
}

fn group_constraint_requirements<ID: Copy + Ord>(
    requirements: &BTreeMap<ID, ConstraintRequirement>,
) -> BTreeMap<Equality, BTreeMap<ID, Degree>> {
    let mut grouped = BTreeMap::<Equality, BTreeMap<ID, Degree>>::new();
    for (id, requirement) in requirements {
        grouped
            .entry(requirement.relation())
            .or_default()
            .insert(*id, requirement.degree());
    }
    grouped
}

fn check_regular_constraints(
    mismatches: &mut Vec<PortableCapabilityMismatch>,
    requirements: &BTreeMap<ConstraintID, ConstraintRequirement>,
    supported: &RelationDegreeLimits,
) {
    for (relation, constraints) in group_constraint_requirements(requirements) {
        let Some(limit) = supported.limit_for(relation) else {
            mismatches.push(
                PortableCapabilityMismatch::UnsupportedRegularConstraintRelation {
                    relation,
                    constraint_ids: constraints.keys().copied().collect(),
                    supported_relations: supported.relations(),
                },
            );
            continue;
        };
        let actual_degrees = constraints
            .into_iter()
            .filter(|(_, degree)| !limit.allows(*degree))
            .collect::<BTreeMap<_, _>>();
        if !actual_degrees.is_empty() {
            mismatches.push(
                PortableCapabilityMismatch::RegularConstraintDegreeExceeded {
                    relation,
                    actual_degrees,
                    limit,
                },
            );
        }
    }
}

fn check_indicator_constraints(
    mismatches: &mut Vec<PortableCapabilityMismatch>,
    requirements: &BTreeMap<IndicatorConstraintID, ConstraintRequirement>,
    supported: &RelationDegreeLimits,
) {
    if requirements.is_empty() {
        return;
    }
    if supported.is_empty() {
        mismatches.push(
            PortableCapabilityMismatch::UnsupportedIndicatorConstraints {
                constraint_ids: requirements.keys().copied().collect(),
            },
        );
        return;
    }
    for (relation, constraints) in group_constraint_requirements(requirements) {
        let Some(limit) = supported.limit_for(relation) else {
            mismatches.push(
                PortableCapabilityMismatch::UnsupportedIndicatorConstraintRelation {
                    relation,
                    constraint_ids: constraints.keys().copied().collect(),
                    supported_relations: supported.relations(),
                },
            );
            continue;
        };
        let actual_degrees = constraints
            .into_iter()
            .filter(|(_, degree)| !limit.allows(*degree))
            .collect::<BTreeMap<_, _>>();
        if !actual_degrees.is_empty() {
            mismatches.push(PortableCapabilityMismatch::IndicatorBodyDegreeExceeded {
                relation,
                actual_degrees,
                limit,
            });
        }
    }
}

/// Validated alternative native capability profiles declared by an adapter.
///
/// These profiles describe direct translator inputs, not the wider set of
/// source instances an adapter might accept through preparation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdapterCapabilities {
    profiles: Vec<CapabilityProfile>,
}

impl AdapterCapabilities {
    pub fn new(profiles: Vec<CapabilityProfile>) -> Result<Self, CapabilityDefinitionError> {
        if profiles.is_empty() {
            return Err(CapabilityDefinitionError::EmptyProfiles);
        }
        let mut names = BTreeSet::new();
        for profile in &profiles {
            if !names.insert(profile.name.clone()) {
                return Err(CapabilityDefinitionError::DuplicateProfileName {
                    profile_name: profile.name.clone(),
                });
            }
        }
        Ok(Self { profiles })
    }

    pub fn profiles(&self) -> &[CapabilityProfile] {
        &self.profiles
    }

    /// Compare complete native profiles without mutating or preparing the input.
    ///
    /// Callers that prepare an instance must derive new requirements and call
    /// this method again before solver translation.
    pub fn check_compatibility(
        &self,
        requirements: &InstanceRequirements,
    ) -> PortableCompatibilityReport {
        PortableCompatibilityReport {
            profiles: self
                .profiles
                .iter()
                .map(|profile| profile.check(requirements))
                .collect(),
        }
    }
}

/// Invalid portable capability declaration.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CapabilityDefinitionError {
    #[error("Capability profile name must not be empty")]
    EmptyProfileName,
    #[error("Capability profile `{profile_name}` must support at least one optimization sense")]
    EmptySupportedSenses { profile_name: String },
    #[error("An adapter must declare at least one capability profile")]
    EmptyProfiles,
    #[error("Duplicate capability profile name `{profile_name}`")]
    DuplicateProfileName { profile_name: String },
}

/// One portable incompatibility between requirements and a complete profile.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PortableCapabilityMismatch {
    UnsupportedVariableKind {
        kind: Kind,
        used_variable_ids: VariableIDSet,
        supported_kinds: BTreeSet<Kind>,
    },
    ObjectiveDegreeExceeded {
        actual_degree: Degree,
        limit: DegreeLimit,
    },
    UnsupportedRegularConstraintRelation {
        relation: Equality,
        constraint_ids: BTreeSet<ConstraintID>,
        supported_relations: BTreeSet<Equality>,
    },
    RegularConstraintDegreeExceeded {
        relation: Equality,
        actual_degrees: BTreeMap<ConstraintID, Degree>,
        limit: DegreeLimit,
    },
    UnsupportedIndicatorConstraints {
        constraint_ids: BTreeSet<IndicatorConstraintID>,
    },
    UnsupportedIndicatorConstraintRelation {
        relation: Equality,
        constraint_ids: BTreeSet<IndicatorConstraintID>,
        supported_relations: BTreeSet<Equality>,
    },
    IndicatorBodyDegreeExceeded {
        relation: Equality,
        actual_degrees: BTreeMap<IndicatorConstraintID, Degree>,
        limit: DegreeLimit,
    },
    UnsupportedOneHotConstraints {
        constraint_ids: BTreeSet<OneHotConstraintID>,
    },
    UnsupportedSos1Constraints {
        constraint_ids: BTreeSet<Sos1ConstraintID>,
    },
    UnsupportedSense {
        sense: Sense,
        supported_senses: BTreeSet<Sense>,
    },
}

impl std::fmt::Display for PortableCapabilityMismatch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedVariableKind {
                kind,
                used_variable_ids,
                supported_kinds,
            } => write!(
                f,
                "unsupported variable kind {kind:?} for IDs {used_variable_ids:?}; supported kinds are {supported_kinds:?}"
            ),
            Self::ObjectiveDegreeExceeded {
                actual_degree,
                limit,
            } => write!(f, "objective degree {actual_degree} exceeds {limit}"),
            Self::UnsupportedRegularConstraintRelation {
                relation,
                constraint_ids,
                supported_relations,
            } => write!(
                f,
                "unsupported regular-constraint relation {relation:?} for IDs {constraint_ids:?}; supported relations are {supported_relations:?}"
            ),
            Self::RegularConstraintDegreeExceeded {
                relation,
                actual_degrees,
                limit,
            } => write!(
                f,
                "regular {relation:?} constraint degrees {actual_degrees:?} exceed {limit}"
            ),
            Self::UnsupportedIndicatorConstraints { constraint_ids } => {
                write!(f, "unsupported indicator constraints {constraint_ids:?}")
            }
            Self::UnsupportedIndicatorConstraintRelation {
                relation,
                constraint_ids,
                supported_relations,
            } => write!(
                f,
                "unsupported indicator relation {relation:?} for IDs {constraint_ids:?}; supported relations are {supported_relations:?}"
            ),
            Self::IndicatorBodyDegreeExceeded {
                relation,
                actual_degrees,
                limit,
            } => write!(
                f,
                "indicator {relation:?} body degrees {actual_degrees:?} exceed {limit}"
            ),
            Self::UnsupportedOneHotConstraints { constraint_ids } => {
                write!(f, "unsupported one-hot constraints {constraint_ids:?}")
            }
            Self::UnsupportedSos1Constraints { constraint_ids } => {
                write!(f, "unsupported SOS1 constraints {constraint_ids:?}")
            }
            Self::UnsupportedSense {
                sense,
                supported_senses,
            } => write!(
                f,
                "unsupported optimization sense {sense:?}; supported senses are {supported_senses:?}"
            ),
        }
    }
}

/// Portable compatibility result for one coherent profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileCompatibilityReport {
    profile_name: String,
    mismatches: Vec<PortableCapabilityMismatch>,
}

impl ProfileCompatibilityReport {
    pub fn profile_name(&self) -> &str {
        &self.profile_name
    }

    pub fn mismatches(&self) -> &[PortableCapabilityMismatch] {
        &self.mismatches
    }

    pub fn is_compatible(&self) -> bool {
        self.mismatches.is_empty()
    }
}

/// Side-effect-free portable compatibility report.
///
/// Requirements are compatible when at least one complete profile matches.
/// Adapter identity and adapter-specific preconditions belong to the adapter
/// integration report layered on top of this portable result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortableCompatibilityReport {
    profiles: Vec<ProfileCompatibilityReport>,
}

impl PortableCompatibilityReport {
    pub fn profiles(&self) -> &[ProfileCompatibilityReport] {
        &self.profiles
    }

    pub fn is_compatible(&self) -> bool {
        self.profiles
            .iter()
            .any(ProfileCompatibilityReport::is_compatible)
    }

    pub fn matching_profiles(&self) -> impl Iterator<Item = &str> {
        self.profiles
            .iter()
            .filter(|profile| profile.is_compatible())
            .map(|profile| profile.profile_name())
    }
}

impl std::fmt::Display for PortableCompatibilityReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_compatible() {
            let names = self.matching_profiles().collect::<Vec<_>>().join(", ");
            return write!(f, "Compatible via profile(s): {names}");
        }
        writeln!(f, "No capability profile accepts the instance:")?;
        for profile in &self.profiles {
            writeln!(f, "- profile `{}`:", profile.profile_name)?;
            for mismatch in &profile.mismatches {
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
        OneHotConstraint, OneHotConstraintID, Sos1Constraint, Sos1ConstraintID, VariableID,
    };
    use proptest::prelude::*;

    fn profile(
        name: &str,
        variable_kinds: &[Kind],
        objective_degree: DegreeLimit,
    ) -> CapabilityProfile {
        CapabilityProfile::new(
            name,
            variable_kinds.iter().copied().collect(),
            objective_degree,
            BTreeSet::from([Sense::Minimize, Sense::Maximize]),
        )
        .unwrap()
    }

    fn covering_profile(requirements: &InstanceRequirements) -> CapabilityProfile {
        let mut profile = CapabilityProfile::new(
            "covering",
            requirements
                .used_variables_by_kind()
                .keys()
                .copied()
                .collect(),
            DegreeLimit::AtMost(requirements.objective_degree()),
            BTreeSet::from([requirements.sense()]),
        )
        .unwrap();

        let mut regular = BTreeMap::<Equality, Degree>::new();
        for requirement in requirements.regular_constraints().values() {
            regular
                .entry(requirement.relation())
                .and_modify(|degree| *degree = (*degree).max(requirement.degree()))
                .or_insert(requirement.degree());
        }
        for (relation, degree) in regular {
            profile = profile.with_regular_constraint(relation, DegreeLimit::AtMost(degree));
        }

        let mut indicator = BTreeMap::<Equality, Degree>::new();
        for requirement in requirements.indicator_constraints().values() {
            indicator
                .entry(requirement.relation())
                .and_modify(|degree| *degree = (*degree).max(requirement.degree()))
                .or_insert(requirement.degree());
        }
        for (relation, degree) in indicator {
            profile = profile.with_indicator_constraint(relation, DegreeLimit::AtMost(degree));
        }
        if !requirements.one_hot_constraint_ids().is_empty() {
            profile = profile.with_one_hot();
        }
        if !requirements.sos1_constraint_ids().is_empty() {
            profile = profile.with_sos1();
        }
        profile
    }

    #[test]
    fn degree_limit_is_cumulative_and_inclusive() {
        let linear = DegreeLimit::at_most(1);
        assert!(linear.allows(0.into()));
        assert!(linear.allows(1.into()));
        assert!(!linear.allows(2.into()));
        assert_eq!(linear.maximum(), Some(1.into()));
        assert!(DegreeLimit::Any.allows(10_000.into()));
        assert_eq!(DegreeLimit::Any.maximum(), None);
    }

    #[test]
    fn multiple_profiles_do_not_cross_combine_capabilities() {
        let x = VariableID::from(1);
        let instance = crate::Instance::new(
            Sense::Minimize,
            Function::Quadratic(quadratic!(x, x).into()),
            BTreeMap::from([(x, DecisionVariable::binary())]),
            BTreeMap::new(),
        )
        .unwrap();

        let milp = profile(
            "milp",
            &[Kind::Binary, Kind::Integer, Kind::Continuous],
            DegreeLimit::at_most(1),
        )
        .with_regular_constraint(Equality::EqualToZero, DegreeLimit::at_most(1))
        .with_regular_constraint(Equality::LessThanOrEqualToZero, DegreeLimit::at_most(1));
        let continuous_qp = profile(
            "continuous-qp",
            &[Kind::Continuous],
            DegreeLimit::at_most(2),
        )
        .with_regular_constraint(Equality::EqualToZero, DegreeLimit::at_most(1))
        .with_regular_constraint(Equality::LessThanOrEqualToZero, DegreeLimit::at_most(1));
        let capabilities = AdapterCapabilities::new(vec![milp, continuous_qp]).unwrap();

        let report = capabilities.check_compatibility(&instance.solver_requirements());
        assert!(!report.is_compatible());
        assert!(matches!(
            report.profiles()[0].mismatches(),
            [PortableCapabilityMismatch::ObjectiveDegreeExceeded { .. }]
        ));
        assert!(matches!(
            report.profiles()[1].mismatches(),
            [PortableCapabilityMismatch::UnsupportedVariableKind { .. }]
        ));
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
        let limited = CapabilityProfile::new(
            "limited",
            BTreeSet::from([Kind::Binary]),
            DegreeLimit::at_most(1),
            BTreeSet::from([Sense::Minimize]),
        )
        .unwrap()
        .with_regular_constraint(Equality::EqualToZero, DegreeLimit::at_most(1))
        .with_indicator_constraint(Equality::EqualToZero, DegreeLimit::at_most(1));
        let report = AdapterCapabilities::new(vec![limited])
            .unwrap()
            .check_compatibility(&instance.solver_requirements());

        assert!(!report.is_compatible());
        assert_eq!(
            report.profiles()[0].mismatches(),
            &[
                PortableCapabilityMismatch::UnsupportedVariableKind {
                    kind: Kind::Continuous,
                    used_variable_ids: BTreeSet::from([y]),
                    supported_kinds: BTreeSet::from([Kind::Binary]),
                },
                PortableCapabilityMismatch::ObjectiveDegreeExceeded {
                    actual_degree: 2.into(),
                    limit: DegreeLimit::at_most(1),
                },
                PortableCapabilityMismatch::RegularConstraintDegreeExceeded {
                    relation: Equality::EqualToZero,
                    actual_degrees: BTreeMap::from([(regular_eq, 2.into())]),
                    limit: DegreeLimit::at_most(1),
                },
                PortableCapabilityMismatch::UnsupportedRegularConstraintRelation {
                    relation: Equality::LessThanOrEqualToZero,
                    constraint_ids: BTreeSet::from([regular_le]),
                    supported_relations: BTreeSet::from([Equality::EqualToZero]),
                },
                PortableCapabilityMismatch::IndicatorBodyDegreeExceeded {
                    relation: Equality::EqualToZero,
                    actual_degrees: BTreeMap::from([(indicator_eq, 2.into())]),
                    limit: DegreeLimit::at_most(1),
                },
                PortableCapabilityMismatch::UnsupportedIndicatorConstraintRelation {
                    relation: Equality::LessThanOrEqualToZero,
                    constraint_ids: BTreeSet::from([indicator_le]),
                    supported_relations: BTreeSet::from([Equality::EqualToZero]),
                },
                PortableCapabilityMismatch::UnsupportedOneHotConstraints {
                    constraint_ids: BTreeSet::from([one_hot]),
                },
                PortableCapabilityMismatch::UnsupportedSos1Constraints {
                    constraint_ids: BTreeSet::from([sos1]),
                },
                PortableCapabilityMismatch::UnsupportedSense {
                    sense: Sense::Maximize,
                    supported_senses: BTreeSet::from([Sense::Minimize]),
                },
            ]
        );
        assert_eq!(instance.to_v2_bytes(), before);
    }

    #[test]
    fn empty_relation_limits_express_an_unconstrained_profile() {
        let x = VariableID::from(1);
        let instance = crate::Instance::new(
            Sense::Maximize,
            Function::Quadratic(quadratic!(x, x).into()),
            BTreeMap::from([(x, DecisionVariable::binary())]),
            BTreeMap::new(),
        )
        .unwrap();
        let qubo = profile("qubo", &[Kind::Binary], DegreeLimit::at_most(2));
        let report = AdapterCapabilities::new(vec![qubo])
            .unwrap()
            .check_compatibility(&instance.solver_requirements());
        assert!(report.is_compatible());
        assert_eq!(report.matching_profiles().collect::<Vec<_>>(), ["qubo"]);
    }

    #[test]
    fn declarations_require_stable_profile_identity() {
        let empty_senses = CapabilityProfile::new(
            "invalid",
            BTreeSet::new(),
            DegreeLimit::Any,
            BTreeSet::new(),
        );
        assert!(matches!(
            empty_senses,
            Err(CapabilityDefinitionError::EmptySupportedSenses { .. })
        ));
        let duplicate = profile("duplicate", &[], DegreeLimit::Any);
        assert!(matches!(
            AdapterCapabilities::new(vec![duplicate.clone(), duplicate]),
            Err(CapabilityDefinitionError::DuplicateProfileName { .. })
        ));
    }

    proptest! {
        #[test]
        fn covering_profile_accepts_every_current_instance(instance: crate::Instance) {
            let requirements = instance.solver_requirements();
            let capabilities = AdapterCapabilities::new(vec![covering_profile(&requirements)]).unwrap();
            let report = capabilities.check_compatibility(&requirements);
            prop_assert!(report.is_compatible(), "{report}");
        }
    }
}
