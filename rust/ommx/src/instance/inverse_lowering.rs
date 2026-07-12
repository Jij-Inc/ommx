//! Proof-checked recovery of capability reductions.
//!
//! The public surface exposes only an opaque, runtime-only handle for one
//! capability-reduction call and the report/state transform produced by
//! recovering that handle. Family-specific proof objects, mutation plans, and
//! assignment-map representation remain private implementation details.
//! General lifecycle reactivation such as `restore_indicator_constraint`
//! remains a distinct, potentially semantics-changing operation and is not an
//! alias for this proof-preserving inverse.

#![allow(dead_code)]

use super::{
    reduction::AssignmentMap, AdditionalCapability, Capabilities, Instance,
    CAPABILITY_REDUCTION_BATCH_TOKEN_PARAMETER, INDICATOR_LOWERING_REASON, ONE_HOT_LOWERING_REASON,
    SOS1_LOWERING_REASON,
};
use crate::{v1, IndicatorConstraintID, OneHotConstraintID, Sos1ConstraintID, VariableIDSet};
use std::fmt;
use uuid::Uuid;

/// The source ID of one special constraint lowered by a capability reduction.
///
/// IDs of different special-constraint families occupy distinct namespaces;
/// the enum preserves that family information without exposing any lowering
/// history, generated-row ID, proof object, or mutation plan.
///
/// Values returned by [`CapabilityReduction::lowered_constraints`] appear in
/// the exact order in which OMMX lowered them: Indicator IDs in ascending
/// order, followed by OneHot IDs in ascending order, followed by SOS1 IDs in
/// ascending order.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LoweredConstraintID {
    /// An Indicator constraint source.
    Indicator(IndicatorConstraintID),
    /// A OneHot constraint source.
    OneHot(OneHotConstraintID),
    /// A SOS1 constraint source.
    Sos1(Sos1ConstraintID),
}

impl LoweredConstraintID {
    /// Return the special-constraint capability represented by this source ID.
    pub fn capability(self) -> AdditionalCapability {
        match self {
            Self::Indicator(_) => AdditionalCapability::Indicator,
            Self::OneHot(_) => AdditionalCapability::OneHot,
            Self::Sos1(_) => AdditionalCapability::Sos1,
        }
    }
}

/// Runtime handle for exactly one call to
/// [`Instance::reduce_capabilities_with_recovery`].
///
/// This is an untrusted candidate locator, not a certificate. It records only
/// the exact special-constraint source IDs lowered by that call and their
/// lowering order. [`Instance::recover_capability_reduction`] consumes the
/// handle and re-verifies every candidate against the current
/// `Instance`, including its removal reason, generated rows, provenance,
/// exact row content, and variable-use invariants.
///
/// The handle is intentionally runtime-only: it is not cloneable or
/// serializable and has no public constructor. A future durable receipt is a
/// separate versioned format rather than a serialization of this type.
#[must_use = "retain this handle to recover exactly this capability-reduction batch"]
#[derive(Debug)]
pub struct CapabilityReduction {
    batch_token: Uuid,
    converted_capabilities: Capabilities,
    lowered_constraints: Vec<LoweredConstraintID>,
}

impl CapabilityReduction {
    /// Capabilities that this reduction converted to regular constraints.
    pub fn converted_capabilities(&self) -> &Capabilities {
        &self.converted_capabilities
    }

    /// Exact source IDs lowered by this call, in actual lowering order.
    ///
    /// Pre-existing removed constraints are never included.
    pub fn lowered_constraints(&self) -> &[LoweredConstraintID] {
        &self.lowered_constraints
    }

    /// Return whether this reduction lowered no special constraints.
    pub fn is_empty(&self) -> bool {
        self.lowered_constraints.is_empty()
    }
}

/// A tracked lowering that could not be proved recoverable from the current
/// `Instance`.
///
/// Skipping is a normal best-effort outcome after presolve: the presolver may
/// have removed a generated row, changed its exact content, fixed or
/// substituted a source variable, or introduced another use of a private SOS1
/// selector. The original lowered representation is left untouched for this
/// source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkippedLowering {
    constraint_id: LoweredConstraintID,
    reason: String,
}

impl SkippedLowering {
    /// Source ID that was not recovered.
    pub fn constraint_id(&self) -> LoweredConstraintID {
        self.constraint_id
    }

    /// Human-readable verifier diagnostic.
    ///
    /// This text is intended for logging and inspection, not stable
    /// machine-readable branching.
    pub fn reason(&self) -> &str {
        &self.reason
    }
}

/// Result of best-effort recovery for one [`CapabilityReduction`].
///
/// The result owns the exact raw-state transform between the `Instance`
/// immediately before and after recovery. Its representation is intentionally
/// opaque so callers cannot construct or compose unchecked mutation plans.
/// Candidate reports retain the original lowering order even though recovery
/// itself is attempted in reverse order.
///
/// State transformation requires a complete finite [`v1::State`] whose keys
/// exactly match the corresponding variable-ID set. The transform is exact
/// mathematical bookkeeping, not a tolerance-based feasibility classifier.
#[must_use = "inspect skipped lowerings and retain the state transform when solutions cross this reduction boundary"]
#[derive(Debug)]
pub struct CapabilityRecovery {
    restored_capabilities: Capabilities,
    recovered: Vec<LoweredConstraintID>,
    skipped: Vec<SkippedLowering>,
    assignment_map: AssignmentMap,
}

impl CapabilityRecovery {
    /// Capabilities for which at least one source was recovered.
    ///
    /// Use [`Self::recovered`] and [`Self::skipped`] when individual source
    /// completeness matters.
    pub fn restored_capabilities(&self) -> &Capabilities {
        &self.restored_capabilities
    }

    /// Sources recovered successfully, in their original lowering order.
    pub fn recovered(&self) -> &[LoweredConstraintID] {
        &self.recovered
    }

    /// Tracked sources that did not pass batch or current-content verification.
    pub fn skipped(&self) -> &[SkippedLowering] {
        &self.skipped
    }

    /// Complete variable-ID set immediately before recovery.
    pub fn before_variable_ids(&self) -> &VariableIDSet {
        self.assignment_map.source_ids()
    }

    /// Complete variable-ID set immediately after recovery.
    pub fn after_variable_ids(&self) -> &VariableIDSet {
        self.assignment_map.target_ids()
    }

    /// Project a complete finite state of the pre-recovery lowered `Instance`
    /// to the post-recovery `Instance`.
    pub fn project_state(&self, before: &v1::State) -> crate::Result<v1::State> {
        Ok(self.assignment_map.project_state(before)?)
    }

    /// Lift a complete finite state of the post-recovery `Instance` to the
    /// pre-recovery lowered `Instance`.
    ///
    /// Fresh SOS1 selectors use mathematical exact zero: a selector is zero
    /// exactly when its member value equals `0.0`. This does not promise
    /// preservation of every tolerance-based `Evaluate(ATol)` classification.
    pub fn lift_state(&self, after: &v1::State) -> crate::Result<v1::State> {
        Ok(self.assignment_map.lift_state(after)?)
    }
}

