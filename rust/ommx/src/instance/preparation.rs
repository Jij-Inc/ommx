//! Canonical construction of adapter-ready [`Instance`] values.
//!
//! [`Preparation`] retains isolated source, policy, and prepared input
//! snapshots after canonical construction reaches a caller-selected
//! [`InstanceClass`]. The prepared [`Instance`] itself owns the data used to
//! evaluate solver outputs.

use super::{Instance, SpecialConstraintKind, SpecialConstraintKinds};
use crate::{
    v1, ATol, ConstraintID, Equality, InstanceClass, InstanceClassMembershipReport, Kind, Sense,
};
use anyhow::Context;
use std::collections::{BTreeMap, BTreeSet, HashMap};

/// Caller-selected integer-slack behavior for canonical preparation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IntegerSlackPolicy {
    max_range: u64,
    allow_approximate: bool,
}

impl IntegerSlackPolicy {
    /// Permit only exact integer slack with the given maximum range.
    pub fn exact(max_range: u64) -> crate::Result<Self> {
        Self::new(max_range, false)
    }

    /// Prefer exact slack and permit discrete approximate slack as fallback.
    pub fn exact_or_approximate(max_range: u64) -> crate::Result<Self> {
        Self::new(max_range, true)
    }

    fn new(max_range: u64, allow_approximate: bool) -> crate::Result<Self> {
        if max_range == 0 {
            crate::bail!("integer slack max_range must be positive");
        }
        Ok(Self {
            max_range,
            allow_approximate,
        })
    }

    /// Maximum integer range permitted for a newly introduced slack variable.
    pub fn max_range(&self) -> u64 {
        self.max_range
    }

    /// Whether approximate slack may be used when exact slack is unavailable.
    pub fn allow_approximate(&self) -> bool {
        self.allow_approximate
    }
}

/// Explicit finite penalty weights permitted by a [`PreparationPolicy`].
///
/// Positive weights are applied only to a minimization candidate. A Policy
/// preparing a maximization source must also permit sense normalization.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum PenaltyWeights {
    /// One positive finite weight for every active regular constraint.
    Uniform { weight: f64 },
    /// Positive finite weights keyed by source regular constraint ID.
    ///
    /// Every source regular row still active at the penalty phase must be
    /// present. Entries for source rows removed as trivial are permitted and
    /// are ignored when materializing the finite penalty. Special-constraint
    /// lowering creates fresh regular rows outside this source-ID domain, so
    /// [`Self::Uniform`] is required when any such row remains active.
    PerConstraint {
        weights: BTreeMap<ConstraintID, f64>,
    },
}

impl PenaltyWeights {
    fn validate(&self) -> crate::Result<()> {
        let validate_weight = |weight: f64| -> crate::Result<()> {
            if !weight.is_finite() || weight <= 0.0 {
                crate::bail!("finite penalty weights must be positive and finite: {weight}");
            }
            Ok(())
        };
        match self {
            Self::Uniform { weight } => validate_weight(*weight),
            Self::PerConstraint { weights } => {
                for (&id, &weight) in weights {
                    validate_weight(weight).map_err(|error| {
                        error.context(format!("invalid penalty weight for constraint {id:?}"))
                    })?;
                }
                Ok(())
            }
        }
    }
}

/// Caller-owned settings interpreted by [`Instance::prepare`].
///
/// This value contains permissions and parameters, not an executable recipe.
/// Canonical phase ordering belongs to the SDK implementation.
#[derive(Debug, Clone)]
pub struct PreparationPolicy {
    acceptable_instance_class: InstanceClass,
    allowed_special_constraint_lowerings: SpecialConstraintKinds,
    allow_integer_log_encoding: bool,
    allow_sense_normalization: bool,
    integer_slack_policy: Option<IntegerSlackPolicy>,
    penalty_weights: Option<PenaltyWeights>,
    atol: ATol,
    max_added_decision_variables: Option<usize>,
    max_added_regular_constraints: Option<usize>,
}

impl PreparationPolicy {
    /// Construct caller-owned permissions and resource limits.
    ///
    /// `acceptable_instance_class` is the complete success criterion. The
    /// remaining arguments permit individual canonical phases but do not set
    /// their order.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        acceptable_instance_class: InstanceClass,
        allowed_special_constraint_lowerings: SpecialConstraintKinds,
        allow_integer_log_encoding: bool,
        allow_sense_normalization: bool,
        integer_slack_policy: Option<IntegerSlackPolicy>,
        penalty_weights: Option<PenaltyWeights>,
        atol: ATol,
        max_added_decision_variables: Option<usize>,
        max_added_regular_constraints: Option<usize>,
    ) -> crate::Result<Self> {
        if let Some(weights) = &penalty_weights {
            weights.validate()?;
        }
        Ok(Self {
            acceptable_instance_class,
            allowed_special_constraint_lowerings,
            allow_integer_log_encoding,
            allow_sense_normalization,
            integer_slack_policy,
            penalty_weights,
            atol,
            max_added_decision_variables,
            max_added_regular_constraints,
        })
    }

    /// The set that the prepared input must belong to.
    pub fn acceptable_instance_class(&self) -> &InstanceClass {
        &self.acceptable_instance_class
    }

    /// Special-constraint families that canonical preparation may lower.
    pub fn allowed_special_constraint_lowerings(&self) -> &SpecialConstraintKinds {
        &self.allowed_special_constraint_lowerings
    }

    /// Whether canonical preparation may log-encode Integer variables.
    pub fn allow_integer_log_encoding(&self) -> bool {
        self.allow_integer_log_encoding
    }

    /// Whether canonical preparation may normalize maximization to minimization.
    pub fn allow_sense_normalization(&self) -> bool {
        self.allow_sense_normalization
    }

    /// Configured integer-slack behavior, if permitted.
    pub fn integer_slack_policy(&self) -> Option<IntegerSlackPolicy> {
        self.integer_slack_policy
    }

    /// Configured finite penalty weights, if permitted.
    pub fn penalty_weights(&self) -> Option<&PenaltyWeights> {
        self.penalty_weights.as_ref()
    }

    /// Absolute tolerance passed to canonical operations that accept one.
    pub fn atol(&self) -> ATol {
        self.atol
    }

    /// Maximum permitted decision-variable count added relative to the source.
    pub fn max_added_decision_variables(&self) -> Option<usize> {
        self.max_added_decision_variables
    }

    /// Maximum permitted active regular-constraint count added relative to the source.
    pub fn max_added_regular_constraints(&self) -> Option<usize> {
        self.max_added_regular_constraints
    }
}

