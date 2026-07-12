use super::{
    EqualityRef, ExactAffine, ExactRational, InequalityRef, LinearProofError,
    LinearRelaxationFingerprint,
};
use crate::{
    constraint::{ConstraintContext, Provenance, RemovedReason},
    instance::{
        CAPABILITY_REDUCTION_BATCH_TOKEN_PARAMETER, GENERATED_CONSTRAINT_IDS_PARAMETER,
        INDICATOR_LOWERING_REASON, ONE_HOT_GENERATED_CONSTRAINT_ID_PARAMETER,
        ONE_HOT_LOWERING_REASON, SOS1_LOWERING_REASON,
    },
    proof::exact::from_f64,
    Bound, ConstraintID, Equality, IndicatorConstraint, IndicatorConstraintID, Instance, Kind,
    ModelingLabel, OneHotConstraint, OneHotConstraintID, Sos1Constraint, Sos1ConstraintID,
    VariableID,
};
use std::collections::{BTreeMap, BTreeSet};

/// Untrusted, versioned lookup result for one historical Indicator Big-M
/// lowering. The generated rows have not yet been proven equivalent to the
/// removed source.
#[derive(Debug, Clone, PartialEq, Eq)]
struct IndicatorBigMV1 {
    source: IndicatorConstraintID,
    generated_rows: Vec<ConstraintID>,
    representation: LinearRelaxationFingerprint,
}

/// Untrusted, versioned lookup result for one historical OneHot lowering.
/// The generated equality and transferred context remain to be verified.
#[derive(Debug, Clone, PartialEq, Eq)]
struct OneHotV1 {
    source: OneHotConstraintID,
    generated_row: ConstraintID,
    representation: LinearRelaxationFingerprint,
}

/// Untrusted, versioned lookup result for one historical SOS1 Big-M lowering.
/// Selector roles and generated-row semantics remain to be verified.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Sos1BigMV1 {
    source: Sos1ConstraintID,
    generated_rows: Vec<ConstraintID>,
    representation: LinearRelaxationFingerprint,
}

/// Exact, representation-bound fact that the recorded regular rows are
/// semantically equivalent to one retained removed Indicator constraint over
/// the current binary branches.
///
/// This remains crate-private proof output. It is not a reusable mutation plan:
/// `Instance` immediately derives a one-shot storage plan from it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct VerifiedIndicatorBigMV1 {
    source_id: IndicatorConstraintID,
    source: IndicatorConstraint,
    generated_rows: Vec<ConstraintID>,
    representation: LinearRelaxationFingerprint,
}

impl VerifiedIndicatorBigMV1 {
    pub(crate) fn source_id(&self) -> IndicatorConstraintID {
        self.source_id
    }

    pub(crate) fn source(&self) -> &IndicatorConstraint {
        &self.source
    }

    pub(crate) fn generated_rows(&self) -> &[ConstraintID] {
        &self.generated_rows
    }
}

/// Exact, representation-bound fact that one retained removed OneHot
/// constraint is represented by the current canonical V1 equality.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct VerifiedOneHotV1 {
    source_id: OneHotConstraintID,
    source: OneHotConstraint,
    generated_row: ConstraintID,
    representation: LinearRelaxationFingerprint,
}

impl VerifiedOneHotV1 {
    pub(crate) fn source_id(&self) -> OneHotConstraintID {
        self.source_id
    }

    pub(crate) fn source(&self) -> &OneHotConstraint {
        &self.source
    }

    pub(crate) fn generated_row(&self) -> ConstraintID {
        self.generated_row
    }
}

/// Exact, representation-bound fact that the recorded regular rows are the
/// complete current V1 lowering of one retained removed SOS1 constraint.
///
/// `selectors` maps every source member to either itself (the binary-reuse
/// case) or to one verified fresh binary selector. The root `Instance` still
/// has to prove that every fresh selector is isolated before removing it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct VerifiedSos1BigMV1 {
    source_id: Sos1ConstraintID,
    source: Sos1Constraint,
    generated_rows: Vec<ConstraintID>,
    selectors: BTreeMap<VariableID, VariableID>,
    representation: LinearRelaxationFingerprint,
}

impl VerifiedSos1BigMV1 {
    pub(crate) fn source_id(&self) -> Sos1ConstraintID {
        self.source_id
    }

    pub(crate) fn source(&self) -> &Sos1Constraint {
        &self.source
    }

    pub(crate) fn generated_rows(&self) -> &[ConstraintID] {
        &self.generated_rows
    }

    pub(crate) fn selectors(&self) -> &BTreeMap<VariableID, VariableID> {
        &self.selectors
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
enum InverseLoweringCandidateError {
    #[error("removed Indicator constraint {id:?} was not found")]
    MissingRemovedIndicator { id: IndicatorConstraintID },
    #[error("removed OneHot constraint {id:?} was not found")]
    MissingRemovedOneHot { id: OneHotConstraintID },
    #[error("removed SOS1 constraint {id:?} was not found")]
    MissingRemovedSos1 { id: Sos1ConstraintID },
    #[error("expected lowering reason {expected}, found {actual}")]
    UnexpectedRemovalReason {
        expected: &'static str,
        actual: String,
    },
    #[error("lowering reason is missing the {GENERATED_CONSTRAINT_IDS_PARAMETER} parameter")]
    MissingGeneratedConstraintIds,
    #[error(
        "lowering reason is missing the {ONE_HOT_GENERATED_CONSTRAINT_ID_PARAMETER} parameter"
    )]
    MissingOneHotGeneratedConstraintId,
    #[error("lowering reason contains unsupported V1 parameter {key:?}")]
    UnexpectedRemovalParameter { key: String },
    #[error("generated constraint ID token {token:?} is not canonical u64 text")]
    InvalidGeneratedConstraintId { token: String },
    #[error("capability-reduction batch token {token:?} is not canonical UUID text")]
    InvalidCapabilityReductionBatchToken { token: String },
    #[error("generated constraint ID {id:?} is listed more than once")]
    DuplicateGeneratedConstraintId { id: ConstraintID },
    #[error("generated regular constraint {id:?} is not active")]
    MissingGeneratedConstraint { id: ConstraintID },
    #[error("generated regular constraint {id:?} has unexpected provenance")]
    ProvenanceMismatch { id: ConstraintID },
    #[error("generated regular constraint {id:?} has a modeling label that would be discarded")]
    GeneratedConstraintHasModelingLabel { id: ConstraintID },
    #[error("generated regular constraint {id:?} does not have the exact transferred context")]
    GeneratedConstraintContextMismatch { id: ConstraintID },
    #[error("active regular constraint {id:?} claims the lowering provenance but is not recorded")]
    UnrecordedGeneratedConstraint { id: ConstraintID },
    #[error(transparent)]
    LinearProof(#[from] LinearProofError),
}

impl Instance {
    fn indicator_big_m_candidate_v1(
        &self,
        id: IndicatorConstraintID,
    ) -> Result<IndicatorBigMV1, InverseLoweringCandidateError> {
        let (_, reason) = self
            .removed_indicator_constraints()
            .get(&id)
            .ok_or(InverseLoweringCandidateError::MissingRemovedIndicator { id })?;
        let generated_rows = parse_generated_constraint_ids(reason, INDICATOR_LOWERING_REASON)?;
        self.validate_candidate_rows(&generated_rows, Provenance::IndicatorConstraint(id))?;
        let representation = self.certified_linear_relaxation()?.fingerprint();
        Ok(IndicatorBigMV1 {
            source: id,
            generated_rows,
            representation,
        })
    }