fn lowering_sources(instance: &Instance, supported: &Capabilities) -> Vec<LoweredConstraintID> {
    let mut sources = Vec::new();
    if !supported.contains(&AdditionalCapability::Indicator) {
        sources.extend(
            instance
                .indicator_constraints()
                .keys()
                .copied()
                .map(LoweredConstraintID::Indicator),
        );
    }
    if !supported.contains(&AdditionalCapability::OneHot) {
        sources.extend(
            instance
                .one_hot_constraints()
                .keys()
                .copied()
                .map(LoweredConstraintID::OneHot),
        );
    }
    if !supported.contains(&AdditionalCapability::Sos1) {
        sources.extend(
            instance
                .sos1_constraints()
                .keys()
                .copied()
                .map(LoweredConstraintID::Sos1),
        );
    }
    sources
}

fn bind_lowered_sources_to_batch(
    instance: &mut Instance,
    sources: &[LoweredConstraintID],
    batch_token: Uuid,
) -> crate::Result<()> {
    let value = batch_token.hyphenated().to_string();
    for source in sources {
        match *source {
            LoweredConstraintID::Indicator(id) => instance
                .indicator_constraint_collection
                .add_removed_reason_parameter(
                    id,
                    CAPABILITY_REDUCTION_BATCH_TOKEN_PARAMETER,
                    value.clone(),
                )?,
            LoweredConstraintID::OneHot(id) => instance
                .one_hot_constraint_collection
                .add_removed_reason_parameter(
                    id,
                    CAPABILITY_REDUCTION_BATCH_TOKEN_PARAMETER,
                    value.clone(),
                )?,
            LoweredConstraintID::Sos1(id) => instance
                .sos1_constraint_collection
                .add_removed_reason_parameter(
                    id,
                    CAPABILITY_REDUCTION_BATCH_TOKEN_PARAMETER,
                    value.clone(),
                )?,
        }
    }
    Ok(())
}

fn verify_source_batch(
    instance: &Instance,
    source: LoweredConstraintID,
    expected_batch_token: Uuid,
) -> Result<(), InverseLoweringError> {
    let reason = match source {
        LoweredConstraintID::Indicator(id) => instance
            .removed_indicator_constraints()
            .get(&id)
            .map(|(_, reason)| reason),
        LoweredConstraintID::OneHot(id) => instance
            .removed_one_hot_constraints()
            .get(&id)
            .map(|(_, reason)| reason),
        LoweredConstraintID::Sos1(id) => instance
            .removed_sos1_constraints()
            .get(&id)
            .map(|(_, reason)| reason),
    }
    .ok_or_else(|| {
        InverseLoweringError::not_recoverable_message(format!(
            "Removed lowering source {source:?} was not found"
        ))
    })?;
    let token = reason
        .parameters
        .get(CAPABILITY_REDUCTION_BATCH_TOKEN_PARAMETER)
        .ok_or_else(|| {
            InverseLoweringError::not_recoverable_message(format!(
                "Removed lowering source {source:?} is not bound to a capability-reduction batch"
            ))
        })?;
    let actual_batch_token = Uuid::parse_str(token).map_err(|_| {
        InverseLoweringError::not_recoverable_message(format!(
            "Removed lowering source {source:?} has non-canonical capability-reduction batch token {token:?}"
        ))
    })?;
    if token != &actual_batch_token.hyphenated().to_string() {
        return Err(InverseLoweringError::not_recoverable_message(format!(
            "Removed lowering source {source:?} has non-canonical capability-reduction batch token {token:?}"
        )));
    }
    if actual_batch_token != expected_batch_token {
        return Err(InverseLoweringError::not_recoverable_message(format!(
            "Removed lowering source {source:?} belongs to capability-reduction batch {actual_batch_token}, not handle batch {expected_batch_token}"
        )));
    }
    Ok(())
}

/// Internal outcome boundary for one checked inverse-lowering attempt.
///
/// Candidate, lifecycle, and selector-isolation rejection is an expected
/// best-effort outcome. Storage, assignment-map, and coordinator failures are
/// operation errors and must abort the root-owned atomic batch.
#[derive(Debug)]
pub(super) enum InverseLoweringError {
    NotRecoverable(crate::Error),
    Operation(crate::Error),
}

impl InverseLoweringError {
    pub(super) fn not_recoverable(error: impl Into<crate::Error>) -> Self {
        Self::NotRecoverable(error.into())
    }

    pub(super) fn not_recoverable_message(message: impl Into<String>) -> Self {
        Self::NotRecoverable(crate::Error::msg(message.into()))
    }

    pub(super) fn operation(error: impl Into<crate::Error>) -> Self {
        Self::Operation(error.into())
    }

    pub(super) fn operation_message(message: impl Into<String>) -> Self {
        Self::Operation(crate::Error::msg(message.into()))
    }
}

impl fmt::Display for InverseLoweringError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotRecoverable(error) | Self::Operation(error) => write!(f, "{error:#}"),
        }
    }
}

impl std::error::Error for InverseLoweringError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::NotRecoverable(error) | Self::Operation(error) => Some(error.as_ref()),
        }
    }
}

/// Result of one checked, root-owned inverse-lowering batch.
///
/// The map transforms complete raw states from the Instance representation
/// before the call to the representation after the call. It is an exact
/// mathematical state map, not a tolerance-based feasibility classifier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct InverseLoweringResult {
    restored_capabilities: Capabilities,
    assignment_map: AssignmentMap,
}

impl InverseLoweringResult {
    pub(super) fn restored_capabilities(&self) -> &Capabilities {
        &self.restored_capabilities
    }

    pub(super) fn before_variable_ids(&self) -> &VariableIDSet {
        self.assignment_map.source_ids()
    }

    pub(super) fn after_variable_ids(&self) -> &VariableIDSet {
        self.assignment_map.target_ids()
    }

    /// Project a complete finite state of the pre-inverse lowered Instance to
    /// the post-inverse restored Instance.
    pub(super) fn project_state(&self, before: &v1::State) -> crate::Result<v1::State> {
        Ok(self.assignment_map.project_state(before)?)
    }

    /// Lift a complete finite state of the post-inverse restored Instance to
    /// the pre-inverse lowered Instance.
    ///
    /// Fresh SOS1 selectors use mathematical exact zero. Callers must evaluate
    /// the returned state against the corresponding Instance; this method does
    /// not promise preservation of every `Evaluate(ATol)` classification.
    pub(super) fn lift_state(&self, after: &v1::State) -> crate::Result<v1::State> {
        Ok(self.assignment_map.lift_state(after)?)
    }
}