/// Immutable construction record produced by [`Instance::prepare`].
///
/// The prepared [`Self::input`] is the owner of every transformation effect
/// needed for evaluation. This record does not define a separate state mapping
/// or duplicate transformation history outside the Instance.
#[derive(Debug, Clone)]
pub struct Preparation {
    source: Instance,
    policy: PreparationPolicy,
    input: Instance,
}

impl Preparation {
    /// Isolated snapshot of the Instance passed to [`Instance::prepare`].
    pub fn source(&self) -> &Instance {
        &self.source
    }

    /// Isolated snapshot of the Policy passed to [`Instance::prepare`].
    pub fn policy(&self) -> &PreparationPolicy {
        &self.policy
    }

    /// Constructed Instance belonging to the Policy's acceptable class.
    ///
    /// Solver states and samples are evaluated against this Instance. Its own
    /// dependency, removed-row, provenance, and auxiliary-variable data record
    /// the effects of the transformations applied during preparation.
    pub fn input(&self) -> &Instance {
        &self.input
    }
}

/// Operational evidence that canonical preparation could not return an input.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum PreparationFailure {
    /// An Instance-specific Policy value cannot be applied to the source or
    /// current canonical candidate.
    PolicyMismatch {
        /// Isolated source snapshot.
        source: Instance,
        /// Isolated Policy snapshot.
        policy: PreparationPolicy,
        /// Last candidate committed by the canonical procedure.
        current: Instance,
        /// Membership evidence for the last committed candidate.
        current_membership: InstanceClassMembershipReport,
        /// Human-readable mismatch detail.
        detail: String,
    },
    /// Committing the next canonical operation would exceed a Policy limit.
    ResourceLimitExceeded {
        /// Isolated source snapshot.
        source: Instance,
        /// Isolated Policy snapshot.
        policy: PreparationPolicy,
        /// Last candidate committed before the rejected operation.
        current: Instance,
        /// Membership evidence for the last committed candidate.
        current_membership: InstanceClassMembershipReport,
        /// Stable name of the operation whose candidate exceeded the limit.
        operation: String,
        /// Resource counted by this limit.
        resource: String,
        /// Candidate count that would have been committed.
        observed: usize,
        /// Policy limit.
        limit: usize,
    },
    /// The canonical procedure exhausted its permitted phases without reaching
    /// the acceptable InstanceClass.
    TargetInstanceClassNotReached {
        /// Isolated source snapshot.
        source: Instance,
        /// Isolated Policy snapshot.
        policy: PreparationPolicy,
        /// Final candidate committed by the canonical procedure.
        current: Instance,
        /// Membership evidence for the final canonical candidate.
        current_membership: InstanceClassMembershipReport,
        /// Human-readable target failure detail.
        detail: String,
    },
}

impl PreparationFailure {
    /// Isolated source snapshot retained as failure evidence.
    pub fn source(&self) -> &Instance {
        match self {
            Self::PolicyMismatch { source, .. }
            | Self::ResourceLimitExceeded { source, .. }
            | Self::TargetInstanceClassNotReached { source, .. } => source,
        }
    }

    /// Isolated Policy snapshot retained as failure evidence.
    pub fn policy(&self) -> &PreparationPolicy {
        match self {
            Self::PolicyMismatch { policy, .. }
            | Self::ResourceLimitExceeded { policy, .. }
            | Self::TargetInstanceClassNotReached { policy, .. } => policy,
        }
    }

    /// Last Instance committed by the canonical procedure.
    pub fn current(&self) -> &Instance {
        match self {
            Self::PolicyMismatch { current, .. }
            | Self::ResourceLimitExceeded { current, .. }
            | Self::TargetInstanceClassNotReached { current, .. } => current,
        }
    }

    /// Membership evidence for the last committed canonical candidate.
    pub fn current_membership(&self) -> &InstanceClassMembershipReport {
        match self {
            Self::PolicyMismatch {
                current_membership, ..
            }
            | Self::ResourceLimitExceeded {
                current_membership, ..
            }
            | Self::TargetInstanceClassNotReached {
                current_membership, ..
            } => current_membership,
        }
    }