    fn one_hot_candidate_v1(
        &self,
        id: OneHotConstraintID,
    ) -> Result<OneHotV1, InverseLoweringCandidateError> {
        let (_, reason) = self
            .removed_one_hot_constraints()
            .get(&id)
            .ok_or(InverseLoweringCandidateError::MissingRemovedOneHot { id })?;
        let generated_row = parse_one_hot_constraint_id(reason)?;

        // OneHot lowering transfers the source's complete modeling context to
        // the regular row and appends exactly one lineage step. Unlike the
        // Indicator/SOS1 lowerers, a non-default generated label is expected.
        let mut expected_context = self.one_hot_constraint_context().collect_for(id);
        let expected_provenance = Provenance::OneHotConstraint(id);
        expected_context
            .provenance
            .push(expected_provenance.clone());
        self.validate_one_hot_candidate_row(generated_row, &expected_context, expected_provenance)?;

        let representation = self.certified_linear_relaxation()?.fingerprint();
        Ok(OneHotV1 {
            source: id,
            generated_row,
            representation,
        })
    }

    fn sos1_big_m_candidate_v1(
        &self,
        id: Sos1ConstraintID,
    ) -> Result<Sos1BigMV1, InverseLoweringCandidateError> {
        let (_, reason) = self
            .removed_sos1_constraints()
            .get(&id)
            .ok_or(InverseLoweringCandidateError::MissingRemovedSos1 { id })?;
        let generated_rows = parse_generated_constraint_ids(reason, SOS1_LOWERING_REASON)?;
        self.validate_candidate_rows(&generated_rows, Provenance::Sos1Constraint(id))?;
        let representation = self.certified_linear_relaxation()?.fingerprint();
        Ok(Sos1BigMV1 {
            source: id,
            generated_rows,
            representation,
        })
    }

    fn validate_candidate_rows(
        &self,
        generated_rows: &[ConstraintID],
        expected_provenance: Provenance,
    ) -> Result<(), InverseLoweringCandidateError> {
        let recorded = generated_rows.iter().copied().collect::<BTreeSet<_>>();
        for id in generated_rows {
            if !self.constraints().contains_key(id) {
                return Err(InverseLoweringCandidateError::MissingGeneratedConstraint { id: *id });
            }
            if self.constraint_context().provenance(*id) != [expected_provenance.clone()] {
                return Err(InverseLoweringCandidateError::ProvenanceMismatch { id: *id });
            }
            if self.constraint_context().collect_for(*id).label != Default::default() {
                return Err(
                    InverseLoweringCandidateError::GeneratedConstraintHasModelingLabel { id: *id },
                );
            }
        }
        for id in self.constraints().keys() {
            if !recorded.contains(id)
                && self
                    .constraint_context()
                    .provenance(*id)
                    .contains(&expected_provenance)
            {
                return Err(
                    InverseLoweringCandidateError::UnrecordedGeneratedConstraint { id: *id },
                );
            }
        }
        Ok(())
    }

