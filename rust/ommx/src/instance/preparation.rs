//! Canonical construction of adapter-ready [`Instance`] values.
//!
//! [`Preparation`] records how an isolated source snapshot was transformed
//! into an input belonging to a caller-selected [`InstanceClass`]. The record
//! contains only the source, policy, input, and applied [`Transform`] values;
//! model evaluation and solver outputs are separate operations.

use super::{
    slack::{ApproximateIntegerSlackReceipt, ExactIntegerSlackReceipt},
    Instance, SpecialConstraintKind, SpecialConstraintKinds,
};
use crate::{
    v1, ATol, ConstraintID, Equality, Evaluate, Function, IndicatorConstraintID, InstanceClass,
    InstanceClassMembershipReport, Kind, Linear, LinearMonomial, OneHotConstraintID, Sampled,
    Sense, Sos1ConstraintID, VariableID,
};
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
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum PenaltyWeights {
    /// One positive finite weight for every active regular constraint.
    Uniform { weight: f64 },
    /// Positive finite weights keyed by source regular constraint ID.
    ///
    /// Every source regular row still active at the penalty phase must be
    /// present. Entries for source rows removed as trivial are permitted and
    /// are omitted from the applied Transform receipt. Special-constraint
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

    /// Absolute tolerance snapshotted into Transform receipts that use it.
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

/// One applied OMMX model transformation and the receipt needed for state
/// encoding and decoding.
///
/// Construction is private to canonical preparation so a `Transform` value
/// always records an operation that was actually committed. It has no model
/// replay method.
#[derive(Debug, Clone)]
pub struct Transform {
    kind: TransformKind,
}

#[allow(dead_code)] // Opaque receipt fields remain available in Debug and future typed views.
#[derive(Debug, Clone)]
enum TransformKind {
    LowerIndicatorConstraints {
        generated_constraints: BTreeMap<IndicatorConstraintID, Vec<ConstraintID>>,
    },
    LowerOneHotConstraints {
        generated_constraints: BTreeMap<OneHotConstraintID, ConstraintID>,
    },
    LowerSos1Constraints {
        generated_constraints: BTreeMap<Sos1ConstraintID, Vec<ConstraintID>>,
        indicator_variables: BTreeMap<Sos1ConstraintID, BTreeMap<VariableID, VariableID>>,
        atol: ATol,
    },
    LogEncodeIntegers {
        encodings: BTreeMap<VariableID, Linear>,
        atol: ATol,
    },
    NormalizeToMinimization,
    ExactIntegerSlack {
        constraint_id: ConstraintID,
        max_range: u64,
        outcome: ExactIntegerSlackOutcome,
        atol: ATol,
    },
    ApproximateIntegerSlack {
        constraint_id: ConstraintID,
        max_range: u64,
        outcome: ApproximateIntegerSlackOutcome,
        atol: ATol,
    },
    FinitePenalty {
        mode: FinitePenaltyMode,
        weights: BTreeMap<ConstraintID, f64>,
        parameter_ids: BTreeMap<ConstraintID, VariableID>,
    },
}

#[derive(Debug, Clone)]
enum ExactIntegerSlackOutcome {
    Applied {
        slack_variable_id: VariableID,
        slack_value: Function,
    },
    Trivial,
}

#[derive(Debug, Clone)]
enum ApproximateIntegerSlackOutcome {
    Applied {
        slack_variable_id: VariableID,
        source_function: Function,
        coefficient: f64,
    },
    Trivial,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FinitePenaltyMode {
    Uniform,
    PerConstraint,
}

impl Transform {
    fn new(kind: TransformKind) -> Self {
        Self { kind }
    }

    /// Stable diagnostic name of this applied operation.
    pub fn name(&self) -> &'static str {
        match &self.kind {
            TransformKind::LowerIndicatorConstraints { .. } => "lower_indicator_constraints",
            TransformKind::LowerOneHotConstraints { .. } => "lower_one_hot_constraints",
            TransformKind::LowerSos1Constraints { .. } => "lower_sos1_constraints",
            TransformKind::LogEncodeIntegers { .. } => "log_encode_integers",
            TransformKind::NormalizeToMinimization => "normalize_to_minimization",
            TransformKind::ExactIntegerSlack { .. } => "exact_integer_slack",
            TransformKind::ApproximateIntegerSlack { .. } => "approximate_integer_slack",
            TransformKind::FinitePenalty { .. } => "finite_penalty",
        }
    }

    /// Mechanically map one state from this Transform's source-side variable
    /// namespace to its input-side namespace.
    ///
    /// This operation does not evaluate either Instance or assert feasibility.
    /// Missing or unsupported state entries retain their operation-owned errors.
    pub fn encode(&self, state: &v1::State) -> crate::Result<v1::State> {
        let mut out = state.clone();
        match &self.kind {
            TransformKind::LowerSos1Constraints {
                indicator_variables,
                atol,
                ..
            } => {
                for variables in indicator_variables.values() {
                    for (&source_id, &indicator_id) in variables {
                        if source_id == indicator_id {
                            continue;
                        }
                        let value =
                            state.entries.get(&source_id.into_inner()).ok_or_else(|| {
                                crate::error!("state is missing SOS1 variable {source_id:?}")
                            })?;
                        if !value.is_finite() {
                            crate::bail!(
                                "cannot encode SOS1 indicator for {source_id:?} from non-finite value {value}"
                            );
                        }
                        out.entries.insert(
                            indicator_id.into_inner(),
                            if value.abs() >= **atol { 1.0 } else { 0.0 },
                        );
                    }
                }
            }
            TransformKind::LogEncodeIntegers { encodings, atol } => {
                for (&source_id, encoding) in encodings {
                    encode_log_value(&mut out, source_id, encoding, *atol)?;
                }
            }
            TransformKind::ExactIntegerSlack {
                outcome:
                    ExactIntegerSlackOutcome::Applied {
                        slack_variable_id,
                        slack_value,
                    },
                atol,
                ..
            } => {
                let value = slack_value.evaluate(&out, *atol)?;
                let value = integer_value(value, *atol, "exact integer slack")?;
                out.entries.insert(slack_variable_id.into_inner(), value);
            }
            TransformKind::ApproximateIntegerSlack {
                max_range,
                outcome:
                    ApproximateIntegerSlackOutcome::Applied {
                        slack_variable_id,
                        source_function,
                        coefficient,
                    },
                atol,
                ..
            } => {
                let function_value = source_function.evaluate(&out, *atol)?;
                if !function_value.is_finite() {
                    crate::bail!(
                        "cannot encode approximate integer slack from non-finite function value {function_value}"
                    );
                }
                let value = if *coefficient == 0.0 {
                    0.0
                } else {
                    ((-function_value / coefficient) + atol.into_inner())
                        .floor()
                        .clamp(0.0, *max_range as f64)
                };
                out.entries.insert(slack_variable_id.into_inner(), value);
            }
            TransformKind::LowerIndicatorConstraints { .. }
            | TransformKind::LowerOneHotConstraints { .. }
            | TransformKind::NormalizeToMinimization
            | TransformKind::ExactIntegerSlack {
                outcome: ExactIntegerSlackOutcome::Trivial,
                ..
            }
            | TransformKind::ApproximateIntegerSlack {
                outcome: ApproximateIntegerSlackOutcome::Trivial,
                ..
            }
            | TransformKind::FinitePenalty { .. } => {}
        }
        Ok(out)
    }

    /// Mechanically map one state from this Transform's input-side variable
    /// namespace back to its source-side namespace.
    ///
    /// This operation does not transport objective values, feasibility, or
    /// solver status, and is not promised to invert every possible encoding.
    pub fn decode(&self, state: &v1::State) -> crate::Result<v1::State> {
        let mut out = state.clone();
        match &self.kind {
            TransformKind::LowerSos1Constraints {
                indicator_variables,
                ..
            } => {
                for variables in indicator_variables.values() {
                    for (&source_id, &indicator_id) in variables {
                        if source_id != indicator_id {
                            out.entries.remove(&indicator_id.into_inner());
                        }
                    }
                }
            }
            TransformKind::LogEncodeIntegers { encodings, atol } => {
                for (&source_id, encoding) in encodings {
                    let value = encoding.evaluate(&out, *atol)?;
                    out.entries.insert(source_id.into_inner(), value);
                    for auxiliary_id in log_auxiliary_ids(encoding) {
                        out.entries.remove(&auxiliary_id.into_inner());
                    }
                }
            }
            TransformKind::ExactIntegerSlack {
                outcome:
                    ExactIntegerSlackOutcome::Applied {
                        slack_variable_id, ..
                    },
                ..
            }
            | TransformKind::ApproximateIntegerSlack {
                outcome:
                    ApproximateIntegerSlackOutcome::Applied {
                        slack_variable_id, ..
                    },
                ..
            } => {
                out.entries.remove(&slack_variable_id.into_inner());
            }
            TransformKind::LowerIndicatorConstraints { .. }
            | TransformKind::LowerOneHotConstraints { .. }
            | TransformKind::NormalizeToMinimization
            | TransformKind::ExactIntegerSlack {
                outcome: ExactIntegerSlackOutcome::Trivial,
                ..
            }
            | TransformKind::ApproximateIntegerSlack {
                outcome: ApproximateIntegerSlackOutcome::Trivial,
                ..
            }
            | TransformKind::FinitePenalty { .. } => {}
        }
        Ok(out)
    }

    /// Apply [`Self::encode`] pointwise while preserving sample IDs and grouping.
    pub fn encode_samples(
        &self,
        samples: &Sampled<v1::State>,
    ) -> crate::Result<Sampled<v1::State>> {
        samples.try_map_ref(|state| self.encode(state))
    }

    /// Apply [`Self::decode`] pointwise while preserving sample IDs and grouping.
    pub fn decode_samples(
        &self,
        samples: &Sampled<v1::State>,
    ) -> crate::Result<Sampled<v1::State>> {
        samples.try_map_ref(|state| self.decode(state))
    }
}

fn integer_value(value: f64, atol: ATol, operation: &str) -> crate::Result<f64> {
    if !value.is_finite() {
        crate::bail!("{operation} produced non-finite value {value}");
    }
    let rounded = value.round();
    if (value - rounded).abs() >= *atol {
        crate::bail!("{operation} produced non-integer value {value}");
    }
    Ok(rounded)
}

fn log_auxiliary_ids(encoding: &Linear) -> Vec<VariableID> {
    let mut ids = encoding
        .iter()
        .filter_map(|(monomial, _)| match monomial {
            LinearMonomial::Variable(id) => Some(*id),
            LinearMonomial::Constant => None,
        })
        .collect::<Vec<_>>();
    ids.sort();
    ids
}

fn encode_log_value(
    state: &mut v1::State,
    source_id: VariableID,
    encoding: &Linear,
    atol: ATol,
) -> crate::Result<()> {
    let source_value = *state.entries.get(&source_id.into_inner()).ok_or_else(|| {
        crate::error!("state is missing Integer variable {source_id:?} for log encoding")
    })?;
    let source_value = integer_value(source_value, atol, "log encoding")?;

    let mut constant = 0.0;
    let mut terms = Vec::new();
    for (monomial, coefficient) in encoding.iter() {
        match monomial {
            LinearMonomial::Variable(id) => terms.push((*id, coefficient.into_inner())),
            LinearMonomial::Constant => constant = coefficient.into_inner(),
        }
    }
    terms.sort_by_key(|(id, _)| *id);

    let mut remaining = source_value - constant;
    if remaining < -*atol {
        crate::bail!(
            "Integer variable {source_id:?} value {source_value} is below its log-encoding offset {constant}"
        );
    }
    for &(id, coefficient) in terms.iter().rev() {
        let bit = if remaining + *atol >= coefficient {
            remaining -= coefficient;
            1.0
        } else {
            0.0
        };
        state.entries.insert(id.into_inner(), bit);
    }
    if remaining.abs() >= *atol {
        crate::bail!(
            "Integer variable {source_id:?} value {source_value} is not representable by its applied log encoding"
        );
    }
    state.entries.remove(&source_id.into_inner());
    Ok(())
}

/// Immutable construction record produced by [`Instance::prepare`].
#[derive(Debug, Clone)]
pub struct Preparation {
    source: Instance,
    policy: PreparationPolicy,
    input: Instance,
    transforms: Vec<Transform>,
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
    pub fn input(&self) -> &Instance {
        &self.input
    }

    /// Applied Transform receipts in construction order.
    pub fn transforms(&self) -> &[Transform] {
        &self.transforms
    }

    /// Compose every applied [`Transform::encode`] in construction order.
    ///
    /// This only maps decision-variable state. It does not evaluate either
    /// Instance or attach source-side solver claims.
    pub fn encode(&self, state: &v1::State) -> crate::Result<v1::State> {
        self.transforms
            .iter()
            .try_fold(state.clone(), |state, transform| transform.encode(&state))
    }

    /// Compose every applied [`Transform::decode`] in reverse construction order.
    ///
    /// This only maps decision-variable state. Evaluate the returned state
    /// against [`Self::source`] explicitly when source-side results are needed.
    pub fn decode(&self, state: &v1::State) -> crate::Result<v1::State> {
        self.transforms
            .iter()
            .rev()
            .try_fold(state.clone(), |state, transform| transform.decode(&state))
    }

    /// Apply [`Self::encode`] pointwise while preserving sample IDs and grouping.
    pub fn encode_samples(
        &self,
        samples: &Sampled<v1::State>,
    ) -> crate::Result<Sampled<v1::State>> {
        self.transforms
            .iter()
            .try_fold(samples.clone(), |samples, transform| {
                transform.encode_samples(&samples)
            })
    }

    /// Apply [`Self::decode`] pointwise while preserving sample IDs and grouping.
    pub fn decode_samples(
        &self,
        samples: &Sampled<v1::State>,
    ) -> crate::Result<Sampled<v1::State>> {
        self.transforms
            .iter()
            .rev()
            .try_fold(samples.clone(), |samples, transform| {
                transform.decode_samples(&samples)
            })
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
        /// Membership evidence for the last committed candidate.
        current_membership: InstanceClassMembershipReport,
        /// Receipts for operations committed before the mismatch was found.
        transforms: Vec<Transform>,
        /// Human-readable mismatch detail.
        detail: String,
    },
    /// Committing the next canonical operation would exceed a Policy limit.
    ResourceLimitExceeded {
        /// Isolated source snapshot.
        source: Instance,
        /// Isolated Policy snapshot.
        policy: PreparationPolicy,
        /// Membership evidence for the last committed candidate.
        current_membership: InstanceClassMembershipReport,
        /// Receipts for operations committed before the limit was reached.
        transforms: Vec<Transform>,
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
        /// Membership evidence for the final canonical candidate.
        current_membership: InstanceClassMembershipReport,
        /// Receipts for every operation committed before exhaustion.
        transforms: Vec<Transform>,
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

    /// Receipts for operations committed before the failure.
    pub fn transforms(&self) -> &[Transform] {
        match self {
            Self::PolicyMismatch { transforms, .. }
            | Self::ResourceLimitExceeded { transforms, .. }
            | Self::TargetInstanceClassNotReached { transforms, .. } => transforms,
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
    /// canonical phase is reached. Permitted phases are applied in the fixed order
    /// Indicator, OneHot, SOS1, source Integer log encoding, sense
    /// normalization, integer slack, finite penalty, and remaining Integer log
    /// encoding. Whole-class membership is checked after every committed
    /// Transform.
    ///
    /// A failure means only that this canonical procedure did not produce the
    /// requested class member. It does not prove that no other Transform
    /// sequence exists. Existing Transform-owned errors are returned without a
    /// Preparation wrapper.
    #[tracing::instrument(skip_all)]
    pub fn prepare(&self, policy: &PreparationPolicy) -> crate::Result<Preparation> {
        let source = self.clone();
        let policy = policy.clone();
        let mut current = source.clone();
        let mut transforms = Vec::new();
        let mut membership = policy.acceptable_instance_class.check_membership(&current);

        if membership.is_member() {
            return Ok(Preparation {
                source,
                policy,
                input: current,
                transforms,
            });
        }

        macro_rules! commit {
            ($candidate:expr, $transform:expr) => {{
                let candidate = $candidate;
                let transform = $transform;
                let candidate_membership = policy
                    .acceptable_instance_class
                    .check_membership(&candidate);
                check_resource_limits(
                    &source,
                    &policy,
                    &candidate,
                    &membership,
                    &transforms,
                    transform.name(),
                )?;
                current = candidate;
                transforms.push(transform);
                membership = candidate_membership;
                if membership.is_member() {
                    return Ok(Preparation {
                        source,
                        policy,
                        input: current,
                        transforms,
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
            let generated_constraints = candidate.convert_all_indicators_to_constraints()?;
            commit!(
                candidate,
                Transform::new(TransformKind::LowerIndicatorConstraints {
                    generated_constraints
                })
            );
        }

        if policy
            .allowed_special_constraint_lowerings
            .contains(&SpecialConstraintKind::OneHot)
            && !current.one_hot_constraints().is_empty()
        {
            let mut candidate = current.clone();
            let generated_constraints =
                candidate.convert_all_one_hots_to_constraints_with_receipt()?;
            commit!(
                candidate,
                Transform::new(TransformKind::LowerOneHotConstraints {
                    generated_constraints
                })
            );
        }

        if policy
            .allowed_special_constraint_lowerings
            .contains(&SpecialConstraintKind::Sos1)
            && !current.sos1_constraints().is_empty()
        {
            let mut candidate = current.clone();
            let receipts = candidate.convert_all_sos1_to_constraints_with_receipts()?;
            let mut generated_constraints = BTreeMap::new();
            let mut indicator_variables = BTreeMap::new();
            for (id, receipt) in receipts {
                generated_constraints.insert(id, receipt.generated_constraints);
                indicator_variables.insert(id, receipt.indicator_variables);
            }
            commit!(
                candidate,
                Transform::new(TransformKind::LowerSos1Constraints {
                    generated_constraints,
                    indicator_variables,
                    atol: policy.atol,
                })
            );
        }

        if policy.allow_integer_log_encoding {
            if let Some((candidate, transform)) =
                log_encode_current_integers(&current, policy.atol)?
            {
                commit!(candidate, transform);
            }
        }

        if policy.allow_sense_normalization && current.sense() == Sense::Maximize {
            let mut candidate = current.clone();
            let changed = candidate.as_minimization_problem();
            debug_assert!(changed);
            commit!(
                candidate,
                Transform::new(TransformKind::NormalizeToMinimization)
            );
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
                let receipt = candidate.convert_inequality_to_equality_with_integer_slack_receipt(
                    constraint_id.into_inner(),
                    slack_policy.max_range,
                    policy.atol,
                );
                let (candidate, transform) = match receipt {
                    Ok(ExactIntegerSlackReceipt::Applied {
                        slack_variable_id,
                        slack_value,
                    }) => (
                        candidate,
                        Transform::new(TransformKind::ExactIntegerSlack {
                            constraint_id,
                            max_range: slack_policy.max_range,
                            outcome: ExactIntegerSlackOutcome::Applied {
                                slack_variable_id,
                                slack_value,
                            },
                            atol: policy.atol,
                        }),
                    ),
                    Ok(ExactIntegerSlackReceipt::Trivial) => (
                        candidate,
                        Transform::new(TransformKind::ExactIntegerSlack {
                            constraint_id,
                            max_range: slack_policy.max_range,
                            outcome: ExactIntegerSlackOutcome::Trivial,
                            atol: policy.atol,
                        }),
                    ),
                    Err(error)
                        if slack_policy.allow_approximate
                            && error.is::<super::slack::ExactIntegerSlackUnavailable>() =>
                    {
                        let mut candidate = current.clone();
                        match candidate.add_integer_slack_to_inequality_receipt(
                            constraint_id.into_inner(),
                            slack_policy.max_range,
                            policy.atol,
                        )? {
                            ApproximateIntegerSlackReceipt::Applied {
                                slack_variable_id,
                                source_function,
                                coefficient,
                                upper_bound,
                            } => (
                                candidate,
                                Transform::new(TransformKind::ApproximateIntegerSlack {
                                    constraint_id,
                                    max_range: upper_bound,
                                    outcome: ApproximateIntegerSlackOutcome::Applied {
                                        slack_variable_id,
                                        source_function,
                                        coefficient,
                                    },
                                    atol: policy.atol,
                                }),
                            ),
                            ApproximateIntegerSlackReceipt::Trivial => (
                                candidate,
                                Transform::new(TransformKind::ApproximateIntegerSlack {
                                    constraint_id,
                                    max_range: slack_policy.max_range,
                                    outcome: ApproximateIntegerSlackOutcome::Trivial,
                                    atol: policy.atol,
                                }),
                            ),
                        }
                    }
                    Err(error) => return Err(error),
                };
                commit!(candidate, transform);
            }
        }

        if let Some(weights) = &policy.penalty_weights {
            if !current.constraints().is_empty() {
                if let PenaltyWeights::PerConstraint { weights } = weights {
                    // Regular constraint IDs are preserved by integer-slack
                    // conversion. Consequently source weights remain attached
                    // to the same active rows, while rows generated by special
                    // lowering have no caller-provided source ID.
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
                            &membership,
                            &transforms,
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
                            &membership,
                            &transforms,
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
                            &membership,
                            &transforms,
                            format!(
                                "per-constraint penalty weights do not cover active source regular constraints {missing:?}"
                            ),
                        ));
                    }
                }
                let (candidate, transform) = apply_finite_penalty(current.clone(), weights)?;
                commit!(candidate, transform);
            }
        }

        if policy.allow_integer_log_encoding {
            if let Some((candidate, transform)) =
                log_encode_current_integers(&current, policy.atol)?
            {
                commit!(candidate, transform);
            }
        }

        Err(target_failure(
            &source,
            &policy,
            &membership,
            &transforms,
            format!(
                "canonical preparation exhausted its permitted phases without reaching the acceptable InstanceClass:\n{membership}"
            ),
        ))
    }
}

fn log_encode_current_integers(
    current: &Instance,
    atol: ATol,
) -> crate::Result<Option<(Instance, Transform)>> {
    let integer_ids = current
        .used_decision_variables()
        .into_iter()
        .filter_map(|(id, variable)| (variable.kind() == Kind::Integer).then_some(id))
        .collect::<BTreeSet<_>>();
    if integer_ids.is_empty() {
        return Ok(None);
    }
    let mut candidate = current.clone();
    let encodings = candidate.log_encode(integer_ids, atol)?;
    Ok(Some((
        candidate,
        Transform::new(TransformKind::LogEncodeIntegers { encodings, atol }),
    )))
}

fn apply_finite_penalty(
    current: Instance,
    selected: &PenaltyWeights,
) -> crate::Result<(Instance, Transform)> {
    let (mode, effective_weights) = match selected {
        PenaltyWeights::Uniform { weight } => (
            FinitePenaltyMode::Uniform,
            current
                .constraints()
                .keys()
                .map(|&id| (id, *weight))
                .collect::<BTreeMap<_, _>>(),
        ),
        PenaltyWeights::PerConstraint { weights } => (
            FinitePenaltyMode::PerConstraint,
            current
                .constraints()
                .keys()
                .map(|&id| (id, weights[&id]))
                .collect(),
        ),
    };

    let receipt = match selected {
        PenaltyWeights::Uniform { .. } => current.uniform_penalty_method_with_receipt()?,
        PenaltyWeights::PerConstraint { .. } => current.penalty_method_with_receipt()?,
    };
    let parameter_ids = receipt.parameter_ids;
    let entries = parameter_ids
        .iter()
        .map(|(&constraint_id, &parameter_id)| {
            (parameter_id.into_inner(), effective_weights[&constraint_id])
        })
        .collect::<HashMap<_, _>>();
    let candidate = receipt
        .instance
        .with_parameters(v1::Parameters { entries })?;
    Ok((
        candidate,
        Transform::new(TransformKind::FinitePenalty {
            mode,
            weights: effective_weights,
            parameter_ids,
        }),
    ))
}

fn check_resource_limits(
    source: &Instance,
    policy: &PreparationPolicy,
    candidate: &Instance,
    current_membership: &InstanceClassMembershipReport,
    completed_transforms: &[Transform],
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
                current_membership: current_membership.clone(),
                transforms: completed_transforms.to_vec(),
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
                current_membership: current_membership.clone(),
                transforms: completed_transforms.to_vec(),
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
    membership: &InstanceClassMembershipReport,
    transforms: &[Transform],
    detail: String,
) -> crate::Error {
    PreparationFailure::TargetInstanceClassNotReached {
        source: source.clone(),
        policy: policy.clone(),
        current_membership: membership.clone(),
        transforms: transforms.to_vec(),
        detail,
    }
    .into()
}

fn policy_mismatch(
    source: &Instance,
    policy: &PreparationPolicy,
    membership: &InstanceClassMembershipReport,
    transforms: &[Transform],
    detail: String,
) -> crate::Error {
    PreparationFailure::PolicyMismatch {
        source: source.clone(),
        policy: policy.clone(),
        current_membership: membership.clone(),
        transforms: transforms.to_vec(),
        detail,
    }
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        coeff, linear, Bound, Constraint, DecisionVariable, DegreeBound, IndicatorConstraint,
        InstanceClassClause, OneHotConstraint, SampleID, Sos1Constraint,
    };

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

    #[test]
    fn lowers_all_special_constraints_for_a_linear_mip_class() {
        let source = special_constraint_instance();
        let source_bytes = source.to_v2_bytes();
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
        assert_eq!(source.to_v2_bytes(), source_bytes);
        assert_eq!(preparation.source().to_v2_bytes(), source_bytes);
        assert_eq!(
            preparation
                .transforms()
                .iter()
                .map(Transform::name)
                .collect::<Vec<_>>(),
            [
                "lower_indicator_constraints",
                "lower_one_hot_constraints",
                "lower_sos1_constraints",
            ]
        );
        assert!(preparation.input().indicator_constraints().is_empty());
        assert!(preparation.input().one_hot_constraints().is_empty());
        assert!(preparation.input().sos1_constraints().is_empty());
    }

    #[test]
    fn sos1_receipt_encodes_and_removes_fresh_indicator_variables() {
        let x = VariableID::from(1);
        let y = VariableID::from(2);
        let sos1_id = Sos1ConstraintID::from(30);
        let continuous = DecisionVariable::new(
            Kind::Continuous,
            Bound::new(0.0, 2.0).unwrap(),
            ATol::default(),
        )
        .unwrap();
        let source = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::from((linear!(x) + linear!(y)).unwrap()))
            .decision_variables(BTreeMap::from([(x, continuous.clone()), (y, continuous)]))
            .constraints(BTreeMap::new())
            .sos1_constraints(BTreeMap::from([(
                sos1_id,
                Sos1Constraint::new(BTreeSet::from([x, y])).unwrap(),
            )]))
            .build()
            .unwrap();
        let policy = PreparationPolicy::new(
            highs_class(),
            BTreeSet::from([SpecialConstraintKind::Sos1]),
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

        let TransformKind::LowerSos1Constraints {
            generated_constraints,
            indicator_variables,
            atol,
        } = &preparation.transforms()[0].kind
        else {
            unreachable!()
        };
        assert_eq!(generated_constraints[&sos1_id].len(), 3);
        assert_eq!(*atol, ATol::default());
        let indicators = &indicator_variables[&sos1_id];
        assert_ne!(indicators[&x], x);
        assert_ne!(indicators[&y], y);

        let source_state = v1::State::from(HashMap::from([(1, 1.0), (2, 0.0)]));
        let encoded = preparation.encode(&source_state).unwrap();
        assert_eq!(encoded.entries[&indicators[&x].into_inner()], 1.0);
        assert_eq!(encoded.entries[&indicators[&y].into_inner()], 0.0);
        assert_eq!(preparation.decode(&encoded).unwrap(), source_state);
    }

    #[test]
    fn constructs_openjij_input_and_composes_state_and_sample_mapping() {
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
        assert_eq!(
            preparation
                .transforms()
                .iter()
                .map(Transform::name)
                .collect::<Vec<_>>(),
            [
                "log_encode_integers",
                "normalize_to_minimization",
                "exact_integer_slack",
                "finite_penalty",
                "log_encode_integers",
            ]
        );
        let TransformKind::ExactIntegerSlack {
            constraint_id,
            max_range,
            outcome: ExactIntegerSlackOutcome::Applied { .. },
            atol,
        } = &preparation.transforms()[2].kind
        else {
            unreachable!()
        };
        assert_eq!(*constraint_id, ConstraintID::from(10));
        assert_eq!(*max_range, 16);
        assert_eq!(*atol, ATol::default());
        let TransformKind::FinitePenalty { mode, .. } = &preparation.transforms()[3].kind else {
            unreachable!()
        };
        assert_eq!(*mode, FinitePenaltyMode::Uniform);

        let source_state = v1::State::from(HashMap::from([(1, 1.0)]));
        let encoded = preparation.encode(&source_state).unwrap();
        assert!(!encoded.entries.contains_key(&1));
        assert_eq!(
            encoded
                .entries
                .keys()
                .copied()
                .map(VariableID::from)
                .collect::<BTreeSet<_>>(),
            preparation.input().used_decision_variable_ids()
        );
        assert_eq!(preparation.decode(&encoded).unwrap(), source_state);

        let ids = [SampleID::from(7), SampleID::from(8)];
        let samples = Sampled::constants(ids.into_iter(), source_state.clone());
        let encoded_samples = preparation.encode_samples(&samples).unwrap();
        let decoded_samples = preparation.decode_samples(&encoded_samples).unwrap();
        assert_eq!(decoded_samples.ids(), samples.ids());
        for id in ids {
            assert_eq!(decoded_samples.get(id), Some(&source_state));
        }
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

        assert_eq!(
            preparation
                .transforms()
                .iter()
                .map(Transform::name)
                .collect::<Vec<_>>(),
            ["approximate_integer_slack", "finite_penalty"]
        );
        let TransformKind::ApproximateIntegerSlack {
            constraint_id,
            max_range,
            outcome: ApproximateIntegerSlackOutcome::Applied { .. },
            atol,
        } = &preparation.transforms()[0].kind
        else {
            unreachable!()
        };
        assert_eq!(*constraint_id, ConstraintID::from(10));
        assert_eq!(*max_range, 1);
        assert_eq!(*atol, ATol::default());
        let state = v1::State::from(HashMap::from([(1, 1.0)]));
        assert_eq!(
            preparation
                .decode(&preparation.encode(&state).unwrap())
                .unwrap(),
            state
        );
    }

    #[test]
    fn approximate_trivial_slack_receipt_keeps_the_operation_and_parameters() {
        let x = VariableID::from(1);
        let constraint_id = ConstraintID::from(10);
        let function = (Function::from(Linear::single_term(
            LinearMonomial::Variable(x),
            crate::Coefficient::try_from(f64::MAX).unwrap(),
        )) + Function::Constant(crate::Coefficient::try_from(-f64::MAX).unwrap()))
        .unwrap();
        let source = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::Zero)
            .decision_variables(BTreeMap::from([(x, DecisionVariable::binary())]))
            .constraints(BTreeMap::from([(
                constraint_id,
                Constraint::less_than_or_equal_to_zero(function),
            )]))
            .build()
            .unwrap();
        let policy = PreparationPolicy::new(
            openjij_class(),
            SpecialConstraintKinds::new(),
            false,
            false,
            Some(IntegerSlackPolicy::exact_or_approximate(7).unwrap()),
            None,
            ATol::default(),
            None,
            None,
        )
        .unwrap();

        let preparation = source.prepare(&policy).unwrap();

        assert_eq!(
            preparation
                .transforms()
                .iter()
                .map(Transform::name)
                .collect::<Vec<_>>(),
            ["approximate_integer_slack"]
        );
        let TransformKind::ApproximateIntegerSlack {
            constraint_id: recorded_constraint_id,
            max_range,
            outcome: ApproximateIntegerSlackOutcome::Trivial,
            atol,
        } = &preparation.transforms()[0].kind
        else {
            unreachable!()
        };
        assert_eq!(*recorded_constraint_id, constraint_id);
        assert_eq!(*max_range, 7);
        assert_eq!(*atol, ATol::default());
    }

    #[test]
    fn resource_failure_keeps_only_committed_transforms() {
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
                transforms,
                ..
            } => {
                assert_eq!(operation, "lower_one_hot_constraints");
                assert_eq!(resource, "regular constraints");
                assert_eq!((*observed, *limit), (2, 1));
                assert_eq!(
                    transforms.iter().map(Transform::name).collect::<Vec<_>>(),
                    ["lower_indicator_constraints"]
                );
            }
            other => panic!("unexpected preparation failure: {other:?}"),
        }
        let mut completed = source.clone();
        completed.convert_all_indicators_to_constraints().unwrap();
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
                transforms,
                ..
            } => {
                assert_eq!(operation, "log_encode_integers");
                assert_eq!(resource, "decision variables");
                assert_eq!((*observed, *limit), (2, 1));
                assert!(transforms.is_empty());
            }
            other => panic!("unexpected preparation failure: {other:?}"),
        }
        assert_eq!(
            failure.current_membership(),
            &policy.acceptable_instance_class().check_membership(&source)
        );
        assert_eq!(failure.source().to_v2_bytes(), source.to_v2_bytes());
    }

    #[test]
    fn per_constraint_policy_uses_only_active_source_rows_in_the_receipt() {
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

        assert_eq!(
            preparation
                .transforms()
                .iter()
                .map(Transform::name)
                .collect::<Vec<_>>(),
            ["exact_integer_slack", "finite_penalty"]
        );
        let TransformKind::ExactIntegerSlack {
            constraint_id,
            max_range,
            outcome: ExactIntegerSlackOutcome::Trivial,
            atol,
        } = &preparation.transforms()[0].kind
        else {
            unreachable!()
        };
        assert_eq!(*constraint_id, trivial_id);
        assert_eq!(*max_range, 16);
        assert_eq!(*atol, ATol::default());
        let TransformKind::FinitePenalty {
            mode,
            weights,
            parameter_ids,
        } = &preparation.transforms().last().unwrap().kind
        else {
            unreachable!()
        };
        assert_eq!(*mode, FinitePenaltyMode::PerConstraint);
        assert_eq!(weights, &BTreeMap::from([(active_id, 4.0)]));
        assert_eq!(
            parameter_ids.keys().copied().collect::<BTreeSet<_>>(),
            BTreeSet::from([active_id])
        );
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
        assert!(failure.transforms().is_empty());
    }

    #[test]
    fn identity_does_not_interpret_unused_transform_parameters() {
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

        assert!(preparation.transforms().is_empty());
        assert_eq!(preparation.input().to_v2_bytes(), source.to_v2_bytes());
    }

    #[test]
    fn transform_owned_error_is_not_reclassified_as_preparation_failure() {
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