impl Instance {
    /// Convert unsupported special constraints while retaining a one-shot
    /// handle for proof-checked recovery after an external transformation.
    ///
    /// The conversion rule and deterministic order are the same as
    /// [`Self::reduce_capabilities`]. Unlike that legacy method, this operation
    /// is atomic across capability families: all conversions are applied to a
    /// staged clone and the receiver is replaced only after every requested
    /// lowering succeeds. On error, `self` is byte-for-byte unchanged and no
    /// incomplete recovery handle is produced.
    ///
    /// The returned [`CapabilityReduction`] contains exactly the active source
    /// IDs lowered by this call. It does not scan or adopt pre-existing removed
    /// histories. Retain the handle while a presolver or other external pass
    /// operates on the lowered `Instance`, then pass it to
    /// [`Self::recover_capability_reduction`].
    ///
    /// The handle is not evidence that recovery remains valid. Every source is
    /// treated as untrusted and re-verified against the post-pass `Instance`.
    #[tracing::instrument(skip_all)]
    pub fn reduce_capabilities_with_recovery(
        &mut self,
        supported: &Capabilities,
    ) -> crate::Result<CapabilityReduction> {
        let batch_token = Uuid::new_v4();
        let lowered_constraints = lowering_sources(self, supported);
        let mut staged = self.clone();
        let converted_capabilities = staged.reduce_capabilities(supported)?;
        bind_lowered_sources_to_batch(&mut staged, &lowered_constraints, batch_token)?;
        debug_assert_eq!(
            converted_capabilities,
            lowered_constraints
                .iter()
                .map(|source| source.capability())
                .collect()
        );
        *self = staged;
        Ok(CapabilityReduction {
            batch_token,
            converted_capabilities,
            lowered_constraints,
        })
    }

    /// Best-effort recovery of exactly one capability-reduction batch.
    ///
    /// `reduction` must be the runtime handle returned by
    /// [`Self::reduce_capabilities_with_recovery`]. It is consumed so one batch
    /// is not accidentally recovered repeatedly. Every source in the handle is
    /// attempted; its converted capabilities are the explicit permission to
    /// add those semantic capabilities to the current `Instance`.
    ///
    /// Sources are attempted in the reverse of their actual lowering order.
    /// Each source's globally unique batch token is matched before its current
    /// content is re-verified. A source that belongs to another reduction batch
    /// or that a presolver changed is reported through
    /// [`CapabilityRecovery::skipped`], and its lowered representation remains
    /// untouched. No unrelated removed history is inspected or recovered.
    ///
    /// Successful source plans and their state maps are composed on one staged
    /// clone, then committed to `self` once. Candidate rejection is a normal
    /// successful result. An operation-level error, such as an internal map
    /// composition inconsistency, leaves `self` byte-for-byte unchanged.
    pub fn recover_capability_reduction(
        &mut self,
        reduction: CapabilityReduction,
    ) -> crate::Result<CapabilityRecovery> {
        let CapabilityReduction {
            batch_token,
            converted_capabilities,
            lowered_constraints,
        } = reduction;
        let mut assignment_map =
            AssignmentMap::identity(self.decision_variables.keys().copied().collect());
        let mut staged = self.clone();
        let mut restored_capabilities = Capabilities::new();
        let mut recovered = Vec::new();
        let mut skipped = Vec::new();

        for source in lowered_constraints.into_iter().rev() {
            let step =
                verify_source_batch(&staged, source, batch_token).and_then(|()| match source {
                    LoweredConstraintID::Indicator(id) => {
                        staged.restore_indicator_from_lowering_checked(id, &converted_capabilities)
                    }
                    LoweredConstraintID::OneHot(id) => {
                        staged.restore_one_hot_from_lowering_checked(id, &converted_capabilities)
                    }
                    LoweredConstraintID::Sos1(id) => {
                        staged.restore_sos1_from_lowering_checked(id, &converted_capabilities)
                    }
                });
            match step {
                Ok(step) => {
                    assignment_map = assignment_map.then(step)?;
                    restored_capabilities.insert(source.capability());
                    recovered.push(source);
                }
                Err(InverseLoweringError::NotRecoverable(error)) => skipped.push(SkippedLowering {
                    constraint_id: source,
                    reason: format!("{error:#}"),
                }),
                Err(InverseLoweringError::Operation(error)) => return Err(error),
            }
        }

        recovered.reverse();
        skipped.reverse();
        let staged_variable_ids = staged
            .decision_variables
            .keys()
            .copied()
            .collect::<VariableIDSet>();
        debug_assert_eq!(assignment_map.target_ids(), &staged_variable_ids);
        *self = staged;

        Ok(CapabilityRecovery {
            restored_capabilities,
            recovered,
            skipped,
            assignment_map,
        })
    }