    fn validate_one_hot_candidate_row(
        &self,
        generated_row: ConstraintID,
        expected_context: &ConstraintContext,
        expected_provenance: Provenance,
    ) -> Result<(), InverseLoweringCandidateError> {
        if !self.constraints().contains_key(&generated_row) {
            return Err(InverseLoweringCandidateError::MissingGeneratedConstraint {
                id: generated_row,
            });
        }
        if self.constraint_context().collect_for(generated_row) != *expected_context {
            return Err(
                InverseLoweringCandidateError::GeneratedConstraintContextMismatch {
                    id: generated_row,
                },
            );
        }
        for id in self.constraints().keys() {
            if *id != generated_row
                && self
                    .constraint_context()
                    .provenance(*id)
                    .contains(&expected_provenance)
            {
                return Err(
                    InverseLoweringCandidateError::UnrecordedGeneratedConstraint { id: *id },
                );
            }
        }
        Ok(())
    }
}

/// Verify the complete current V1 OneHot lowering exactly.
///
/// The single generated row must be the canonical equality
/// `sum(member) - 1 = 0`, every member must retain a binary domain, and the
/// row's context must be the exact source context plus the lowering lineage
/// step. Scalar multiples are deliberately rejected: this verifier consumes a
/// current OMMX history rather than recognizing arbitrary flat models.
pub(crate) fn verify_one_hot_v1(
    instance: &Instance,
    id: OneHotConstraintID,
) -> crate::Result<VerifiedOneHotV1> {
    let candidate = instance.one_hot_candidate_v1(id)?;
    let snapshot = instance.certified_linear_relaxation()?;
    if candidate.representation != snapshot.fingerprint() {
        anyhow::bail!(
            "OneHot inverse-lowering candidate {id:?} is bound to a stale representation"
        );
    }

    let (source, _) = instance
        .removed_one_hot_constraints()
        .get(&id)
        .expect("candidate lookup proved that the removed source exists");
    if source.variables.is_empty() {
        anyhow::bail!("Removed OneHot constraint {id:?} has no members");
    }
    for &member in &source.variables {
        let Some(variable) = instance.decision_variables().get(&member) else {
            anyhow::bail!("OneHot member {member:?} is not registered");
        };
        if variable.kind() != Kind::Binary {
            anyhow::bail!("OneHot member {member:?} is not binary");
        }
    }

    let actual = snapshot.equality(EqualityRef::RegularConstraint(candidate.generated_row))?;
    let one = ExactRational::from_integer(1.into());
    let expected = ExactAffine {
        coefficients: source
            .variables
            .iter()
            .copied()
            .map(|member| (member, one.clone()))
            .collect(),
        constant: ExactRational::from_integer((-1).into()),
    };
    if actual != expected {
        anyhow::bail!("Generated OneHot row does not match the canonical V1 equality exactly");
    }

    Ok(VerifiedOneHotV1 {
        source_id: candidate.source,
        source: source.clone(),
        generated_row: candidate.generated_row,
        representation: candidate.representation,
    })
}

/// Verify the current V1 Indicator Big-M history exactly.
///
/// For each generated row, substitution of the active branch must equal the
/// corresponding Indicator side exactly, while substitution of the inactive
/// branch must be implied by exact variable-domain facts. A missing side is
/// accepted only when the Indicator side itself is implied by those facts.
/// Generated rows are never used to prove their own redundancy.
pub(crate) fn verify_indicator_big_m_v1(
    instance: &Instance,
    id: IndicatorConstraintID,
) -> crate::Result<VerifiedIndicatorBigMV1> {
    let candidate = instance.indicator_big_m_candidate_v1(id)?;
    let snapshot = instance.certified_linear_relaxation()?;
    if candidate.representation != snapshot.fingerprint() {
        anyhow::bail!(
            "Indicator inverse-lowering candidate {id:?} is bound to a stale representation"
        );
    }

    let (source, _) = instance
        .removed_indicator_constraints()
        .get(&id)
        .expect("candidate lookup proved that the removed source exists");
    let indicator_variable = source.indicator_variable;
    let Some(indicator) = instance.decision_variables().get(&indicator_variable) else {
        anyhow::bail!("Indicator variable {indicator_variable:?} is not registered");
    };
    if indicator.kind() != Kind::Binary {
        anyhow::bail!("Indicator variable {indicator_variable:?} is not binary");
    }

    let body = ExactAffine::from_function(source.function())?.ok_or_else(|| {
        anyhow::anyhow!("Removed Indicator constraint {id:?} has a nonlinear body")
    })?;
    let zero = ExactRational::from_integer(0.into());
    let one = ExactRational::from_integer(1.into());
    let active_upper = body.substitute(indicator_variable, &one);
    let active_lower = active_upper.negated();

    let mut roles = Vec::with_capacity(candidate.generated_rows.len());
    for row_id in &candidate.generated_rows {
        let row = snapshot.inequality(InequalityRef::RegularConstraint(*row_id))?;
        let inactive = row.substitute(indicator_variable, &zero);
        if !snapshot.variable_facts_imply_nonpositive(&inactive)? {
            anyhow::bail!(
                "Generated Indicator row {row_id:?} is not redundant on the inactive branch"
            );
        }
        let active = row.substitute(indicator_variable, &one);
        roles.push((active == active_upper, active == active_lower));
    }

    let upper_implied = snapshot.variable_facts_imply_nonpositive(&active_upper)?;
    match source.equality {
        Equality::LessThanOrEqualToZero => {
            if roles.len() > 1 {
                anyhow::bail!("Inequality Indicator {id:?} has more than one generated row");
            }
            if let Some((matches_upper, _)) = roles.first() {
                if !matches_upper {
                    anyhow::bail!(
                        "Generated Indicator row does not equal the active upper side exactly"
                    );
                }
            } else if !upper_implied {
                anyhow::bail!(
                    "Missing Indicator upper side is not implied by exact variable facts"
                );
            }
        }
        Equality::EqualToZero => {
            if roles.len() > 2 {
                anyhow::bail!("Equality Indicator {id:?} has more than two generated rows");
            }
            let lower_implied = snapshot.variable_facts_imply_nonpositive(&active_lower)?;
            if !equality_roles_cover(&roles, upper_implied, lower_implied) {
                anyhow::bail!(
                    "Generated Indicator rows do not provide independent exact upper and lower active sides"
                );
            }
        }
    }

    Ok(VerifiedIndicatorBigMV1 {
        source_id: candidate.source,
        source: source.clone(),
        generated_rows: candidate.generated_rows,
        representation: candidate.representation,
    })
}

/// Verify the complete current V1 SOS1 Big-M lowering exactly.
///
/// The V1 contract is intentionally strict: selector roles are reconstructed
/// from the exact generated-variable label, every selector has the original
/// binary domain, and generated rows must appear in the lowerer's canonical
/// order with exact dyadic content. For fresh selectors this checks every
/// nontrivial bound link and the final cardinality row; reused binary members
/// contribute only to cardinality.
///
/// Together with the current member bounds, these rows prove both directions:
/// projection of a feasible gadget satisfies SOS1, and the canonical lift
/// `selector = 1 iff member != 0` satisfies the gadget for every feasible SOS1
/// assignment. These are exact mathematical statements: tolerance-based
/// classification of a tiny nonzero value by `Sos1Constraint::evaluate` is not
/// an alternate zero test for the lift. Fresh-selector isolation is a separate
/// root-owned obligation.
pub(crate) fn verify_sos1_big_m_v1(
    instance: &Instance,
    id: Sos1ConstraintID,
) -> crate::Result<VerifiedSos1BigMV1> {
    let candidate = instance.sos1_big_m_candidate_v1(id)?;
    let snapshot = instance.certified_linear_relaxation()?;
    if candidate.representation != snapshot.fingerprint() {
        anyhow::bail!("SOS1 inverse-lowering candidate {id:?} is bound to a stale representation");
    }

    let (source, _) = instance
        .removed_sos1_constraints()
        .get(&id)
        .expect("candidate lookup proved that the removed source exists");
    let mut selectors = BTreeMap::new();
    let mut fresh_member_bounds = BTreeMap::new();
    let mut expected_row_count = 1usize; // The final cardinality row.

    for &member in &source.variables {
        let variable = instance
            .decision_variables()
            .get(&member)
            .ok_or_else(|| anyhow::anyhow!("SOS1 member {member:?} is not registered"))?;
        let bound = variable.bound();
        if variable.kind() == Kind::Binary && bound == Bound::of_binary() {
            selectors.insert(member, member);
            continue;
        }
        if matches!(variable.kind(), Kind::SemiContinuous | Kind::SemiInteger) {
            anyhow::bail!(
                "SOS1 member {member:?} has a semi-variable kind unsupported by V1 lowering"
            );
        }
        if !bound.is_finite() || bound.lower() > 0.0 || bound.upper() < 0.0 {
            anyhow::bail!(
                "SOS1 member {member:?} does not have a finite V1 Big-M domain containing zero"
            );
        }
        expected_row_count += usize::from(bound.upper() > 0.0);
        expected_row_count += usize::from(bound.lower() < 0.0);
        fresh_member_bounds.insert(member, bound);
    }

    if candidate.generated_rows.len() != expected_row_count {
        anyhow::bail!("SOS1 lowering has an unexpected number of generated rows");
    }

    // The V1 lowerer emits cardinality last. Read selector candidates only
    // from that exact row so an unrelated variable with the same generated
    // label cannot make an otherwise valid lowering ambiguous.
    let cardinality_id = *candidate
        .generated_rows
        .last()
        .expect("a non-empty SOS1 V1 lowering always has a cardinality row");
    let cardinality = snapshot.inequality(InequalityRef::RegularConstraint(cardinality_id))?;
    let one = ExactRational::from_integer(1.into());
    let minus_one = ExactRational::from_integer((-1).into());
    if cardinality.constant() != &minus_one
        || cardinality.coefficients().len() != source.variables.len()
        || cardinality
            .coefficients()
            .values()
            .any(|coefficient| coefficient != &one)
    {
        anyhow::bail!("Generated SOS1 cardinality row does not have canonical V1 shape");
    }
    for (&member, &selector) in &selectors {
        debug_assert_eq!(member, selector);
        if !cardinality.coefficients().contains_key(&selector) {
            anyhow::bail!("Generated SOS1 cardinality row is missing reused member {member:?}");
        }
    }

    let fresh_selector_candidates = cardinality
        .coefficients()
        .keys()
        .copied()
        .filter(|selector| !selectors.values().any(|reused| reused == selector))
        .collect::<BTreeSet<_>>();
    if fresh_selector_candidates.len() != fresh_member_bounds.len()
        || fresh_selector_candidates
            .iter()
            .any(|selector| source.variables.contains(selector))
    {
        anyhow::bail!("Generated SOS1 cardinality row has an invalid fresh-selector set");
    }

    for &member in fresh_member_bounds.keys() {
        let expected_label = sos1_selector_label_v1(id, member);
        let matching_selectors = fresh_selector_candidates
            .iter()
            .copied()
            .filter(|selector| instance.variable_labels().collect_for(*selector) == expected_label)
            .collect::<Vec<_>>();
        let [selector] = matching_selectors.as_slice() else {
            anyhow::bail!("SOS1 member {member:?} does not have exactly one canonical V1 selector");
        };
        let selector = *selector;
        let selector_variable = instance
            .decision_variables()
            .get(&selector)
            .expect("selector ID was read from the decision-variable table");
        if selector_variable.kind() != Kind::Binary
            || selector_variable.bound() != Bound::of_binary()
        {
            anyhow::bail!("SOS1 selector {selector:?} does not have the canonical binary domain");
        }
        if selectors.values().any(|existing| *existing == selector) {
            anyhow::bail!("SOS1 selector {selector:?} is assigned to more than one member");
        }
        selectors.insert(member, selector);
    }

    let mut expected_rows = Vec::with_capacity(expected_row_count);
    for (&member, &bound) in &fresh_member_bounds {
        let selector = selectors[&member];
        if bound.upper() > 0.0 {
            expected_rows.push(ExactAffine {
                coefficients: BTreeMap::from([
                    (member, ExactRational::from_integer(1.into())),
                    (selector, -from_f64(bound.upper())?),
                ]),
                constant: ExactRational::from_integer(0.into()),
            });
        }
        if bound.lower() < 0.0 {
            expected_rows.push(ExactAffine {
                coefficients: BTreeMap::from([
                    (member, ExactRational::from_integer((-1).into())),
                    (selector, from_f64(bound.lower())?),
                ]),
                constant: ExactRational::from_integer(0.into()),
            });
        }
    }

    let cardinality_coefficients = selectors
        .values()
        .copied()
        .map(|selector| (selector, ExactRational::from_integer(1.into())))
        .collect();
    expected_rows.push(ExactAffine {
        coefficients: cardinality_coefficients,
        constant: ExactRational::from_integer((-1).into()),
    });

    debug_assert_eq!(expected_rows.len(), expected_row_count);
    for (index, (&row_id, expected)) in candidate
        .generated_rows
        .iter()
        .zip(&expected_rows)
        .enumerate()
    {
        let actual = snapshot.inequality(InequalityRef::RegularConstraint(row_id))?;
        if actual != *expected {
            anyhow::bail!(
                "Generated SOS1 row {row_id:?} at index {index} does not match the canonical V1 content exactly"
            );
        }
    }

    Ok(VerifiedSos1BigMV1 {
        source_id: candidate.source,
        source: source.clone(),
        generated_rows: candidate.generated_rows,
        selectors,
        representation: candidate.representation,
    })
}

fn sos1_selector_label_v1(id: Sos1ConstraintID, member: VariableID) -> ModelingLabel {
    ModelingLabel {
        name: Some("ommx.sos1_indicator".to_string()),
        subscripts: vec![id.into_inner() as i64, member.into_inner() as i64],
        ..Default::default()
    }
}

fn equality_roles_cover(roles: &[(bool, bool)], upper_implied: bool, lower_implied: bool) -> bool {
    for assignment in 0..(1usize << roles.len()) {
        let mut upper_used = false;
        let mut lower_used = false;
        let mut valid = true;
        for (index, &(matches_upper, matches_lower)) in roles.iter().enumerate() {
            if assignment & (1 << index) == 0 {
                if !matches_upper || upper_used {
                    valid = false;
                    break;
                }
                upper_used = true;
            } else {
                if !matches_lower || lower_used {
                    valid = false;
                    break;
                }
                lower_used = true;
            }
        }
        if valid && (upper_used || upper_implied) && (lower_used || lower_implied) {
            return true;
        }
    }
    false
}

fn parse_one_hot_constraint_id(
    reason: &RemovedReason,
) -> Result<ConstraintID, InverseLoweringCandidateError> {
    if reason.reason != ONE_HOT_LOWERING_REASON {
        return Err(InverseLoweringCandidateError::UnexpectedRemovalReason {
            expected: ONE_HOT_LOWERING_REASON,
            actual: reason.reason.clone(),
        });
    }
    if let Some(key) = reason
        .parameters
        .keys()
        .filter(|key| {
            key.as_str() != ONE_HOT_GENERATED_CONSTRAINT_ID_PARAMETER
                && key.as_str() != CAPABILITY_REDUCTION_BATCH_TOKEN_PARAMETER
        })
        .min()
    {
        return Err(InverseLoweringCandidateError::UnexpectedRemovalParameter { key: key.clone() });
    }
    validate_optional_capability_reduction_batch_token(reason)?;
    let token = reason
        .parameters
        .get(ONE_HOT_GENERATED_CONSTRAINT_ID_PARAMETER)
        .ok_or(InverseLoweringCandidateError::MissingOneHotGeneratedConstraintId)?;
    let value = token.parse::<u64>().map_err(|_| {
        InverseLoweringCandidateError::InvalidGeneratedConstraintId {
            token: token.clone(),
        }
    })?;
    if token != &value.to_string() {
        return Err(
            InverseLoweringCandidateError::InvalidGeneratedConstraintId {
                token: token.clone(),
            },
        );
    }
    Ok(ConstraintID::from(value))
}

fn parse_generated_constraint_ids(
    reason: &RemovedReason,
    expected_reason: &'static str,
) -> Result<Vec<ConstraintID>, InverseLoweringCandidateError> {
    if reason.reason != expected_reason {
        return Err(InverseLoweringCandidateError::UnexpectedRemovalReason {
            expected: expected_reason,
            actual: reason.reason.clone(),
        });
    }
    if let Some(key) = reason
        .parameters
        .keys()
        .filter(|key| {
            key.as_str() != GENERATED_CONSTRAINT_IDS_PARAMETER
                && key.as_str() != CAPABILITY_REDUCTION_BATCH_TOKEN_PARAMETER
        })
        .min()
    {
        return Err(InverseLoweringCandidateError::UnexpectedRemovalParameter { key: key.clone() });
    }
    validate_optional_capability_reduction_batch_token(reason)?;
    let raw = reason
        .parameters
        .get(GENERATED_CONSTRAINT_IDS_PARAMETER)
        .ok_or(InverseLoweringCandidateError::MissingGeneratedConstraintIds)?;
    if raw.is_empty() {
        return Ok(Vec::new());
    }

    let mut seen = BTreeSet::new();
    let mut ids = Vec::new();
    for token in raw.split(',') {
        let value = token.parse::<u64>().map_err(|_| {
            InverseLoweringCandidateError::InvalidGeneratedConstraintId {
                token: token.to_string(),
            }
        })?;
        if token != value.to_string() {
            return Err(
                InverseLoweringCandidateError::InvalidGeneratedConstraintId {
                    token: token.to_string(),
                },
            );
        }
        let id = ConstraintID::from(value);
        if !seen.insert(id) {
            return Err(InverseLoweringCandidateError::DuplicateGeneratedConstraintId { id });
        }
        ids.push(id);
    }
    Ok(ids)
}

fn validate_optional_capability_reduction_batch_token(
    reason: &RemovedReason,
) -> Result<(), InverseLoweringCandidateError> {
    let Some(token) = reason
        .parameters
        .get(CAPABILITY_REDUCTION_BATCH_TOKEN_PARAMETER)
    else {
        return Ok(());
    };
    let value = uuid::Uuid::parse_str(token).map_err(|_| {
        InverseLoweringCandidateError::InvalidCapabilityReductionBatchToken {
            token: token.clone(),
        }
    })?;
    if token != &value.hyphenated().to_string() {
        return Err(
            InverseLoweringCandidateError::InvalidCapabilityReductionBatchToken {
                token: token.clone(),
            },
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        coeff, linear, Bound, Constraint, ConstraintContextStore, DecisionVariable, Equality,
        Function, IndicatorConstraint, Kind, ModelingLabel, OneHotConstraint, Sense,
        Sos1Constraint, VariableID, VariableLabelStore,
    };
    use fnv::FnvHashMap;
    use std::collections::{BTreeMap, BTreeSet};

    fn removed_reason(reason: &str, ids: &str) -> RemovedReason {
        RemovedReason {
            reason: reason.to_string(),
            parameters: FnvHashMap::from_iter([(
                GENERATED_CONSTRAINT_IDS_PARAMETER.to_string(),
                ids.to_string(),
            )]),
        }
    }

    fn forged_indicator_semantics(
        x_bound: Bound,
        source_equality: Equality,
        source_function: Function,
        generated: Vec<Constraint>,
    ) -> Instance {
        let generated_ids = generated
            .iter()
            .enumerate()
            .map(|(offset, _)| ConstraintID::from(10 + offset as u64))
            .collect::<Vec<_>>();
        let constraints = generated_ids
            .iter()
            .copied()
            .zip(generated)
            .collect::<BTreeMap<_, _>>();
        let mut context = ConstraintContextStore::default();
        for id in &generated_ids {
            context.set_provenance(
                *id,
                vec![Provenance::IndicatorConstraint(
                    IndicatorConstraintID::from(7),
                )],
            );
        }
        let generated_text = generated_ids
            .iter()
            .map(|id| id.into_inner().to_string())
            .collect::<Vec<_>>()
            .join(",");
        Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::zero())
            .decision_variables(BTreeMap::from([
                (
                    VariableID::from(1),
                    DecisionVariable::new(Kind::Continuous, x_bound, crate::ATol::default())
                        .unwrap(),
                ),
                (VariableID::from(2), DecisionVariable::binary()),
            ]))
            .constraints(constraints)
            .constraint_context(context)
            .removed_indicator_constraints(BTreeMap::from([(
                IndicatorConstraintID::from(7),
                (
                    IndicatorConstraint::new(VariableID::from(2), source_equality, source_function),
                    removed_reason(INDICATOR_LOWERING_REASON, &generated_text),
                ),
            )]))
            .build()
            .unwrap()
    }

    fn current_one_hot_history() -> (Instance, ConstraintID) {
        let one_hot_id = OneHotConstraintID::from(7);
        let mut instance = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::zero())
            .decision_variables(BTreeMap::from([
                (VariableID::from(1), DecisionVariable::binary()),
                (VariableID::from(2), DecisionVariable::binary()),
            ]))
            .constraints(BTreeMap::new())
            .one_hot_constraints(BTreeMap::from([(
                one_hot_id,
                OneHotConstraint::new(BTreeSet::from([VariableID::from(1), VariableID::from(2)]))
                    .unwrap(),
            )]))
            .build()
            .unwrap();
        instance
            .set_one_hot_constraint_context(
                one_hot_id,
                ConstraintContext {
                    label: ModelingLabel {
                        name: Some("choose".to_string()),
                        ..Default::default()
                    },
                    provenance: vec![Provenance::Sos1Constraint(Sos1ConstraintID::from(3))],
                },
            )
            .unwrap();
        let generated = instance.convert_one_hot_to_constraint(one_hot_id).unwrap();
        (instance, generated)
    }