    /// Human-readable detail suitable for diagnostics.
    pub fn detail(&self) -> String {
        match self {
            Self::ResourceLimitExceeded {
                operation,
                resource,
                observed,
                limit,
                ..
            } => format!(
                "{operation} would use {observed} added {resource}, exceeding the limit {limit}"
            ),
            Self::PolicyMismatch { detail, .. }
            | Self::TargetInstanceClassNotReached { detail, .. } => detail.clone(),
        }
    }
}

impl std::fmt::Display for PreparationFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.detail())
    }
}

impl std::error::Error for PreparationFailure {}

impl Instance {
    /// Apply the SDK's versioned canonical preparation procedure.
    ///
    /// The source and policy are cloned before any operation. Identity is
    /// checked first; source-relative Policy IDs are interpreted only if their
    /// canonical phase is reached. Permitted phases are applied in the fixed
    /// order Indicator, OneHot, SOS1, source Integer log encoding, sense
    /// normalization, integer slack, finite penalty, and remaining Integer log
    /// encoding. Whole-class membership is checked after every committed
    /// operation.
    ///
    /// A successful result is evaluated through [`Preparation::input`]. The
    /// prepared Instance itself records dependency, removed-row, provenance,
    /// and auxiliary-variable data; Preparation does not add encode/decode
    /// operations or a parallel transformation history.
    ///
    /// A failure means only that this canonical procedure did not produce the
    /// requested class member. It does not prove that no other operation
    /// sequence exists. Existing operation-owned errors are returned without a
    /// Preparation wrapper.
    #[tracing::instrument(skip_all)]
    pub fn prepare(&self, policy: &PreparationPolicy) -> crate::Result<Preparation> {
        let source = self.clone();
        let policy = policy.clone();
        let mut current = source.clone();
        let mut membership = policy.acceptable_instance_class.check_membership(&current);

        if membership.is_member() {
            return Ok(Preparation {
                source,
                policy,
                input: current,
            });
        }

        macro_rules! commit {
            ($candidate:expr, $operation:expr) => {{
                let candidate = $candidate;
                let candidate_membership = policy
                    .acceptable_instance_class
                    .check_membership(&candidate);
                check_resource_limits(
                    &source,
                    &policy,
                    &current,
                    &candidate,
                    &membership,
                    $operation,
                )?;
                current = candidate;
                membership = candidate_membership;
                if membership.is_member() {
                    return Ok(Preparation {
                        source,
                        policy,
                        input: current,
                    });
                }
            }};
        }

        if policy
            .allowed_special_constraint_lowerings
            .contains(&SpecialConstraintKind::Indicator)
            && !current.indicator_constraints().is_empty()
        {
            let mut candidate = current.clone();
            candidate.convert_all_indicators_to_constraints()?;
            commit!(candidate, "lower_indicator_constraints");
        }

        if policy
            .allowed_special_constraint_lowerings
            .contains(&SpecialConstraintKind::OneHot)
            && !current.one_hot_constraints().is_empty()
        {
            let mut candidate = current.clone();
            candidate.convert_all_one_hots_to_constraints()?;
            commit!(candidate, "lower_one_hot_constraints");
        }

        if policy
            .allowed_special_constraint_lowerings
            .contains(&SpecialConstraintKind::Sos1)
            && !current.sos1_constraints().is_empty()
        {
            let mut candidate = current.clone();
            candidate.convert_all_sos1_to_constraints()?;
            commit!(candidate, "lower_sos1_constraints");
        }

        if policy.allow_integer_log_encoding {
            if let Some(candidate) = log_encode_current_integers(&current, policy.atol)? {
                commit!(candidate, "log_encode_integers");
            }
        }

        if policy.allow_sense_normalization && current.sense() == Sense::Maximize {
            let mut candidate = current.clone();
            let changed = candidate.as_minimization_problem();
            debug_assert!(changed);
            commit!(candidate, "normalize_to_minimization");
        }

        if let Some(slack_policy) = policy.integer_slack_policy {
            let inequality_ids = current
                .constraints()
                .iter()
                .filter_map(|(&id, constraint)| {
                    (constraint.equality == Equality::LessThanOrEqualToZero).then_some(id)
                })
                .collect::<Vec<_>>();
            for constraint_id in inequality_ids {
                let mut candidate = current.clone();
                let operation = match candidate.convert_inequality_to_equality_with_integer_slack(
                    constraint_id.into_inner(),
                    slack_policy.max_range,
                    policy.atol,
                ) {
                    Ok(()) => "exact_integer_slack",
                    Err(error)
                        if slack_policy.allow_approximate
                            && error.is::<super::slack::ExactIntegerSlackUnavailable>() =>
                    {
                        candidate = current.clone();
                        candidate.add_integer_slack_to_inequality(
                            constraint_id.into_inner(),
                            slack_policy.max_range,
                        )?;
                        "approximate_integer_slack"
                    }
                    Err(error) => return Err(error),
                };
                commit!(candidate, operation);
            }
        }

        if let Some(weights) = &policy.penalty_weights {
            if !current.constraints().is_empty() {
                if current.sense() != Sense::Minimize {
                    return Err(policy_mismatch(
                        &source,
                        &policy,
                        &current,
                        &membership,
                        "positive finite penalty weights require minimization; permit sense normalization before the penalty phase".to_string(),
                    ));
                }
                if let PenaltyWeights::PerConstraint { weights } = weights {
                    let source_regular_ids = source
                        .constraints()
                        .keys()
                        .copied()
                        .collect::<BTreeSet<_>>();
                    let active_ids = current
                        .constraints()
                        .keys()
                        .copied()
                        .collect::<BTreeSet<_>>();
                    let configured_ids = weights.keys().copied().collect::<BTreeSet<_>>();
                    let unexpected = configured_ids
                        .difference(&source_regular_ids)
                        .copied()
                        .collect::<BTreeSet<_>>();
                    if !unexpected.is_empty() {
                        return Err(policy_mismatch(
                            &source,
                            &policy,
                            &current,
                            &membership,
                            format!(
                                "per-constraint penalty weights contain IDs outside Preparation.source active regular constraints: {unexpected:?}"
                            ),
                        ));
                    }
                    let generated = active_ids
                        .difference(&source_regular_ids)
                        .copied()
                        .collect::<BTreeSet<_>>();
                    if !generated.is_empty() {
                        return Err(policy_mismatch(
                            &source,
                            &policy,
                            &current,
                            &membership,
                            format!(
                                "special-constraint lowering generated active regular constraints {generated:?}; use a uniform penalty weight"
                            ),
                        ));
                    }
                    let remaining_source_ids = active_ids
                        .intersection(&source_regular_ids)
                        .copied()
                        .collect::<BTreeSet<_>>();
                    let missing = remaining_source_ids
                        .difference(&configured_ids)
                        .copied()
                        .collect::<BTreeSet<_>>();
                    if !missing.is_empty() {
                        return Err(policy_mismatch(
                            &source,
                            &policy,
                            &current,
                            &membership,
                            format!(
                                "per-constraint penalty weights do not cover active source regular constraints {missing:?}"
                            ),
                        ));
                    }
                }
                let candidate = apply_finite_penalty(current.clone(), weights)?;
                commit!(candidate, "finite_penalty");
            }
        }

        if policy.allow_integer_log_encoding {
            if let Some(candidate) = log_encode_current_integers(&current, policy.atol)? {
                commit!(candidate, "log_encode_integers");
            }
        }

        Err(target_failure(
            &source,
            &policy,
            &current,
            &membership,
            format!(
                "canonical preparation exhausted its permitted phases without reaching the acceptable InstanceClass:\n{membership}"
            ),
        ))
    }
}