    /// Restore every current OMMX V1 Indicator/OneHot/SOS1 lowering history in the
    /// requested families, or leave the Instance entirely unchanged.
    ///
    /// `requested` is both a family filter and explicit permission to add that
    /// semantic capability. Ordinary lifecycle removals with other reasons are
    /// ignored. Once an exact OMMX lowering reason selects a history, any
    /// malformed parameter, row, provenance, selector, or use is a hard error
    /// for the complete batch rather than a skipped candidate.
    ///
    /// Public generic naming, serialized receipts, and Python bindings remain
    /// deferred.
    pub(super) fn restore_lowered_capabilities_checked(
        &mut self,
        requested: &Capabilities,
    ) -> crate::Result<InverseLoweringResult> {
        let mut indicator_ids = if requested.contains(&AdditionalCapability::Indicator) {
            self.removed_indicator_constraints()
                .iter()
                .filter_map(|(&id, (_, reason))| {
                    (reason.reason == INDICATOR_LOWERING_REASON).then_some(id)
                })
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        let mut one_hot_ids = if requested.contains(&AdditionalCapability::OneHot) {
            self.removed_one_hot_constraints()
                .iter()
                .filter_map(|(&id, (_, reason))| {
                    (reason.reason == ONE_HOT_LOWERING_REASON).then_some(id)
                })
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        let mut sos1_ids = if requested.contains(&AdditionalCapability::Sos1) {
            self.removed_sos1_constraints()
                .iter()
                .filter_map(|(&id, (_, reason))| {
                    (reason.reason == SOS1_LOWERING_REASON).then_some(id)
                })
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };

        let mut assignment_map =
            AssignmentMap::identity(self.decision_variables.keys().copied().collect());
        if indicator_ids.is_empty() && one_hot_ids.is_empty() && sos1_ids.is_empty() {
            return Ok(InverseLoweringResult {
                restored_capabilities: Capabilities::new(),
                assignment_map,
            });
        }

        // `reduce_capabilities` lowers Indicator, then OneHot, then SOS1, and
        // each family lowers IDs in ascending order. Undo that deterministic
        // stack in reverse. Every family operation commits only to this staged
        // clone; the caller-visible root is replaced once after the whole batch.
        indicator_ids.reverse();
        one_hot_ids.reverse();
        sos1_ids.reverse();
        let mut staged = self.clone();
        let mut restored_capabilities = Capabilities::new();

        for id in sos1_ids {
            let step = staged.restore_sos1_from_lowering_checked(id, requested)?;
            assignment_map = assignment_map.then(step)?;
            restored_capabilities.insert(AdditionalCapability::Sos1);
        }
        for id in one_hot_ids {
            let step = staged.restore_one_hot_from_lowering_checked(id, requested)?;
            assignment_map = assignment_map.then(step)?;
            restored_capabilities.insert(AdditionalCapability::OneHot);
        }
        for id in indicator_ids {
            let step = staged.restore_indicator_from_lowering_checked(id, requested)?;
            assignment_map = assignment_map.then(step)?;
            restored_capabilities.insert(AdditionalCapability::Indicator);
        }

        let staged_variable_ids = staged
            .decision_variables
            .keys()
            .copied()
            .collect::<VariableIDSet>();
        debug_assert_eq!(assignment_map.target_ids(), &staged_variable_ids);
        *self = staged;
        Ok(InverseLoweringResult {
            restored_capabilities,
            assignment_map,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        coeff, linear, Bound, Constraint, DecisionVariable, Equality, Evaluate, Function,
        IndicatorConstraint, Kind, OneHotConstraint, Sense, Sos1Constraint, VariableID,
    };
    use maplit::btreemap;
    use std::collections::{BTreeMap, BTreeSet};

    fn requested(values: impl IntoIterator<Item = AdditionalCapability>) -> Capabilities {
        values.into_iter().collect()
    }

    fn combined_instance() -> Instance {
        let bounded = DecisionVariable::new(
            Kind::Continuous,
            Bound::new(0.0, 5.0).unwrap(),
            crate::ATol::default(),
        )
        .unwrap();
        let integer = DecisionVariable::new(
            Kind::Integer,
            Bound::new(-2.0, 3.0).unwrap(),
            crate::ATol::default(),
        )
        .unwrap();
        Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::from(
                ((linear!(1) + linear!(2)).unwrap() + linear!(3)).unwrap(),
            ))
            .decision_variables(btreemap! {
                VariableID::from(0) => DecisionVariable::binary(),
                VariableID::from(1) => bounded,
                VariableID::from(2) => DecisionVariable::binary(),
                VariableID::from(3) => integer,
            })
            .constraints(BTreeMap::new())
            .indicator_constraints(BTreeMap::from([(
                crate::IndicatorConstraintID::from(7),
                IndicatorConstraint::new(
                    VariableID::from(0),
                    Equality::LessThanOrEqualToZero,
                    Function::from((linear!(1) + coeff!(-2.0)).unwrap()),
                ),
            )]))
            .sos1_constraints(BTreeMap::from([(
                crate::Sos1ConstraintID::from(9),
                Sos1Constraint::new(BTreeSet::from([VariableID::from(2), VariableID::from(3)]))
                    .unwrap(),
            )]))
            .build()
            .unwrap()
    }

    fn first_recorded_constraint_id(reason: &crate::RemovedReason) -> crate::ConstraintID {
        let first = reason.parameters[super::super::GENERATED_CONSTRAINT_IDS_PARAMETER]
            .split(',')
            .next()
            .expect("current lowering records at least one generated row")
            .parse::<u64>()
            .unwrap();
        crate::ConstraintID::from(first)
    }

    #[test]
    fn restores_reduce_capabilities_indicator_and_sos1_end_to_end() {
        let mut instance = combined_instance();
        let original = instance.clone();
        let original_bytes = instance.to_v2_bytes();
        assert_eq!(
            instance.reduce_capabilities(&Capabilities::new()).unwrap(),
            requested([AdditionalCapability::Indicator, AdditionalCapability::Sos1,])
        );
        let before_variable_ids = instance
            .decision_variables()
            .keys()
            .copied()
            .collect::<VariableIDSet>();
        let lowered = instance.clone();
        let selector = *before_variable_ids
            .difference(&original.decision_variables().keys().copied().collect())
            .next()
            .expect("SOS1 lowering introduced one fresh selector");

        let result = instance
            .restore_lowered_capabilities_checked(&requested([
                AdditionalCapability::Indicator,
                AdditionalCapability::Sos1,
            ]))
            .unwrap();

        assert_eq!(instance, original);
        assert_eq!(instance.to_v2_bytes(), original_bytes);
        assert_eq!(
            result.restored_capabilities(),
            &requested([AdditionalCapability::Indicator, AdditionalCapability::Sos1,])
        );
        assert_eq!(result.before_variable_ids(), &before_variable_ids);
        assert_eq!(
            result.after_variable_ids(),
            &instance
                .decision_variables()
                .keys()
                .copied()
                .collect::<VariableIDSet>()
        );

        let before = v1::State::from_iter([
            (0, 1.0),
            (1, 2.0),
            (2, 0.0),
            (3, 1.0),
            (selector.into_inner(), 1.0),
        ]);
        let after = result.project_state(&before).unwrap();
        assert_eq!(after.entries.get(&selector.into_inner()), None);
        assert_eq!(result.lift_state(&after).unwrap(), before);
        assert_eq!(
            result
                .project_state(&result.lift_state(&after).unwrap())
                .unwrap(),
            after
        );
        let before_solution = lowered.evaluate(&before, crate::ATol::default()).unwrap();
        let after_solution = instance.evaluate(&after, crate::ATol::default()).unwrap();
        assert_eq!(before_solution.feasible(), after_solution.feasible());
        assert_eq!(before_solution.objective(), after_solution.objective());
    }

    #[test]
    fn requested_family_filter_leaves_other_lowering_untouched() {
        let mut instance = combined_instance();
        instance.reduce_capabilities(&Capabilities::new()).unwrap();
        let removed_indicator = instance.removed_indicator_constraints().clone();

        let result = instance
            .restore_lowered_capabilities_checked(&requested([AdditionalCapability::Sos1]))
            .unwrap();

        assert_eq!(
            result.restored_capabilities(),
            &requested([AdditionalCapability::Sos1])
        );
        assert_eq!(instance.removed_indicator_constraints(), &removed_indicator);
        assert!(instance.indicator_constraints().is_empty());
        assert_eq!(
            instance.required_capabilities(),
            requested([AdditionalCapability::Sos1])
        );
    }

    #[test]
    fn cross_family_failure_rolls_back_earlier_staged_restoration() {
        let mut instance = combined_instance();
        instance.reduce_capabilities(&Capabilities::new()).unwrap();
        let (_, reason) =
            &instance.removed_indicator_constraints()[&crate::IndicatorConstraintID::from(7)];
        let indicator_row = first_recorded_constraint_id(reason);
        instance
            .relax_constraint(indicator_row, "test corruption".to_string(), [])
            .unwrap();
        let before = instance.clone();
        let before_bytes = instance.to_v2_bytes();

        let error = instance
            .restore_lowered_capabilities_checked(&requested([
                AdditionalCapability::Indicator,
                AdditionalCapability::Sos1,
            ]))
            .unwrap_err();

        assert!(error.to_string().contains("is not active"));
        assert_eq!(instance, before);
        assert_eq!(instance.to_v2_bytes(), before_bytes);
    }

    #[test]
    fn two_sos1_maps_compose_in_reverse_lowering_order() {
        let variables = btreemap! {
            VariableID::from(0) => DecisionVariable::new(
                Kind::Continuous,
                Bound::new(-1.0, 2.0).unwrap(),
                crate::ATol::default(),
            ).unwrap(),
            VariableID::from(1) => DecisionVariable::new(
                Kind::Continuous,
                Bound::new(-3.0, 4.0).unwrap(),
                crate::ATol::default(),
            ).unwrap(),
        };
        let mut instance = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::from((linear!(0) + linear!(1)).unwrap()))
            .decision_variables(variables)
            .constraints(BTreeMap::new())
            .sos1_constraints(BTreeMap::from([
                (
                    crate::Sos1ConstraintID::from(1),
                    Sos1Constraint::new(BTreeSet::from([VariableID::from(0)])).unwrap(),
                ),
                (
                    crate::Sos1ConstraintID::from(2),
                    Sos1Constraint::new(BTreeSet::from([VariableID::from(1)])).unwrap(),
                ),
            ]))
            .build()
            .unwrap();
        let original = instance.clone();
        instance.reduce_capabilities(&Capabilities::new()).unwrap();
        let source_ids = instance
            .decision_variables()
            .keys()
            .copied()
            .collect::<VariableIDSet>();
        let selectors = source_ids
            .difference(&original.decision_variables().keys().copied().collect())
            .copied()
            .collect::<Vec<_>>();
        assert_eq!(selectors.len(), 2);

        let result = instance
            .restore_lowered_capabilities_checked(&requested([AdditionalCapability::Sos1]))
            .unwrap();
        assert_eq!(instance, original);

        let after = v1::State::from_iter([(0, 2.0), (1, 0.0)]);
        let before = result.lift_state(&after).unwrap();
        assert_eq!(before.entries[&selectors[0].into_inner()], 1.0);
        assert_eq!(before.entries[&selectors[1].into_inner()], 0.0);
        assert_eq!(result.project_state(&before).unwrap(), after);
    }

    #[test]
    fn empty_and_unmatched_requests_are_atomic() {
        let mut instance = combined_instance();
        let before = instance.clone();
        let before_bytes = instance.to_v2_bytes();

        let empty = instance
            .restore_lowered_capabilities_checked(&Capabilities::new())
            .unwrap();
        assert!(empty.restored_capabilities().is_empty());
        let state = v1::State::from_iter([(0, 1.0), (1, 2.0), (2, 0.0), (3, 1.0)]);
        assert_eq!(empty.project_state(&state).unwrap(), state);
        assert_eq!(instance, before);
        assert_eq!(instance.to_v2_bytes(), before_bytes);

        let unmatched = instance
            .restore_lowered_capabilities_checked(&requested([AdditionalCapability::OneHot]))
            .unwrap();
        assert!(unmatched.restored_capabilities().is_empty());
        assert_eq!(instance, before);
        assert_eq!(instance.to_v2_bytes(), before_bytes);
    }

    #[test]
    fn restores_reduce_capabilities_one_hot_end_to_end() {
        let id = crate::OneHotConstraintID::from(4);
        let mut instance = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::from((linear!(0) + linear!(1)).unwrap()))
            .decision_variables(btreemap! {
                VariableID::from(0) => DecisionVariable::binary(),
                VariableID::from(1) => DecisionVariable::binary(),
            })
            .constraints(BTreeMap::new())
            .one_hot_constraints(BTreeMap::from([(
                id,
                OneHotConstraint::new(BTreeSet::from([VariableID::from(0), VariableID::from(1)]))
                    .unwrap(),
            )]))
            .build()
            .unwrap();
        instance
            .set_one_hot_constraint_context(
                id,
                crate::ConstraintContext {
                    label: crate::ModelingLabel {
                        name: Some("choose".to_string()),
                        ..Default::default()
                    },
                    provenance: vec![crate::Provenance::IndicatorConstraint(
                        crate::IndicatorConstraintID::from(12),
                    )],
                },
            )
            .unwrap();
        let original = instance.clone();
        let original_bytes = instance.to_v2_bytes();
        assert_eq!(
            instance.reduce_capabilities(&Capabilities::new()).unwrap(),
            requested([AdditionalCapability::OneHot])
        );
        let lowered = instance.clone();

        let result = instance
            .restore_lowered_capabilities_checked(&requested([AdditionalCapability::OneHot]))
            .unwrap();

        assert_eq!(instance, original);
        assert_eq!(instance.to_v2_bytes(), original_bytes);
        assert_eq!(
            result.restored_capabilities(),
            &requested([AdditionalCapability::OneHot])
        );
        assert_eq!(result.before_variable_ids(), result.after_variable_ids());
        for state in [
            v1::State::from_iter([(0, 1.0), (1, 0.0)]),
            v1::State::from_iter([(0, 0.0), (1, 0.0)]),
        ] {
            let projected = result.project_state(&state).unwrap();
            assert_eq!(projected, state);
            assert_eq!(result.lift_state(&projected).unwrap(), state);
            assert_eq!(
                lowered
                    .evaluate(&state, crate::ATol::default())
                    .unwrap()
                    .feasible(),
                instance
                    .evaluate(&projected, crate::ATol::default())
                    .unwrap()
                    .feasible()
            );
        }
    }

    #[test]
    fn malformed_one_hot_rolls_back_an_earlier_sos1_restoration() {
        let one_hot_id = crate::OneHotConstraintID::from(4);
        let mut instance = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::from(
                ((linear!(0) + linear!(1)).unwrap() + linear!(2)).unwrap(),
            ))
            .decision_variables(btreemap! {
                VariableID::from(0) => DecisionVariable::binary(),
                VariableID::from(1) => DecisionVariable::binary(),
                VariableID::from(2) => DecisionVariable::new(
                    Kind::Continuous,
                    Bound::new(-1.0, 2.0).unwrap(),
                    crate::ATol::default(),
                ).unwrap(),
            })
            .constraints(BTreeMap::new())
            .one_hot_constraints(BTreeMap::from([(
                one_hot_id,
                OneHotConstraint::new(BTreeSet::from([VariableID::from(0), VariableID::from(1)]))
                    .unwrap(),
            )]))
            .sos1_constraints(BTreeMap::from([(
                crate::Sos1ConstraintID::from(5),
                Sos1Constraint::new(BTreeSet::from([VariableID::from(2)])).unwrap(),
            )]))
            .build()
            .unwrap();
        instance.reduce_capabilities(&Capabilities::new()).unwrap();
        let (_, reason) = &instance.removed_one_hot_constraints()[&one_hot_id];
        let row = crate::ConstraintID::from(
            reason.parameters[super::super::ONE_HOT_GENERATED_CONSTRAINT_ID_PARAMETER]
                .parse::<u64>()
                .unwrap(),
        );
        instance
            .insert_constraint(
                row,
                Constraint::equal_to_zero(Function::from(
                    ((linear!(0) + linear!(1)).unwrap() + coeff!(-2.0)).unwrap(),
                )),
            )
            .unwrap();
        let before = instance.clone();
        let before_bytes = instance.to_v2_bytes();

        let error = instance
            .restore_lowered_capabilities_checked(&requested([
                AdditionalCapability::OneHot,
                AdditionalCapability::Sos1,
            ]))
            .unwrap_err();

        assert!(error.to_string().contains("canonical V1 equality exactly"));
        assert_eq!(instance, before);
        assert_eq!(instance.to_v2_bytes(), before_bytes);
    }