    #[test]
    fn verifies_current_one_hot_lowering_with_exact_transferred_context() {
        let (instance, generated) = current_one_hot_history();
        let candidate = instance
            .one_hot_candidate_v1(OneHotConstraintID::from(7))
            .unwrap();
        assert_eq!(candidate.source, OneHotConstraintID::from(7));
        assert_eq!(candidate.generated_row, generated);

        let verified = verify_one_hot_v1(&instance, OneHotConstraintID::from(7)).unwrap();
        assert_eq!(verified.source_id(), OneHotConstraintID::from(7));
        assert_eq!(verified.generated_row(), generated);
        assert_eq!(
            verified.source().variables,
            BTreeSet::from([VariableID::from(1), VariableID::from(2)])
        );
    }

    #[test]
    fn rejects_one_hot_scalar_multiple_and_context_edits() {
        let (instance, generated) = current_one_hot_history();

        let mut scaled = instance.clone();
        scaled
            .insert_constraint(
                generated,
                Constraint::equal_to_zero(Function::from(
                    (((coeff!(2.0) * linear!(1)).unwrap() + (coeff!(2.0) * linear!(2)).unwrap())
                        .unwrap()
                        + coeff!(-2.0))
                    .unwrap(),
                )),
            )
            .unwrap();
        let error = verify_one_hot_v1(&scaled, OneHotConstraintID::from(7)).unwrap_err();
        assert!(error.to_string().contains("canonical V1 equality exactly"));

        let mut relabeled = instance;
        let mut context = relabeled.constraint_context().collect_for(generated);
        context.label.name = Some("presolver-edited".to_string());
        relabeled
            .set_constraint_context(generated, context)
            .unwrap();
        let error = verify_one_hot_v1(&relabeled, OneHotConstraintID::from(7)).unwrap_err();
        assert!(error.to_string().contains("exact transferred context"));
    }