fn log_encode_current_integers(current: &Instance, atol: ATol) -> crate::Result<Option<Instance>> {
    let integer_ids = current
        .used_decision_variables()
        .into_iter()
        .filter_map(|(id, variable)| (variable.kind() == Kind::Integer).then_some(id))
        .collect::<BTreeSet<_>>();
    if integer_ids.is_empty() {
        return Ok(None);
    }
    let mut candidate = current.clone();
    candidate.log_encode(integer_ids, atol)?;
    Ok(Some(candidate))
}

fn apply_finite_penalty(current: Instance, selected: &PenaltyWeights) -> crate::Result<Instance> {
    match selected {
        PenaltyWeights::Uniform { weight } => {
            let parametric = current.uniform_penalty_method()?;
            anyhow::ensure!(
                parametric.parameters().ids().len() == 1,
                "uniform penalty conversion must create exactly one parameter"
            );
            let parameter_id = parametric
                .parameters()
                .ids()
                .iter()
                .next()
                .copied()
                .context("uniform penalty conversion did not create a parameter")?;
            parametric.with_parameters(v1::Parameters {
                entries: HashMap::from([(parameter_id.into_inner(), *weight)]),
            })
        }
        PenaltyWeights::PerConstraint { weights } => {
            let active_weights = current
                .constraints()
                .keys()
                .map(|&constraint_id| (constraint_id, weights[&constraint_id]))
                .collect::<BTreeMap<_, _>>();
            super::penalty::penalty_method_with_weights(current, &active_weights)
        }
    }
}

fn check_resource_limits(
    source: &Instance,
    policy: &PreparationPolicy,
    current: &Instance,
    candidate: &Instance,
    current_membership: &InstanceClassMembershipReport,
    operation: &'static str,
) -> crate::Result<()> {
    let added_variables = candidate
        .decision_variables()
        .len()
        .saturating_sub(source.decision_variables().len());
    if let Some(limit) = policy.max_added_decision_variables {
        if added_variables > limit {
            return Err(PreparationFailure::ResourceLimitExceeded {
                source: source.clone(),
                policy: policy.clone(),
                current: current.clone(),
                current_membership: current_membership.clone(),
                operation: operation.to_string(),
                resource: "decision variables".to_string(),
                observed: added_variables,
                limit,
            }
            .into());
        }
    }

    let added_constraints = candidate
        .constraints()
        .len()
        .saturating_sub(source.constraints().len());
    if let Some(limit) = policy.max_added_regular_constraints {
        if added_constraints > limit {
            return Err(PreparationFailure::ResourceLimitExceeded {
                source: source.clone(),
                policy: policy.clone(),
                current: current.clone(),
                current_membership: current_membership.clone(),
                operation: operation.to_string(),
                resource: "regular constraints".to_string(),
                observed: added_constraints,
                limit,
            }
            .into());
        }
    }
    Ok(())
}

