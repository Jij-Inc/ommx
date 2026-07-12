use super::{
    ExactAffine, ExactRational, InequalityRef, LinearProofError, LinearRelaxationFingerprint,
};
use crate::{
    constraint::{Provenance, RemovedReason},
    instance::{
        GENERATED_CONSTRAINT_IDS_PARAMETER, INDICATOR_LOWERING_REASON, SOS1_LOWERING_REASON,
    },
    ConstraintID, Equality, IndicatorConstraint, IndicatorConstraintID, Instance, Kind,
    Sos1ConstraintID,
};
use std::collections::BTreeSet;

/// Untrusted, versioned lookup result for one historical Indicator Big-M
/// lowering. The generated rows have not yet been proven equivalent to the
/// removed source.
#[derive(Debug, Clone, PartialEq, Eq)]
struct IndicatorBigMV1 {
    source: IndicatorConstraintID,
    generated_rows: Vec<ConstraintID>,
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

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
enum InverseLoweringCandidateError {
    #[error("removed Indicator constraint {id:?} was not found")]
    MissingRemovedIndicator { id: IndicatorConstraintID },
    #[error("removed SOS1 constraint {id:?} was not found")]
    MissingRemovedSos1 { id: Sos1ConstraintID },
    #[error("expected lowering reason {expected}, found {actual}")]
    UnexpectedRemovalReason {
        expected: &'static str,
        actual: String,
    },
    #[error("lowering reason is missing the {GENERATED_CONSTRAINT_IDS_PARAMETER} parameter")]
    MissingGeneratedConstraintIds,
    #[error("lowering reason contains unsupported V1 parameter {key:?}")]
    UnexpectedRemovalParameter { key: String },
    #[error("generated constraint ID token {token:?} is not canonical u64 text")]
    InvalidGeneratedConstraintId { token: String },
    #[error("generated constraint ID {id:?} is listed more than once")]
    DuplicateGeneratedConstraintId { id: ConstraintID },
    #[error("generated regular constraint {id:?} is not active")]
    MissingGeneratedConstraint { id: ConstraintID },
    #[error("generated regular constraint {id:?} has unexpected provenance")]
    ProvenanceMismatch { id: ConstraintID },
    #[error("generated regular constraint {id:?} has a modeling label that would be discarded")]
    GeneratedConstraintHasModelingLabel { id: ConstraintID },
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
        Ok(())
    }
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
        crate::bail!(
            { ?id },
            "Indicator inverse-lowering candidate {id:?} is bound to a stale representation"
        );
    }

    let (source, _) = instance
        .removed_indicator_constraints()
        .get(&id)
        .expect("candidate lookup proved that the removed source exists");
    let indicator_variable = source.indicator_variable;
    let Some(indicator) = instance.decision_variables().get(&indicator_variable) else {
        crate::bail!(
            { ?id, ?indicator_variable },
            "Indicator variable {indicator_variable:?} is not registered"
        );
    };
    if indicator.kind() != Kind::Binary {
        crate::bail!(
            { ?id, ?indicator_variable },
            "Indicator variable {indicator_variable:?} is not binary"
        );
    }

    let body = ExactAffine::from_function(source.function())?.ok_or_else(|| {
        crate::error!(
            { ?id },
            "Removed Indicator constraint {id:?} has a nonlinear body"
        )
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
            crate::bail!(
                { ?id, ?row_id },
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
                crate::bail!(
                    { ?id, generated_rows = roles.len() },
                    "Inequality Indicator {id:?} has more than one generated row"
                );
            }
            if let Some((matches_upper, _)) = roles.first() {
                if !matches_upper {
                    crate::bail!(
                        { ?id, row_id = ?candidate.generated_rows[0] },
                        "Generated Indicator row does not equal the active upper side exactly"
                    );
                }
            } else if !upper_implied {
                crate::bail!(
                    { ?id },
                    "Missing Indicator upper side is not implied by exact variable facts"
                );
            }
        }
        Equality::EqualToZero => {
            if roles.len() > 2 {
                crate::bail!(
                    { ?id, generated_rows = roles.len() },
                    "Equality Indicator {id:?} has more than two generated rows"
                );
            }
            let lower_implied = snapshot.variable_facts_imply_nonpositive(&active_lower)?;
            if !equality_roles_cover(&roles, upper_implied, lower_implied) {
                crate::bail!(
                    { ?id },
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
        .filter(|key| key.as_str() != GENERATED_CONSTRAINT_IDS_PARAMETER)
        .min()
    {
        return Err(InverseLoweringCandidateError::UnexpectedRemovalParameter { key: key.clone() });
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        coeff, linear, Bound, Constraint, ConstraintContextStore, DecisionVariable, Equality,
        Function, IndicatorConstraint, Kind, ModelingLabel, Sense, Sos1Constraint, VariableID,
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
    fn reads_current_sos1_lowering_without_inferring_selector_roles() {
        let x = DecisionVariable::new(
            Kind::Continuous,
            Bound::new(-1.0, 2.0).unwrap(),
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
    }

    fn forged_indicator_history(
        generated_ids: &str,
        provenance: Vec<Provenance>,
        include_row: bool,
    ) -> Instance {
        let row_id = ConstraintID::from(10);
        let constraints = include_row
            .then(|| {
                BTreeMap::from([(
                    row_id,
                    Constraint::less_than_or_equal_to_zero(Function::from(linear!(1))),
                )])
            })
            .unwrap_or_default();
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