    #[test]
    fn rejects_unrecorded_one_hot_provenance_and_malformed_parameters() {
        let (mut instance, _) = current_one_hot_history();
        let extra = instance
            .add_constraint(
                Constraint::equal_to_zero(Function::zero()),
                ConstraintContext {
                    label: Default::default(),
                    provenance: vec![Provenance::OneHotConstraint(OneHotConstraintID::from(7))],
                },
            )
            .unwrap();
        let error = verify_one_hot_v1(&instance, OneHotConstraintID::from(7)).unwrap_err();
        assert!(error.to_string().contains(&format!("{extra:?}")));
        assert!(error.to_string().contains("not recorded"));

        let missing = RemovedReason {
            reason: ONE_HOT_LOWERING_REASON.to_string(),
            parameters: FnvHashMap::default(),
        };
        assert!(matches!(
            parse_one_hot_constraint_id(&missing),
            Err(InverseLoweringCandidateError::MissingOneHotGeneratedConstraintId)
        ));
        for token in ["01", "1,2", "-1"] {
            let reason = RemovedReason {
                reason: ONE_HOT_LOWERING_REASON.to_string(),
                parameters: FnvHashMap::from_iter([(
                    ONE_HOT_GENERATED_CONSTRAINT_ID_PARAMETER.to_string(),
                    token.to_string(),
                )]),
            };
            assert!(matches!(
                parse_one_hot_constraint_id(&reason),
                Err(InverseLoweringCandidateError::InvalidGeneratedConstraintId { .. })
            ));
        }
        let extra_parameter = RemovedReason {
            reason: ONE_HOT_LOWERING_REASON.to_string(),
            parameters: FnvHashMap::from_iter([
                (
                    ONE_HOT_GENERATED_CONSTRAINT_ID_PARAMETER.to_string(),
                    "1".to_string(),
                ),
                ("future".to_string(), "value".to_string()),
            ]),
        };
        assert!(matches!(
            parse_one_hot_constraint_id(&extra_parameter),
            Err(InverseLoweringCandidateError::UnexpectedRemovalParameter { .. })
        ));

        let malformed_batch = RemovedReason {
            reason: ONE_HOT_LOWERING_REASON.to_string(),
            parameters: FnvHashMap::from_iter([
                (
                    ONE_HOT_GENERATED_CONSTRAINT_ID_PARAMETER.to_string(),
                    "1".to_string(),
                ),
                (
                    CAPABILITY_REDUCTION_BATCH_TOKEN_PARAMETER.to_string(),
                    "550E8400-E29B-41D4-A716-446655440000".to_string(),
                ),
            ]),
        };
        assert!(matches!(
            parse_one_hot_constraint_id(&malformed_batch),
            Err(InverseLoweringCandidateError::InvalidCapabilityReductionBatchToken { .. })
        ));
    }

    #[test]
    fn reads_current_indicator_lowering_as_untrusted_candidate() {
        let x = DecisionVariable::new(
            Kind::Continuous,
            Bound::new(0.0, 5.0).unwrap(),
            crate::ATol::default(),
        )
        .unwrap();
        let mut instance = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::zero())
            .decision_variables(BTreeMap::from([
                (VariableID::from(1), x),
                (VariableID::from(10), DecisionVariable::binary()),
            ]))
            .constraints(BTreeMap::new())
            .indicator_constraints(BTreeMap::from([(
                IndicatorConstraintID::from(7),
                IndicatorConstraint::new(
                    VariableID::from(10),
                    Equality::LessThanOrEqualToZero,
                    Function::from((linear!(1) + coeff!(-2.0)).unwrap()),
                ),
            )]))
            .build()
            .unwrap();
        let generated = instance
            .convert_indicator_to_constraint(IndicatorConstraintID::from(7))
            .unwrap();