    #[test]
    fn manual_lifecycle_removal_is_not_an_inverse_lowering_candidate() {
        let mut instance = combined_instance();
        instance
            .relax_indicator_constraint(
                crate::IndicatorConstraintID::from(7),
                "manual lifecycle removal".to_string(),
                [],
            )
            .unwrap();
        let before = instance.clone();
        let result = instance
            .restore_lowered_capabilities_checked(&requested([AdditionalCapability::Indicator]))
            .unwrap();

        assert!(result.restored_capabilities().is_empty());
        assert_eq!(instance, before);
    }

    #[test]
    fn lowered_one_hot_is_not_silently_restored() {
        let mut instance = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::from((linear!(0) + linear!(1)).unwrap()))
            .decision_variables(btreemap! {
                VariableID::from(0) => DecisionVariable::binary(),
                VariableID::from(1) => DecisionVariable::binary(),
            })
            .constraints(BTreeMap::new())
            .one_hot_constraints(BTreeMap::from([(
                crate::OneHotConstraintID::from(4),
                OneHotConstraint::new(BTreeSet::from([VariableID::from(0), VariableID::from(1)]))
                    .unwrap(),
            )]))
            .build()
            .unwrap();
        instance.reduce_capabilities(&Capabilities::new()).unwrap();
        let before = instance.clone();