fn target_failure(
    source: &Instance,
    policy: &PreparationPolicy,
    current: &Instance,
    membership: &InstanceClassMembershipReport,
    detail: String,
) -> crate::Error {
    PreparationFailure::TargetInstanceClassNotReached {
        source: source.clone(),
        policy: policy.clone(),
        current: current.clone(),
        current_membership: membership.clone(),
        detail,
    }
    .into()
}

fn policy_mismatch(
    source: &Instance,
    policy: &PreparationPolicy,
    current: &Instance,
    membership: &InstanceClassMembershipReport,
    detail: String,
) -> crate::Error {
    PreparationFailure::PolicyMismatch {
        source: source.clone(),
        policy: policy.clone(),
        current: current.clone(),
        current_membership: membership.clone(),
        detail,
    }
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        coeff, linear, v1, Bound, Constraint, DecisionVariable, DegreeBound, Evaluate, Function,
        IndicatorConstraint, IndicatorConstraintID, InstanceClassClause, OneHotConstraint,
        OneHotConstraintID, Sos1Constraint, Sos1ConstraintID, VariableID,
    };
    use std::collections::HashMap;

    fn highs_class() -> InstanceClass {
        InstanceClass::new(vec![InstanceClassClause::new(
            "linear-mip",
            BTreeSet::from([Kind::Binary, Kind::Integer, Kind::Continuous]),
            DegreeBound::at_most(1),
            BTreeSet::from([Sense::Minimize, Sense::Maximize]),
        )
        .with_regular_constraint(Equality::EqualToZero, DegreeBound::at_most(1))
        .with_regular_constraint(
            Equality::LessThanOrEqualToZero,
            DegreeBound::at_most(1),
        )])
    }

    fn openjij_class() -> InstanceClass {
        InstanceClass::new(vec![InstanceClassClause::new(
            "binary-quadratic-unconstrained-minimization",
            BTreeSet::from([Kind::Binary]),
            DegreeBound::at_most(2),
            BTreeSet::from([Sense::Minimize]),
        )])
    }

    fn special_constraint_instance() -> Instance {
        let x = VariableID::from(1);
        let y = VariableID::from(2);
        let z = VariableID::from(3);
        let w = VariableID::from(4);
        Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::from((linear!(x) + linear!(w)).unwrap()))
            .decision_variables(BTreeMap::from([
                (x, DecisionVariable::binary()),
                (y, DecisionVariable::binary()),
                (z, DecisionVariable::binary()),
                (w, DecisionVariable::binary()),
            ]))
            .constraints(BTreeMap::new())
            .indicator_constraints(BTreeMap::from([(
                IndicatorConstraintID::from(10),
                IndicatorConstraint::new(
                    x,
                    Equality::LessThanOrEqualToZero,
                    Function::from(linear!(w) + coeff!(-0.5)),
                ),
            )]))
            .one_hot_constraints(BTreeMap::from([(
                OneHotConstraintID::from(20),
                OneHotConstraint::new(BTreeSet::from([y, z])).unwrap(),
            )]))
            .sos1_constraints(BTreeMap::from([(
                Sos1ConstraintID::from(30),
                Sos1Constraint::new(BTreeSet::from([x, y])).unwrap(),
            )]))
            .build()
            .unwrap()
    }

    fn integer_inequality_instance() -> Instance {
        let x = VariableID::from(1);
        let variable = DecisionVariable::new(
            Kind::Integer,
            Bound::new(0.0, 3.0).unwrap(),
            ATol::default(),
        )
        .unwrap();
        Instance::builder()
            .sense(Sense::Maximize)
            .objective(Function::from(linear!(x)))
            .decision_variables(BTreeMap::from([(x, variable)]))
            .constraints(BTreeMap::from([(
                ConstraintID::from(10),
                Constraint::less_than_or_equal_to_zero(Function::from(linear!(x) + coeff!(-2.0))),
            )]))
            .build()
            .unwrap()
    }

    fn zero_state_for(instance: &Instance) -> v1::State {
        v1::State::from(
            instance
                .used_decision_variable_ids()
                .into_iter()
                .map(|id| (id.into_inner(), 0.0))
                .collect::<HashMap<_, _>>(),
        )
    }

    #[test]
    fn lowers_all_special_constraints_for_a_linear_mip_class() {
        let source = special_constraint_instance();
        let policy = PreparationPolicy::new(
            highs_class(),
            BTreeSet::from([
                SpecialConstraintKind::Indicator,
                SpecialConstraintKind::OneHot,
                SpecialConstraintKind::Sos1,
            ]),
            false,
            false,
            None,
            None,
            ATol::default(),
            None,
            None,
        )
        .unwrap();

        let preparation = source.prepare(&policy).unwrap();

        assert!(policy
            .acceptable_instance_class()
            .contains(preparation.input()));
        assert_eq!(preparation.source(), &source);
        assert!(preparation.input().indicator_constraints().is_empty());
        assert!(preparation.input().one_hot_constraints().is_empty());
        assert!(preparation.input().sos1_constraints().is_empty());
        assert!(preparation
            .input()
            .removed_indicator_constraints()
            .contains_key(&IndicatorConstraintID::from(10)));
        assert!(preparation
            .input()
            .removed_one_hot_constraints()
            .contains_key(&OneHotConstraintID::from(20)));
        assert!(preparation
            .input()
            .removed_sos1_constraints()
            .contains_key(&Sos1ConstraintID::from(30)));

        let solution = preparation
            .input()
            .evaluate(&zero_state_for(preparation.input()), ATol::default())
            .unwrap();
        assert!(solution
            .evaluated_indicator_constraints()
            .is_removed(&IndicatorConstraintID::from(10)));
        assert!(solution
            .evaluated_one_hot_constraints()
            .is_removed(&OneHotConstraintID::from(20)));
        assert!(solution
            .evaluated_sos1_constraints()
            .is_removed(&Sos1ConstraintID::from(30)));
    }

    #[test]
    fn constructs_openjij_input_evaluated_by_the_prepared_instance() {
        let source = integer_inequality_instance();
        let policy = PreparationPolicy::new(
            openjij_class(),
            SpecialConstraintKinds::new(),
            true,
            true,
            Some(IntegerSlackPolicy::exact(16).unwrap()),
            Some(PenaltyWeights::Uniform { weight: 5.0 }),
            ATol::default(),
            None,
            None,
        )
        .unwrap();

        let preparation = source.prepare(&policy).unwrap();

        assert!(policy
            .acceptable_instance_class()
            .contains(preparation.input()));
        assert_eq!(preparation.input().sense(), Sense::Minimize);
        assert!(preparation.input().constraints().is_empty());
        assert!(preparation
            .input()
            .removed_constraints()
            .contains_key(&ConstraintID::from(10)));
        assert!(preparation
            .input()
            .decision_variable_dependency()
            .get(&VariableID::from(1))
            .is_some());

        let solution = preparation
            .input()
            .evaluate(&zero_state_for(preparation.input()), ATol::default())
            .unwrap();
        assert_eq!(solution.state().entries[&1], 0.0);
        assert!(solution
            .evaluated_constraints()
            .is_removed(&ConstraintID::from(10)));
    }

    #[test]
    fn approximate_slack_is_selected_only_for_the_recoverable_exact_signal() {
        let mut source = integer_inequality_instance();
        source.as_minimization_problem();
        let target = InstanceClass::new(vec![InstanceClassClause::new(
            "integer-quadratic-unconstrained",
            BTreeSet::from([Kind::Integer]),
            DegreeBound::at_most(2),
            BTreeSet::from([Sense::Minimize]),
        )]);
        let policy = PreparationPolicy::new(
            target,
            SpecialConstraintKinds::new(),
            false,
            false,
            Some(IntegerSlackPolicy::exact_or_approximate(1).unwrap()),
            Some(PenaltyWeights::Uniform { weight: 3.0 }),
            ATol::default(),
            None,
            None,
        )
        .unwrap();

        let preparation = source.prepare(&policy).unwrap();

        assert!(policy
            .acceptable_instance_class()
            .contains(preparation.input()));
        assert!(preparation.input().constraints().is_empty());
        assert!(preparation
            .input()
            .removed_constraints()
            .contains_key(&ConstraintID::from(10)));
        assert_eq!(
            preparation.input().decision_variables().len(),
            source.decision_variables().len() + 1
        );
        preparation
            .input()
            .evaluate(&zero_state_for(preparation.input()), ATol::default())
            .unwrap();
    }

    #[test]
    fn resource_failure_keeps_the_last_committed_instance() {
        let source = special_constraint_instance();
        let policy = PreparationPolicy::new(
            highs_class(),
            BTreeSet::from([
                SpecialConstraintKind::Indicator,
                SpecialConstraintKind::OneHot,
                SpecialConstraintKind::Sos1,
            ]),
            false,
            false,
            None,
            None,
            ATol::default(),
            None,
            Some(1),
        )
        .unwrap();

        let error = source.prepare(&policy).unwrap_err();
        let failure = error.downcast_ref::<PreparationFailure>().unwrap();

        match failure {
            PreparationFailure::ResourceLimitExceeded {
                operation,
                resource,
                observed,
                limit,
                current,
                ..
            } => {
                assert_eq!(operation, "lower_one_hot_constraints");
                assert_eq!(resource, "regular constraints");
                assert_eq!((*observed, *limit), (2, 1));
                let mut expected = source.clone();
                expected.convert_all_indicators_to_constraints().unwrap();
                assert_eq!(current, &expected);
            }
            other => panic!("unexpected preparation failure: {other:?}"),
        }
        let mut completed = source.clone();
        completed.convert_all_indicators_to_constraints().unwrap();
        assert_eq!(failure.current(), &completed);
        assert_eq!(
            failure.current_membership(),
            &policy
                .acceptable_instance_class()
                .check_membership(&completed)
        );
    }

    #[test]
    fn decision_variable_resource_limit_rejects_an_uncommitted_log_encoding() {
        let mut source = integer_inequality_instance();
        source.as_minimization_problem();
        let target = InstanceClass::new(vec![InstanceClassClause::new(
            "binary-linear",
            BTreeSet::from([Kind::Binary]),
            DegreeBound::at_most(1),
            BTreeSet::from([Sense::Minimize]),
        )
        .with_regular_constraint(Equality::LessThanOrEqualToZero, DegreeBound::at_most(1))]);
        let policy = PreparationPolicy::new(
            target,
            SpecialConstraintKinds::new(),
            true,
            false,
            None,
            None,
            ATol::default(),
            Some(1),
            None,
        )
        .unwrap();

        let error = source.prepare(&policy).unwrap_err();
        let failure = error.downcast_ref::<PreparationFailure>().unwrap();

        match failure {
            PreparationFailure::ResourceLimitExceeded {
                operation,
                resource,
                observed,
                limit,
                current,
                ..
            } => {
                assert_eq!(operation, "log_encode_integers");
                assert_eq!(resource, "decision variables");
                assert_eq!((*observed, *limit), (2, 1));
                assert_eq!(current, &source);
            }
            other => panic!("unexpected preparation failure: {other:?}"),
        }
        assert_eq!(
            failure.current_membership(),
            &policy.acceptable_instance_class().check_membership(&source)
        );
        assert_eq!(failure.source(), &source);
        assert_eq!(failure.current(), &source);
    }

    #[test]
    fn per_constraint_policy_uses_only_active_source_rows() {
        let x = VariableID::from(1);
        let y = VariableID::from(2);
        let trivial_id = ConstraintID::from(10);
        let active_id = ConstraintID::from(11);
        let source = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::from((linear!(x) + linear!(y)).unwrap()))
            .decision_variables(BTreeMap::from([
                (x, DecisionVariable::binary()),
                (y, DecisionVariable::binary()),
            ]))
            .constraints(BTreeMap::from([
                (
                    trivial_id,
                    Constraint::less_than_or_equal_to_zero(Function::from(
                        linear!(x) + coeff!(-2.0),
                    )),
                ),
                (
                    active_id,
                    Constraint::equal_to_zero(Function::from(linear!(y) + coeff!(-1.0))),
                ),
            ]))
            .build()
            .unwrap();
        let policy = PreparationPolicy::new(
            openjij_class(),
            SpecialConstraintKinds::new(),
            false,
            false,
            Some(IntegerSlackPolicy::exact(16).unwrap()),
            Some(PenaltyWeights::PerConstraint {
                weights: BTreeMap::from([(trivial_id, 2.0), (active_id, 4.0)]),
            }),
            ATol::default(),
            None,
            None,
        )
        .unwrap();

        let preparation = source.prepare(&policy).unwrap();

        assert!(policy
            .acceptable_instance_class()
            .contains(preparation.input()));
        assert!(preparation.input().constraints().is_empty());
        assert_eq!(
            preparation
                .input()
                .removed_constraints()
                .keys()
                .copied()
                .collect::<BTreeSet<_>>(),
            BTreeSet::from([trivial_id, active_id])
        );
        assert_eq!(
            preparation.input().removed_constraints()[&trivial_id]
                .1
                .reason,
            "ommx.Instance.convert_inequality_to_equality_with_integer_slack"
        );
        assert_eq!(
            preparation.input().removed_constraints()[&active_id]
                .1
                .reason,
            "ommx.Instance.penalty_method"
        );

        let solution = preparation
            .input()
            .evaluate(&zero_state_for(preparation.input()), ATol::default())
            .unwrap();
        assert!(solution.evaluated_constraints().is_removed(&trivial_id));
        assert!(solution.evaluated_constraints().is_removed(&active_id));
    }

    #[test]
    fn per_constraint_weights_are_materialized_by_constraint_id() {
        let x = VariableID::from(1);
        let y = VariableID::from(2);
        let x_constraint = ConstraintID::from(10);
        let y_constraint = ConstraintID::from(20);
        let source = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::Zero)
            .decision_variables(BTreeMap::from([
                (x, DecisionVariable::binary()),
                (y, DecisionVariable::binary()),
            ]))
            .constraints(BTreeMap::from([
                (
                    x_constraint,
                    Constraint::equal_to_zero(Function::from(linear!(x) + coeff!(-1.0))),
                ),
                (
                    y_constraint,
                    Constraint::equal_to_zero(Function::from(linear!(y) + coeff!(-1.0))),
                ),
            ]))
            .build()
            .unwrap();
        let policy = PreparationPolicy::new(
            openjij_class(),
            SpecialConstraintKinds::new(),
            false,
            false,
            None,
            Some(PenaltyWeights::PerConstraint {
                weights: BTreeMap::from([(x_constraint, 2.0), (y_constraint, 5.0)]),
            }),
            ATol::default(),
            None,
            None,
        )
        .unwrap();

        let preparation = source.prepare(&policy).unwrap();
        let x_only = preparation
            .input()
            .evaluate(
                &v1::State::from(HashMap::from([
                    (x.into_inner(), 0.0),
                    (y.into_inner(), 1.0),
                ])),
                ATol::default(),
            )
            .unwrap();
        let y_only = preparation
            .input()
            .evaluate(
                &v1::State::from(HashMap::from([
                    (x.into_inner(), 1.0),
                    (y.into_inner(), 0.0),
                ])),
                ATol::default(),
            )
            .unwrap();

        assert_eq!(*x_only.objective(), 2.0);
        assert_eq!(*y_only.objective(), 5.0);
    }

    #[test]
    fn positive_penalty_weights_require_minimization() {
        let x = VariableID::from(1);
        let constraint_id = ConstraintID::from(10);
        let source = Instance::builder()
            .sense(Sense::Maximize)
            .objective(Function::Zero)
            .decision_variables(BTreeMap::from([(x, DecisionVariable::binary())]))
            .constraints(BTreeMap::from([(
                constraint_id,
                Constraint::equal_to_zero(Function::from(linear!(x) + coeff!(-1.0))),
            )]))
            .build()
            .unwrap();
        let policy = PreparationPolicy::new(
            openjij_class(),
            SpecialConstraintKinds::new(),
            false,
            false,
            None,
            Some(PenaltyWeights::PerConstraint {
                weights: BTreeMap::from([(constraint_id, 2.0)]),
            }),
            ATol::default(),
            None,
            None,
        )
        .unwrap();

        let error = source.prepare(&policy).unwrap_err();
        let failure = error.downcast_ref::<PreparationFailure>().unwrap();

        match failure {
            PreparationFailure::PolicyMismatch {
                current, detail, ..
            } => {
                assert_eq!(current, &source);
                assert!(detail.contains("require minimization"));
            }
            other => panic!("unexpected preparation failure: {other:?}"),
        }
    }

    #[test]
    fn policy_mismatch_is_distinct_from_target_non_membership() {
        let x = VariableID::from(1);
        let source = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::Zero)
            .decision_variables(BTreeMap::from([(x, DecisionVariable::binary())]))
            .constraints(BTreeMap::from([(
                ConstraintID::from(10),
                Constraint::equal_to_zero(Function::from(linear!(x) + coeff!(-1.0))),
            )]))
            .build()
            .unwrap();
        let policy = PreparationPolicy::new(
            openjij_class(),
            SpecialConstraintKinds::new(),
            false,
            false,
            None,
            Some(PenaltyWeights::PerConstraint {
                weights: BTreeMap::from([(ConstraintID::from(99), 1.0)]),
            }),
            ATol::default(),
            None,
            None,
        )
        .unwrap();

        let error = source.prepare(&policy).unwrap_err();
        let failure = error.downcast_ref::<PreparationFailure>().unwrap();

        assert!(matches!(failure, PreparationFailure::PolicyMismatch { .. }));
        assert!(!failure.current_membership().is_member());
        assert_eq!(failure.source(), &source);
        assert_eq!(failure.current(), &source);
    }

    #[test]
    fn target_non_membership_retains_the_final_prepared_candidate() {
        let source = integer_inequality_instance();
        let policy = PreparationPolicy::new(
            openjij_class(),
            SpecialConstraintKinds::new(),
            false,
            false,
            None,
            None,
            ATol::default(),
            None,
            None,
        )
        .unwrap();

        let error = source.prepare(&policy).unwrap_err();
        let failure = error.downcast_ref::<PreparationFailure>().unwrap();

        assert!(matches!(
            failure,
            PreparationFailure::TargetInstanceClassNotReached { .. }
        ));
        assert_eq!(failure.source(), &source);
        assert_eq!(failure.current(), &source);
        assert_eq!(
            failure.current_membership(),
            &policy.acceptable_instance_class().check_membership(&source)
        );
    }

    #[test]
    fn identity_does_not_interpret_unused_policy_parameters() {
        let source = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::Zero)
            .decision_variables(BTreeMap::new())
            .constraints(BTreeMap::new())
            .build()
            .unwrap();
        let policy = PreparationPolicy::new(
            InstanceClass::new(vec![InstanceClassClause::new(
                "empty",
                BTreeSet::new(),
                DegreeBound::at_most(0),
                BTreeSet::from([Sense::Minimize]),
            )]),
            SpecialConstraintKinds::new(),
            false,
            false,
            None,
            Some(PenaltyWeights::PerConstraint {
                weights: BTreeMap::from([(ConstraintID::from(99), 1.0)]),
            }),
            ATol::default(),
            None,
            None,
        )
        .unwrap();

        let preparation = source.prepare(&policy).unwrap();

        assert_eq!(preparation.source(), &source);
        assert_eq!(preparation.input(), &source);
        assert!(preparation
            .policy()
            .acceptable_instance_class()
            .contains(preparation.input()));
        preparation
            .input()
            .evaluate(&v1::State::default(), ATol::default())
            .unwrap();
    }

    #[test]
    fn operation_owned_error_is_not_reclassified_as_preparation_failure() {
        let x = VariableID::from(1);
        let source = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::from(linear!(x)))
            .decision_variables(BTreeMap::from([(
                x,
                DecisionVariable::new(
                    Kind::Continuous,
                    Bound::new(0.0, 3.0).unwrap(),
                    ATol::default(),
                )
                .unwrap(),
            )]))
            .constraints(BTreeMap::from([(
                ConstraintID::from(10),
                Constraint::less_than_or_equal_to_zero(Function::from(linear!(x) + coeff!(-2.0))),
            )]))
            .build()
            .unwrap();
        let target = InstanceClass::new(vec![InstanceClassClause::new(
            "continuous-linear-equality",
            BTreeSet::from([Kind::Continuous]),
            DegreeBound::at_most(1),
            BTreeSet::from([Sense::Minimize]),
        )
        .with_regular_constraint(Equality::EqualToZero, DegreeBound::at_most(1))]);
        let policy = PreparationPolicy::new(
            target,
            SpecialConstraintKinds::new(),
            false,
            false,
            Some(IntegerSlackPolicy::exact_or_approximate(16).unwrap()),
            None,
            ATol::default(),
            None,
            None,
        )
        .unwrap();

        let error = source.prepare(&policy).unwrap_err();

        assert!(error.downcast_ref::<PreparationFailure>().is_none());
        assert!(error.to_string().contains("continuous decision variables"));
    }
}