        let candidate = instance
            .indicator_big_m_candidate_v1(IndicatorConstraintID::from(7))
            .unwrap();
        assert_eq!(candidate.source, IndicatorConstraintID::from(7));
        assert_eq!(candidate.generated_rows, generated);
        assert_eq!(
            candidate.representation,
            instance
                .certified_linear_relaxation()
                .unwrap()
                .fingerprint()
        );
    }

    #[test]
    fn verifies_current_indicator_lowering_exactly() {
        for equality in [Equality::LessThanOrEqualToZero, Equality::EqualToZero] {
            let x = DecisionVariable::new(
                Kind::Continuous,
                Bound::new(0.0, 5.0).unwrap(),
                crate::ATol::default(),
            )
            .unwrap();
            let mut instance = Instance::builder()
                .sense(Sense::Minimize)
                .objective(Function::zero())
                .decision_variables(BTreeMap::from([
                    (VariableID::from(1), x),
                    (VariableID::from(2), DecisionVariable::binary()),
                ]))
                .constraints(BTreeMap::new())
                .indicator_constraints(BTreeMap::from([(
                    IndicatorConstraintID::from(7),
                    IndicatorConstraint::new(
                        VariableID::from(2),
                        equality,
                        Function::from((linear!(1) + coeff!(-2.0)).unwrap()),
                    ),
                )]))
                .build()
                .unwrap();
            let generated = instance
                .convert_indicator_to_constraint(IndicatorConstraintID::from(7))
                .unwrap();

            let verified =
                verify_indicator_big_m_v1(&instance, IndicatorConstraintID::from(7)).unwrap();
            assert_eq!(verified.source_id(), IndicatorConstraintID::from(7));
            assert_eq!(verified.generated_rows(), generated);
        }
    }

    #[test]
    fn verifies_indicator_body_that_references_its_indicator_variable() {
        let x = DecisionVariable::new(
            Kind::Continuous,
            Bound::new(0.0, 5.0).unwrap(),
            crate::ATol::default(),
        )
        .unwrap();
        let mut instance = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::zero())
            .decision_variables(BTreeMap::from([
                (VariableID::from(1), x),
                (VariableID::from(2), DecisionVariable::binary()),
            ]))
            .constraints(BTreeMap::new())
            .indicator_constraints(BTreeMap::from([(
                IndicatorConstraintID::from(7),
                IndicatorConstraint::new(
                    VariableID::from(2),
                    Equality::LessThanOrEqualToZero,
                    Function::from(((linear!(1) + linear!(2)).unwrap() + coeff!(-3.0)).unwrap()),
                ),
            )]))
            .build()
            .unwrap();
        instance
            .convert_indicator_to_constraint(IndicatorConstraintID::from(7))
            .unwrap();

        verify_indicator_big_m_v1(&instance, IndicatorConstraintID::from(7)).unwrap();
    }

    #[test]
    fn verifies_exactly_implied_omitted_indicator_sides() {
        let mut inequality = forged_indicator_semantics(
            Bound::new(0.0, 5.0).unwrap(),
            Equality::LessThanOrEqualToZero,
            Function::from((linear!(1) + coeff!(-10.0)).unwrap()),
            Vec::new(),
        );
        // Rebuild this fixture through the current lowerer so the empty history
        // contract itself is also exercised.
        inequality = {
            let source = inequality.removed_indicator_constraints()
                [&IndicatorConstraintID::from(7)]
                .0
                .clone();
            let mut current = Instance::builder()
                .sense(Sense::Minimize)
                .objective(Function::zero())
                .decision_variables(inequality.decision_variables().clone())
                .constraints(BTreeMap::new())
                .indicator_constraints(BTreeMap::from([(IndicatorConstraintID::from(7), source)]))
                .build()
                .unwrap();
            assert!(current
                .convert_indicator_to_constraint(IndicatorConstraintID::from(7))
                .unwrap()
                .is_empty());
            current
        };
        verify_indicator_big_m_v1(&inequality, IndicatorConstraintID::from(7)).unwrap();

        // y=1 -> x=0 with x in [0,5]: upper is encoded, while -x<=0 is
        // supplied exactly by the lower bound.
        let equality = forged_indicator_semantics(
            Bound::new(0.0, 5.0).unwrap(),
            Equality::EqualToZero,
            Function::from(linear!(1)),
            vec![Constraint::less_than_or_equal_to_zero(Function::from(
                ((linear!(1) + coeff!(5.0) * linear!(2)).unwrap() + coeff!(-5.0)).unwrap(),
            ))],
        );
        verify_indicator_big_m_v1(&equality, IndicatorConstraintID::from(7)).unwrap();
    }

    #[test]
    fn rejects_inactive_indicator_row_that_is_not_redundant() {
        // Active y=1 substitution is x-2 exactly, but inactive y=0 leaves
        // x-3<=0, which is not implied by x<=5.
        let row = Constraint::less_than_or_equal_to_zero(Function::from(
            ((linear!(1) + linear!(2)).unwrap() + coeff!(-3.0)).unwrap(),
        ));
        let instance = forged_indicator_semantics(
            Bound::new(0.0, 5.0).unwrap(),
            Equality::LessThanOrEqualToZero,
            Function::from((linear!(1) + coeff!(-2.0)).unwrap()),
            vec![row],
        );
        let error =
            verify_indicator_big_m_v1(&instance, IndicatorConstraintID::from(7)).unwrap_err();
        assert!(error.to_string().contains("inactive branch"));
    }

    #[test]
    fn rejects_equality_with_an_unproved_missing_side() {
        // The one row supplies x<=0 on the active branch and is redundant at
        // y=0, but x>=0 is not implied by x in [-1,1].
        let row = Constraint::less_than_or_equal_to_zero(Function::from(
            ((linear!(1) + linear!(2)).unwrap() + coeff!(-1.0)).unwrap(),
        ));
        let instance = forged_indicator_semantics(
            Bound::new(-1.0, 1.0).unwrap(),
            Equality::EqualToZero,
            Function::from(linear!(1)),
            vec![row],
        );
        let error =
            verify_indicator_big_m_v1(&instance, IndicatorConstraintID::from(7)).unwrap_err();
        assert!(error.to_string().contains("upper and lower"));
    }

    #[test]
    fn rejects_round_to_nearest_cancellation_from_the_current_lowerer() {
        let x = DecisionVariable::new(
            Kind::Continuous,
            Bound::new(0.0, 1.0e20).unwrap(),
            crate::ATol::default(),
        )
        .unwrap();
        let mut instance = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::zero())
            .decision_variables(BTreeMap::from([
                (VariableID::from(1), x),
                (VariableID::from(2), DecisionVariable::binary()),
            ]))
            .constraints(BTreeMap::new())
            .indicator_constraints(BTreeMap::from([(
                IndicatorConstraintID::from(7),
                IndicatorConstraint::new(
                    VariableID::from(2),
                    Equality::LessThanOrEqualToZero,
                    Function::from((linear!(1) + coeff!(1.0)).unwrap()),
                ),
            )]))
            .build()
            .unwrap();
        let generated = instance
            .convert_indicator_to_constraint(IndicatorConstraintID::from(7))
            .unwrap();
        assert_eq!(generated.len(), 1);

        let error =
            verify_indicator_big_m_v1(&instance, IndicatorConstraintID::from(7)).unwrap_err();
        assert!(error.to_string().contains("active upper side exactly"));
    }

    #[test]
    fn rejects_inward_rounded_inactive_bound_from_the_current_lowerer() {
        let a = f64::from_bits(1.0f64.to_bits() + 1);
        let mut instance = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::zero())
            .decision_variables(BTreeMap::from([
                (
                    VariableID::from(1),
                    DecisionVariable::new(
                        Kind::Continuous,
                        Bound::new(0.0, a).unwrap(),
                        crate::ATol::default(),
                    )
                    .unwrap(),
                ),
                (VariableID::from(2), DecisionVariable::binary()),
            ]))
            .constraints(BTreeMap::new())
            .indicator_constraints(BTreeMap::from([(
                IndicatorConstraintID::from(7),
                IndicatorConstraint::new(
                    VariableID::from(2),
                    Equality::LessThanOrEqualToZero,
                    Function::from((coeff!(a) * linear!(1)).unwrap()),
                ),
            )]))
            .build()
            .unwrap();
        let generated = instance
            .convert_indicator_to_constraint(IndicatorConstraintID::from(7))
            .unwrap();
        assert_eq!(generated.len(), 1);

        let error =
            verify_indicator_big_m_v1(&instance, IndicatorConstraintID::from(7)).unwrap_err();
        assert!(error.to_string().contains("inactive branch"));
    }

    #[test]
    fn rejects_underflowed_skipped_side_from_the_current_lowerer() {
        let tiny = f64::from_bits(1);
        let mut instance = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::zero())
            .decision_variables(BTreeMap::from([
                (
                    VariableID::from(1),
                    DecisionVariable::new(
                        Kind::Continuous,
                        Bound::new(0.0, 0.5).unwrap(),
                        crate::ATol::default(),
                    )
                    .unwrap(),
                ),
                (VariableID::from(2), DecisionVariable::binary()),
            ]))
            .constraints(BTreeMap::new())
            .indicator_constraints(BTreeMap::from([(
                IndicatorConstraintID::from(7),
                IndicatorConstraint::new(
                    VariableID::from(2),
                    Equality::LessThanOrEqualToZero,
                    Function::from((coeff!(tiny) * linear!(1)).unwrap()),
                ),
            )]))
            .build()
            .unwrap();
        assert!(instance
            .convert_indicator_to_constraint(IndicatorConstraintID::from(7))
            .unwrap()
            .is_empty());

        let error =
            verify_indicator_big_m_v1(&instance, IndicatorConstraintID::from(7)).unwrap_err();
        assert!(error.to_string().contains("Missing Indicator upper side"));
    }

    #[test]
    fn reads_and_verifies_current_sos1_lowering() {
        let x = DecisionVariable::new(
            Kind::Continuous,
            Bound::new(-1.0, 2.0).unwrap(),
            crate::ATol::default(),
        )
        .unwrap();
        let mut labels = VariableLabelStore::default();
        labels.set_name(VariableID::from(50), "ommx.sos1_indicator");
        labels.set_subscripts(VariableID::from(50), vec![9, 1]);
        let mut instance = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::zero())
            .decision_variables(BTreeMap::from([
                (VariableID::from(1), x),
                (VariableID::from(2), DecisionVariable::binary()),
                // A pre-existing exact label collision is legal. Selector
                // reconstruction is scoped to the cardinality row, so this
                // unrelated variable must not make the history ambiguous.
                (VariableID::from(50), DecisionVariable::binary()),
            ]))
            .variable_labels(labels)
            .constraints(BTreeMap::new())
            .sos1_constraints(BTreeMap::from([(
                Sos1ConstraintID::from(9),
                Sos1Constraint::new(BTreeSet::from([VariableID::from(1), VariableID::from(2)]))
                    .unwrap(),
            )]))
            .build()
            .unwrap();
        let generated = instance
            .convert_sos1_to_constraints(Sos1ConstraintID::from(9))
            .unwrap();

        let candidate = instance
            .sos1_big_m_candidate_v1(Sos1ConstraintID::from(9))
            .unwrap();
        assert_eq!(candidate.source, Sos1ConstraintID::from(9));
        assert_eq!(candidate.generated_rows, generated);
        assert_eq!(
            candidate.representation,
            instance
                .certified_linear_relaxation()
                .unwrap()
                .fingerprint()
        );

        let verified = verify_sos1_big_m_v1(&instance, Sos1ConstraintID::from(9)).unwrap();
        assert_eq!(verified.source_id(), Sos1ConstraintID::from(9));
        assert_eq!(verified.generated_rows(), generated);
        assert_eq!(
            verified.selectors(),
            &BTreeMap::from([
                (VariableID::from(1), VariableID::from(51)),
                (VariableID::from(2), VariableID::from(2)),
            ])
        );
    }

    #[test]
    fn verifies_all_current_sos1_omitted_side_shapes() {
        let tiny = f64::from_bits(1);
        for (kind, lower, upper, expected_rows) in [
            (Kind::Integer, -2.0, 3.0, 3),
            (Kind::Integer, 0.0, 3.0, 2),
            (Kind::Integer, -2.0, 0.0, 2),
            (Kind::Integer, 0.0, 0.0, 1),
            // A restricted Binary is not the canonical [0, 1] reuse case.
            (Kind::Binary, 0.0, 0.0, 1),
            (Kind::Continuous, -f64::MAX, f64::MAX, 3),
            (Kind::Continuous, -tiny, tiny, 3),
        ] {
            let variable = DecisionVariable::new(
                kind,
                Bound::new(lower, upper).unwrap(),
                crate::ATol::default(),
            )
            .unwrap();
            let mut instance = Instance::builder()
                .sense(Sense::Minimize)
                .objective(Function::zero())
                .decision_variables(BTreeMap::from([(VariableID::from(0), variable)]))
                .constraints(BTreeMap::new())
                .sos1_constraints(BTreeMap::from([(
                    Sos1ConstraintID::from(9),
                    Sos1Constraint::new(BTreeSet::from([VariableID::from(0)])).unwrap(),
                )]))
                .build()
                .unwrap();
            let generated = instance
                .convert_sos1_to_constraints(Sos1ConstraintID::from(9))
                .unwrap();
            assert_eq!(generated.len(), expected_rows);

            let verified = verify_sos1_big_m_v1(&instance, Sos1ConstraintID::from(9)).unwrap();
            assert_eq!(
                verified.selectors()[&VariableID::from(0)],
                VariableID::from(1)
            );
        }
    }

    #[test]
    fn rejects_sos1_history_after_member_bound_change() {
        let variable = DecisionVariable::new(
            Kind::Continuous,
            Bound::new(-2.0, 3.0).unwrap(),
            crate::ATol::default(),
        )
        .unwrap();
        let mut instance = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::zero())
            .decision_variables(BTreeMap::from([(VariableID::from(0), variable)]))
            .constraints(BTreeMap::new())
            .sos1_constraints(BTreeMap::from([(
                Sos1ConstraintID::from(9),
                Sos1Constraint::new(BTreeSet::from([VariableID::from(0)])).unwrap(),
            )]))
            .build()
            .unwrap();
        instance
            .convert_sos1_to_constraints(Sos1ConstraintID::from(9))
            .unwrap();
        instance
            .clip_bounds(
                &crate::Bounds::from_iter([(VariableID::from(0), Bound::new(-2.0, 2.0).unwrap())]),
                crate::ATol::default(),
            )
            .unwrap();

        let error = verify_sos1_big_m_v1(&instance, Sos1ConstraintID::from(9)).unwrap_err();
        assert!(error.to_string().contains("canonical V1 content exactly"));
    }

    #[test]
    fn generated_id_text_is_strict_and_unique() {
        assert_eq!(
            parse_generated_constraint_ids(
                &removed_reason(INDICATOR_LOWERING_REASON, ""),
                INDICATOR_LOWERING_REASON,
            )
            .unwrap(),
            Vec::<ConstraintID>::new()
        );
        assert!(matches!(
            parse_generated_constraint_ids(
                &removed_reason(INDICATOR_LOWERING_REASON, "01"),
                INDICATOR_LOWERING_REASON,
            ),
            Err(InverseLoweringCandidateError::InvalidGeneratedConstraintId { .. })
        ));
        assert!(matches!(
            parse_generated_constraint_ids(
                &removed_reason(INDICATOR_LOWERING_REASON, "1,1"),
                INDICATOR_LOWERING_REASON,
            ),
            Err(InverseLoweringCandidateError::DuplicateGeneratedConstraintId { .. })
        ));
        assert!(matches!(
            parse_generated_constraint_ids(
                &removed_reason("other", "1"),
                INDICATOR_LOWERING_REASON,
            ),
            Err(InverseLoweringCandidateError::UnexpectedRemovalReason { .. })
        ));

        let mut extra_parameter = removed_reason(INDICATOR_LOWERING_REASON, "1");
        extra_parameter
            .parameters
            .insert("future".to_string(), "value".to_string());
        assert!(matches!(
            parse_generated_constraint_ids(&extra_parameter, INDICATOR_LOWERING_REASON),
            Err(InverseLoweringCandidateError::UnexpectedRemovalParameter { .. })
        ));

        let mut malformed_batch = removed_reason(INDICATOR_LOWERING_REASON, "1");
        malformed_batch.parameters.insert(
            CAPABILITY_REDUCTION_BATCH_TOKEN_PARAMETER.to_string(),
            "not-a-uuid".to_string(),
        );
        assert!(matches!(
            parse_generated_constraint_ids(&malformed_batch, INDICATOR_LOWERING_REASON),
            Err(InverseLoweringCandidateError::InvalidCapabilityReductionBatchToken { .. })
        ));
    }

    fn forged_indicator_history(
        generated_ids: &str,
        provenance: Vec<Provenance>,
        include_row: bool,
    ) -> Instance {
        let row_id = ConstraintID::from(10);
        let constraints = if include_row {
            BTreeMap::from([(
                row_id,
                Constraint::less_than_or_equal_to_zero(Function::from(linear!(1))),
            )])
        } else {
            BTreeMap::new()
        };
        let mut context = ConstraintContextStore::default();
        if include_row {
            context.set_provenance(row_id, provenance);
        }
        Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::zero())
            .decision_variables(BTreeMap::from([
                (VariableID::from(1), DecisionVariable::continuous()),
                (VariableID::from(2), DecisionVariable::binary()),
            ]))
            .constraints(constraints)
            .constraint_context(context)
            .removed_indicator_constraints(BTreeMap::from([(
                IndicatorConstraintID::from(7),
                (
                    IndicatorConstraint::new(
                        VariableID::from(2),
                        Equality::LessThanOrEqualToZero,
                        Function::from(linear!(1)),
                    ),
                    removed_reason(INDICATOR_LOWERING_REASON, generated_ids),
                ),
            )]))
            .build()
            .unwrap()
    }

    #[test]
    fn candidate_lookup_requires_active_rows_and_exact_lineage_hint() {
        let missing = forged_indicator_history(
            "10",
            vec![Provenance::IndicatorConstraint(
                IndicatorConstraintID::from(7),
            )],
            false,
        );
        assert!(matches!(
            missing.indicator_big_m_candidate_v1(IndicatorConstraintID::from(7)),
            Err(InverseLoweringCandidateError::MissingGeneratedConstraint { .. })
        ));

        let wrong_provenance = forged_indicator_history(
            "10",
            vec![Provenance::IndicatorConstraint(
                IndicatorConstraintID::from(8),
            )],
            true,
        );
        assert!(matches!(
            wrong_provenance.indicator_big_m_candidate_v1(IndicatorConstraintID::from(7)),
            Err(InverseLoweringCandidateError::ProvenanceMismatch { .. })
        ));

        let extra_provenance = forged_indicator_history(
            "10",
            vec![
                Provenance::IndicatorConstraint(IndicatorConstraintID::from(7)),
                Provenance::Sos1Constraint(Sos1ConstraintID::from(99)),
            ],
            true,
        );
        assert!(matches!(
            extra_provenance.indicator_big_m_candidate_v1(IndicatorConstraintID::from(7)),
            Err(InverseLoweringCandidateError::ProvenanceMismatch { .. })
        ));

        let mut labeled = forged_indicator_history(
            "10",
            vec![Provenance::IndicatorConstraint(
                IndicatorConstraintID::from(7),
            )],
            true,
        );
        labeled
            .set_constraint_context(
                ConstraintID::from(10),
                crate::ConstraintContext {
                    label: ModelingLabel {
                        name: Some("edited-generated-row".to_string()),
                        ..Default::default()
                    },
                    provenance: vec![Provenance::IndicatorConstraint(
                        IndicatorConstraintID::from(7),
                    )],
                },
            )
            .unwrap();
        assert!(matches!(
            labeled.indicator_big_m_candidate_v1(IndicatorConstraintID::from(7)),
            Err(InverseLoweringCandidateError::GeneratedConstraintHasModelingLabel { .. })
        ));

        let mut unrecorded = forged_indicator_history(
            "10",
            vec![Provenance::IndicatorConstraint(
                IndicatorConstraintID::from(7),
            )],
            true,
        );
        let unrecorded_id = unrecorded
            .add_constraint(
                Constraint::less_than_or_equal_to_zero(Function::zero()),
                crate::ConstraintContext {
                    label: Default::default(),
                    provenance: vec![Provenance::IndicatorConstraint(
                        IndicatorConstraintID::from(7),
                    )],
                },
            )
            .unwrap();
        assert!(matches!(
            unrecorded.indicator_big_m_candidate_v1(IndicatorConstraintID::from(7)),
            Err(InverseLoweringCandidateError::UnrecordedGeneratedConstraint { id })
                if id == unrecorded_id
        ));
    }

    #[test]
    fn candidate_lookup_rejects_a_generated_row_that_is_only_removed() {
        let row_id = ConstraintID::from(10);
        let mut context = ConstraintContextStore::default();
        context.set_provenance(
            row_id,
            vec![Provenance::IndicatorConstraint(
                IndicatorConstraintID::from(7),
            )],
        );
        let instance = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::zero())
            .decision_variables(BTreeMap::from([
                (VariableID::from(1), DecisionVariable::continuous()),
                (VariableID::from(2), DecisionVariable::binary()),
            ]))
            .constraints(BTreeMap::new())
            .removed_constraints(BTreeMap::from([(
                row_id,
                (
                    Constraint::less_than_or_equal_to_zero(Function::from(linear!(1))),
                    RemovedReason {
                        reason: "already removed".to_string(),
                        parameters: FnvHashMap::default(),
                    },
                ),
            )]))
            .constraint_context(context)
            .removed_indicator_constraints(BTreeMap::from([(
                IndicatorConstraintID::from(7),
                (
                    IndicatorConstraint::new(
                        VariableID::from(2),
                        Equality::LessThanOrEqualToZero,
                        Function::from(linear!(1)),
                    ),
                    removed_reason(INDICATOR_LOWERING_REASON, "10"),
                ),
            )]))
            .build()
            .unwrap();

        assert!(matches!(
            instance.indicator_big_m_candidate_v1(IndicatorConstraintID::from(7)),
            Err(InverseLoweringCandidateError::MissingGeneratedConstraint { id }) if id == row_id
        ));
    }
}