        let result = instance
            .restore_lowered_capabilities_checked(&requested([
                AdditionalCapability::Indicator,
                AdditionalCapability::Sos1,
            ]))
            .unwrap();

        assert!(result.restored_capabilities().is_empty());
        assert_eq!(instance, before);
        assert!(instance.one_hot_constraints().is_empty());
        assert_eq!(instance.removed_one_hot_constraints().len(), 1);
    }

    #[test]
    fn unrecorded_row_with_sos1_provenance_is_a_hard_batch_error() {
        let mut instance = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::from((linear!(0) + linear!(1)).unwrap()))
            .decision_variables(btreemap! {
                VariableID::from(0) => DecisionVariable::binary(),
                VariableID::from(1) => DecisionVariable::binary(),
            })
            .constraints(BTreeMap::new())
            .sos1_constraints(BTreeMap::from([(
                crate::Sos1ConstraintID::from(5),
                Sos1Constraint::new(BTreeSet::from([VariableID::from(0), VariableID::from(1)]))
                    .unwrap(),
            )]))
            .build()
            .unwrap();
        instance.reduce_capabilities(&Capabilities::new()).unwrap();
        let extra_id = instance
            .add_constraint(
                Constraint::less_than_or_equal_to_zero(Function::zero()),
                crate::ConstraintContext {
                    label: Default::default(),
                    provenance: vec![crate::Provenance::Sos1Constraint(
                        crate::Sos1ConstraintID::from(5),
                    )],
                },
            )
            .unwrap();
        let before = instance.clone();
        let before_bytes = instance.to_v2_bytes();

        let error = instance
            .restore_lowered_capabilities_checked(&requested([AdditionalCapability::Sos1]))
            .unwrap_err();

        assert!(error.to_string().contains(&format!("{extra_id:?}")));
        assert!(error.to_string().contains("not recorded"));
        assert_eq!(instance, before);
        assert_eq!(instance.to_v2_bytes(), before_bytes);
    }

    fn two_one_hot_instance() -> Instance {
        let members = BTreeSet::from([VariableID::from(0), VariableID::from(1)]);
        Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::zero())
            .decision_variables(btreemap! {
                VariableID::from(0) => DecisionVariable::binary(),
                VariableID::from(1) => DecisionVariable::binary(),
            })
            .constraints(BTreeMap::new())
            .one_hot_constraints(BTreeMap::from([
                (
                    crate::OneHotConstraintID::from(1),
                    OneHotConstraint::new(members.clone()).unwrap(),
                ),
                (
                    crate::OneHotConstraintID::from(2),
                    OneHotConstraint::new(members).unwrap(),
                ),
            ]))
            .build()
            .unwrap()
    }

    fn one_hot_generated_row(instance: &Instance, id: u64) -> crate::ConstraintID {
        crate::ConstraintID::from(
            instance.removed_one_hot_constraints()[&crate::OneHotConstraintID::from(id)]
                .1
                .parameters[super::super::ONE_HOT_GENERATED_CONSTRAINT_ID_PARAMETER]
                .parse::<u64>()
                .unwrap(),
        )
    }

    #[test]
    fn public_reduction_handle_does_not_adopt_pre_existing_history() {
        let mut instance = two_one_hot_instance();
        instance
            .convert_one_hot_to_constraint(crate::OneHotConstraintID::from(1))
            .unwrap();

        let reduction = instance
            .reduce_capabilities_with_recovery(&Capabilities::new())
            .unwrap();
        assert_eq!(
            reduction.lowered_constraints(),
            &[LoweredConstraintID::OneHot(
                crate::OneHotConstraintID::from(2)
            )]
        );

        let recovery = instance.recover_capability_reduction(reduction).unwrap();

        assert_eq!(
            recovery.recovered(),
            &[LoweredConstraintID::OneHot(
                crate::OneHotConstraintID::from(2)
            )]
        );
        assert!(recovery.skipped().is_empty());
        assert!(instance
            .one_hot_constraints()
            .contains_key(&crate::OneHotConstraintID::from(2)));
        assert!(!instance
            .one_hot_constraints()
            .contains_key(&crate::OneHotConstraintID::from(1)));
        assert!(instance
            .removed_one_hot_constraints()
            .contains_key(&crate::OneHotConstraintID::from(1)));
    }

    #[test]
    fn public_recovery_is_best_effort_per_recorded_source() {
        let mut instance = two_one_hot_instance();
        let reduction = instance
            .reduce_capabilities_with_recovery(&Capabilities::new())
            .unwrap();
        let corrupted = one_hot_generated_row(&instance, 1);
        instance
            .insert_constraint(
                corrupted,
                Constraint::equal_to_zero(Function::from(
                    ((linear!(0) + linear!(1)).unwrap() + coeff!(-2.0)).unwrap(),
                )),
            )
            .unwrap();

        let recovery = instance.recover_capability_reduction(reduction).unwrap();

        assert_eq!(
            recovery.recovered(),
            &[LoweredConstraintID::OneHot(
                crate::OneHotConstraintID::from(2)
            )]
        );
        assert_eq!(recovery.skipped().len(), 1);
        assert_eq!(
            recovery.skipped()[0].constraint_id(),
            LoweredConstraintID::OneHot(crate::OneHotConstraintID::from(1))
        );
        assert!(recovery.skipped()[0]
            .reason()
            .contains("canonical V1 equality exactly"));
        assert_eq!(
            recovery.restored_capabilities(),
            &requested([AdditionalCapability::OneHot])
        );
        assert!(instance
            .one_hot_constraints()
            .contains_key(&crate::OneHotConstraintID::from(2)));
        assert!(instance
            .removed_one_hot_constraints()
            .contains_key(&crate::OneHotConstraintID::from(1)));
    }

    #[test]
    fn public_handle_and_report_preserve_lowering_order() {
        let one_hot_members = BTreeSet::from([VariableID::from(0), VariableID::from(2)]);
        let mut instance = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::zero())
            .decision_variables(btreemap! {
                VariableID::from(0) => DecisionVariable::binary(),
                VariableID::from(1) => DecisionVariable::new(
                    Kind::Continuous,
                    Bound::new(0.0, 5.0).unwrap(),
                    crate::ATol::default(),
                ).unwrap(),
                VariableID::from(2) => DecisionVariable::binary(),
                VariableID::from(3) => DecisionVariable::new(
                    Kind::Integer,
                    Bound::new(-2.0, 3.0).unwrap(),
                    crate::ATol::default(),
                ).unwrap(),
            })
            .constraints(BTreeMap::new())
            .indicator_constraints(BTreeMap::from([(
                crate::IndicatorConstraintID::from(7),
                IndicatorConstraint::new(
                    VariableID::from(0),
                    Equality::LessThanOrEqualToZero,
                    Function::from((linear!(1) + coeff!(-2.0)).unwrap()),
                ),
            )]))
            .one_hot_constraints(BTreeMap::from([
                (
                    crate::OneHotConstraintID::from(5),
                    OneHotConstraint::new(one_hot_members.clone()).unwrap(),
                ),
                (
                    crate::OneHotConstraintID::from(3),
                    OneHotConstraint::new(one_hot_members).unwrap(),
                ),
            ]))
            .sos1_constraints(BTreeMap::from([(
                crate::Sos1ConstraintID::from(9),
                Sos1Constraint::new(BTreeSet::from([VariableID::from(2), VariableID::from(3)]))
                    .unwrap(),
            )]))
            .build()
            .unwrap();

        let expected = vec![
            LoweredConstraintID::Indicator(crate::IndicatorConstraintID::from(7)),
            LoweredConstraintID::OneHot(crate::OneHotConstraintID::from(3)),
            LoweredConstraintID::OneHot(crate::OneHotConstraintID::from(5)),
            LoweredConstraintID::Sos1(crate::Sos1ConstraintID::from(9)),
        ];
        let reduction = instance
            .reduce_capabilities_with_recovery(&Capabilities::new())
            .unwrap();
        assert_eq!(reduction.lowered_constraints(), expected);
        assert_eq!(
            reduction.converted_capabilities(),
            &requested([
                AdditionalCapability::Indicator,
                AdditionalCapability::OneHot,
                AdditionalCapability::Sos1,
            ])
        );

        let recovery = instance.recover_capability_reduction(reduction).unwrap();
        assert_eq!(recovery.recovered(), expected);
        assert!(recovery.skipped().is_empty());
    }

    #[test]
    fn public_recovery_owns_the_composed_exact_state_map() {
        let mut instance = combined_instance();
        let original_ids = instance
            .decision_variables()
            .keys()
            .copied()
            .collect::<VariableIDSet>();
        let reduction = instance
            .reduce_capabilities_with_recovery(&Capabilities::new())
            .unwrap();
        let lowered_ids = instance
            .decision_variables()
            .keys()
            .copied()
            .collect::<VariableIDSet>();
        let selector = *lowered_ids
            .difference(&original_ids)
            .next()
            .expect("SOS1 lowering introduces one fresh selector");

        let recovery = instance.recover_capability_reduction(reduction).unwrap();
        assert_eq!(recovery.before_variable_ids(), &lowered_ids);
        assert_eq!(recovery.after_variable_ids(), &original_ids);
        assert_eq!(
            recovery.recovered(),
            &[
                LoweredConstraintID::Indicator(crate::IndicatorConstraintID::from(7)),
                LoweredConstraintID::Sos1(crate::Sos1ConstraintID::from(9)),
            ]
        );

        let lowered_state = v1::State::from_iter([
            (0, 1.0),
            (1, 2.0),
            (2, 0.0),
            (3, 1.0),
            (selector.into_inner(), 1.0),
        ]);
        let recovered_state = recovery.project_state(&lowered_state).unwrap();
        assert!(!recovered_state.entries.contains_key(&selector.into_inner()));
        assert_eq!(
            recovery.lift_state(&recovered_state).unwrap(),
            lowered_state
        );
        assert!(recovery
            .project_state(&v1::State::from_iter([(0, 1.0)]))
            .is_err());
    }

    #[test]
    fn public_reduction_is_atomic_across_capability_families() {
        let mut instance = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::zero())
            .decision_variables(btreemap! {
                VariableID::from(0) => DecisionVariable::binary(),
                VariableID::from(1) => DecisionVariable::new(
                    Kind::Continuous,
                    Bound::new(0.0, 2.0).unwrap(),
                    crate::ATol::default(),
                ).unwrap(),
                VariableID::from(2) => DecisionVariable::continuous(),
            })
            .constraints(BTreeMap::new())
            .indicator_constraints(BTreeMap::from([(
                crate::IndicatorConstraintID::from(1),
                IndicatorConstraint::new(
                    VariableID::from(0),
                    Equality::LessThanOrEqualToZero,
                    Function::from((linear!(1) + coeff!(-1.0)).unwrap()),
                ),
            )]))
            .sos1_constraints(BTreeMap::from([(
                crate::Sos1ConstraintID::from(2),
                Sos1Constraint::new(BTreeSet::from([VariableID::from(2)])).unwrap(),
            )]))
            .build()
            .unwrap();
        let before = instance.clone();
        let before_bytes = instance.to_v2_bytes();

        let error = instance
            .reduce_capabilities_with_recovery(&Capabilities::new())
            .unwrap_err();

        assert!(error.to_string().contains("non-finite bound"));
        assert_eq!(instance, before);
        assert_eq!(instance.to_v2_bytes(), before_bytes);
    }

    #[test]
    fn capability_reduction_handle_rejects_another_instances_batch() {
        let mut first = two_one_hot_instance();
        let first_reduction = first
            .reduce_capabilities_with_recovery(&Capabilities::new())
            .unwrap();

        let mut second = two_one_hot_instance();
        let second_reduction = second
            .reduce_capabilities_with_recovery(&Capabilities::new())
            .unwrap();
        assert_ne!(first_reduction.batch_token, second_reduction.batch_token);
        let before = second.clone();
        let before_bytes = second.to_v2_bytes();

        let recovery = second
            .recover_capability_reduction(first_reduction)
            .unwrap();

        assert!(recovery.recovered().is_empty());
        assert_eq!(recovery.skipped().len(), 2);
        assert!(recovery
            .skipped()
            .iter()
            .all(|skipped| skipped.reason().contains("not handle batch")));
        assert_eq!(second, before);
        assert_eq!(second.to_v2_bytes(), before_bytes);

        let recovery = second
            .recover_capability_reduction(second_reduction)
            .unwrap();
        assert_eq!(recovery.recovered().len(), 2);
        assert!(recovery.skipped().is_empty());
    }

    #[test]
    fn stale_handle_does_not_recover_a_later_relowering_of_the_same_ids() {
        let mut instance = two_one_hot_instance();
        let stale = instance
            .reduce_capabilities_with_recovery(&Capabilities::new())
            .unwrap();
        instance
            .restore_lowered_capabilities_checked(&requested([AdditionalCapability::OneHot]))
            .unwrap();

        let current = instance
            .reduce_capabilities_with_recovery(&Capabilities::new())
            .unwrap();
        let before = instance.clone();
        let before_bytes = instance.to_v2_bytes();

        let stale_recovery = instance.recover_capability_reduction(stale).unwrap();
        assert!(stale_recovery.recovered().is_empty());
        assert_eq!(stale_recovery.skipped().len(), 2);
        assert!(stale_recovery
            .skipped()
            .iter()
            .all(|skipped| skipped.reason().contains("not handle batch")));
        assert_eq!(instance, before);
        assert_eq!(instance.to_v2_bytes(), before_bytes);

        let current_recovery = instance.recover_capability_reduction(current).unwrap();
        assert_eq!(current_recovery.recovered().len(), 2);
        assert!(current_recovery.skipped().is_empty());
    }

    #[test]
    fn cloned_instance_preserves_the_runtime_batch_binding() {
        let mut instance = two_one_hot_instance();
        let reduction = instance
            .reduce_capabilities_with_recovery(&Capabilities::new())
            .unwrap();
        let mut cloned = instance.clone();

        let recovery = cloned.recover_capability_reduction(reduction).unwrap();

        assert_eq!(recovery.recovered().len(), 2);
        assert!(recovery.skipped().is_empty());
        assert_eq!(
            cloned.required_capabilities(),
            requested([AdditionalCapability::OneHot])
        );
    }

    #[test]
    fn partial_sos1_recovery_removes_only_the_proved_selector() {
        let continuous = || {
            DecisionVariable::new(
                Kind::Continuous,
                Bound::new(-1.0, 2.0).unwrap(),
                crate::ATol::default(),
            )
            .unwrap()
        };
        let mut instance = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::zero())
            .decision_variables(btreemap! {
                VariableID::from(0) => continuous(),
                VariableID::from(1) => continuous(),
            })
            .constraints(BTreeMap::new())
            .sos1_constraints(BTreeMap::from([
                (
                    crate::Sos1ConstraintID::from(1),
                    Sos1Constraint::new(BTreeSet::from([VariableID::from(0)])).unwrap(),
                ),
                (
                    crate::Sos1ConstraintID::from(2),
                    Sos1Constraint::new(BTreeSet::from([VariableID::from(1)])).unwrap(),
                ),
            ]))
            .build()
            .unwrap();
        let original_ids = instance
            .decision_variables()
            .keys()
            .copied()
            .collect::<VariableIDSet>();
        let reduction = instance
            .reduce_capabilities_with_recovery(&Capabilities::new())
            .unwrap();
        let lowered_ids = instance
            .decision_variables()
            .keys()
            .copied()
            .collect::<VariableIDSet>();
        let selectors = lowered_ids
            .difference(&original_ids)
            .copied()
            .collect::<Vec<_>>();
        assert_eq!(selectors.len(), 2);
        let skipped_selector = selectors[0];
        let recovered_selector = selectors[1];

        let damaged_row = first_recorded_constraint_id(
            &instance.removed_sos1_constraints()[&crate::Sos1ConstraintID::from(1)].1,
        );
        instance
            .relax_constraint(damaged_row, "presolver removed row".to_string(), [])
            .unwrap();

        let recovery = instance.recover_capability_reduction(reduction).unwrap();
        let expected_after_ids = original_ids
            .iter()
            .copied()
            .chain([skipped_selector])
            .collect::<VariableIDSet>();
        assert_eq!(recovery.before_variable_ids(), &lowered_ids);
        assert_eq!(recovery.after_variable_ids(), &expected_after_ids);
        assert_eq!(
            recovery.recovered(),
            &[LoweredConstraintID::Sos1(crate::Sos1ConstraintID::from(2))]
        );
        assert_eq!(recovery.skipped().len(), 1);
        assert_eq!(
            recovery.skipped()[0].constraint_id(),
            LoweredConstraintID::Sos1(crate::Sos1ConstraintID::from(1))
        );
        assert!(recovery.skipped()[0].reason().contains("is not active"));
        assert!(instance
            .decision_variables()
            .contains_key(&skipped_selector));
        assert!(!instance
            .decision_variables()
            .contains_key(&recovered_selector));

        let before = v1::State::from_iter([
            (0, 0.0),
            (1, 1.5),
            (skipped_selector.into_inner(), 0.0),
            (recovered_selector.into_inner(), 1.0),
        ]);
        let after = recovery.project_state(&before).unwrap();
        assert!(after.entries.contains_key(&skipped_selector.into_inner()));
        assert!(!after.entries.contains_key(&recovered_selector.into_inner()));
        assert_eq!(recovery.lift_state(&after).unwrap(), before);
    }

    #[test]
    fn public_sos1_selector_isolation_failure_is_skipped() {
        let mut instance = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::zero())
            .decision_variables(btreemap! {
                VariableID::from(0) => DecisionVariable::new(
                    Kind::Continuous,
                    Bound::new(-1.0, 2.0).unwrap(),
                    crate::ATol::default(),
                ).unwrap(),
            })
            .constraints(BTreeMap::new())
            .sos1_constraints(BTreeMap::from([(
                crate::Sos1ConstraintID::from(1),
                Sos1Constraint::new(BTreeSet::from([VariableID::from(0)])).unwrap(),
            )]))
            .build()
            .unwrap();
        let original_ids = instance
            .decision_variables()
            .keys()
            .copied()
            .collect::<VariableIDSet>();
        let reduction = instance
            .reduce_capabilities_with_recovery(&Capabilities::new())
            .unwrap();
        let selector = *instance
            .decision_variables()
            .keys()
            .copied()
            .collect::<VariableIDSet>()
            .difference(&original_ids)
            .next()
            .expect("SOS1 lowering introduces one fresh selector");
        instance
            .add_constraint(
                Constraint::equal_to_zero(Function::from(linear!(selector.into_inner()))),
                Default::default(),
            )
            .unwrap();
        let before = instance.clone();
        let before_bytes = instance.to_v2_bytes();

        let recovery = instance.recover_capability_reduction(reduction).unwrap();

        assert!(recovery.recovered().is_empty());
        assert_eq!(recovery.skipped().len(), 1);
        assert_eq!(
            recovery.skipped()[0].constraint_id(),
            LoweredConstraintID::Sos1(crate::Sos1ConstraintID::from(1))
        );
        assert!(recovery.skipped()[0]
            .reason()
            .contains("used by active regular constraint"));
        assert_eq!(instance, before);
        assert_eq!(instance.to_v2_bytes(), before_bytes);
    }

    #[test]
    fn public_operation_error_is_propagated_and_atomic() {
        let mut instance = two_one_hot_instance();
        let mut reduction = instance
            .reduce_capabilities_with_recovery(&Capabilities::new())
            .unwrap();
        // The handle is opaque to SDK callers. Corrupt its internal permission
        // only here to exercise the operation-error branch deterministically.
        reduction.converted_capabilities.clear();
        let before = instance.clone();
        let before_bytes = instance.to_v2_bytes();

        let error = instance
            .recover_capability_reduction(reduction)
            .unwrap_err();

        assert!(error
            .to_string()
            .contains("requires explicit OneHot capability permission"));
        assert_eq!(instance, before);
        assert_eq!(instance.to_v2_bytes(), before_bytes);
    }
}
