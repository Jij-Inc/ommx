use super::{LinearProofError, LinearRelaxationFingerprint};
use crate::{
    constraint::{Provenance, RemovedReason},
    ConstraintID, IndicatorConstraintID, Instance, Sos1ConstraintID,
};
use std::collections::BTreeSet;

const GENERATED_CONSTRAINT_IDS: &str = "constraint_ids";
const INDICATOR_LOWERING_REASON: &str = "ommx.Instance.convert_indicator_to_constraint";
const SOS1_LOWERING_REASON: &str = "ommx.Instance.convert_sos1_to_constraints";

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
    #[error("lowering reason is missing the {GENERATED_CONSTRAINT_IDS} parameter")]
    MissingGeneratedConstraintIds,
    #[error("generated constraint ID token {token:?} is not canonical u64 text")]
    InvalidGeneratedConstraintId { token: String },
    #[error("generated constraint ID {id:?} is listed more than once")]
    DuplicateGeneratedConstraintId { id: ConstraintID },
    #[error("generated regular constraint {id:?} is not active")]
    MissingGeneratedConstraint { id: ConstraintID },
    #[error("generated regular constraint {id:?} has unexpected provenance")]
    ProvenanceMismatch { id: ConstraintID },
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
        }
        Ok(())
    }
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
    let raw = reason
        .parameters
        .get(GENERATED_CONSTRAINT_IDS)
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
        Function, IndicatorConstraint, Kind, Sense, Sos1Constraint, VariableID,
    };
    use fnv::FnvHashMap;
    use std::collections::{BTreeMap, BTreeSet};

    fn removed_reason(reason: &str, ids: &str) -> RemovedReason {
        RemovedReason {
            reason: reason.to_string(),
            parameters: FnvHashMap::from_iter([(
                GENERATED_CONSTRAINT_IDS.to_string(),
                ids.to_string(),
            )]),
        }
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
