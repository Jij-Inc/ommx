use crate::{
    ATol, OneHotConstraintID, SampleID, SampleSet, Solution, Sos1ConstraintID, VariableID,
    VariableIDSet,
};
use std::collections::BTreeMap;

/// Why a OneHot candidate fails at the result's retained evaluation tolerance.
///
/// Returned on demand by [`Solution::one_hot_violation_details`] and
/// [`SampleSet::one_hot_violation_details`]. Every variant contains all
/// finite member values, including exact zeros. Feasible candidates return
/// `None`, including when tolerance neighborhoods overlap.
#[derive(Debug, Clone, PartialEq)]
pub enum OneHotViolation {
    /// Every member is within tolerance of zero, but the row is infeasible.
    /// No particular member is designated as the missing selection.
    MissingSelection { values: BTreeMap<VariableID, f64> },
    /// At least two members are within tolerance of one and the row is infeasible.
    /// `selected_variables` contains all such members; `values` also retains any
    /// other member errors, which can coexist with multiple selections.
    MultipleSelections {
        selected_variables: VariableIDSet,
        values: BTreeMap<VariableID, f64>,
    },
    /// Other deviations from a OneHot assignment.
    ///
    /// `target_variable` is one closest assignment's member to set to one:
    /// the largest member value, with ties resolved by the largest variable ID.
    /// It is a repair candidate, not a uniquely intended selection.
    /// `errors` contains absolute changes exceeding tolerance: distance to one
    /// for the target and distance to zero for every other member.
    ValueErrors {
        target_variable: VariableID,
        errors: BTreeMap<VariableID, f64>,
        values: BTreeMap<VariableID, f64>,
    },
}

fn diagnose(
    variables: &VariableIDSet,
    mut value_of: impl FnMut(VariableID) -> f64,
    violation: f64,
    atol: ATol,
) -> Option<OneHotViolation> {
    if crate::constraint_type::violation_is_feasible(violation, atol) {
        return None;
    }
    let values: BTreeMap<_, _> = variables.iter().map(|&id| (id, value_of(id))).collect();
    if values.values().all(|&value| atol.approx_is_zero(value)) {
        return Some(OneHotViolation::MissingSelection { values });
    }
    let selected_variables: VariableIDSet = values
        .iter()
        .filter(|(_, value)| atol.approx_eq(**value, 1.0))
        .map(|(&id, _)| id)
        .collect();
    if selected_variables.len() > 1 {
        return Some(OneHotViolation::MultipleSelections {
            selected_variables,
            values,
        });
    }
    let (&target_variable, _) = values
        .iter()
        .max_by(|(_, a), (_, b)| a.total_cmp(b))
        .expect("OneHot constraints have at least one member");
    let errors = values
        .iter()
        .filter_map(|(&id, &value)| {
            let error = (value - if id == target_variable { 1.0 } else { 0.0 }).abs();
            (!atol.approx_is_zero(error)).then_some((id, error))
        })
        .collect();
    Some(OneHotViolation::ValueErrors {
        target_variable,
        errors,
        values,
    })
}

fn sos1_values(
    variables: &VariableIDSet,
    mut value_of: impl FnMut(VariableID) -> f64,
    atol: ATol,
) -> BTreeMap<VariableID, f64> {
    let retained = variables
        .iter()
        .map(|&id| (id, value_of(id)))
        .max_by(|(_, a), (_, b)| a.abs().total_cmp(&b.abs()))
        .map(|(id, _)| id);
    variables
        .iter()
        .filter_map(|&id| {
            let value = value_of(id);
            (Some(id) != retained && !atol.approx_is_zero(value)).then_some((id, value))
        })
        .collect()
}

impl Solution {
    /// Diagnose a OneHot constraint using the stored member values, on demand.
    ///
    /// Includes inactive constraints regardless of feasibility participation.
    /// Returns `None` exactly when this row is feasible at [`Self::feasibility_atol`].
    /// Other feasibility queries do not change this tolerance. Details are newly
    /// computed on each call and are neither cached nor stored in constraint rows.
    ///
    /// # Errors
    /// Returns an error if `constraint_id` is not a retained OneHot constraint.
    pub fn one_hot_violation_details(
        &self,
        constraint_id: OneHotConstraintID,
    ) -> crate::Result<Option<OneHotViolation>> {
        let row = self
            .evaluated_one_hot_constraints()
            .get(&constraint_id)
            .ok_or_else(|| crate::error!("Unknown OneHot constraint ID: {constraint_id}"))?;
        Ok(diagnose(
            &row.variables,
            |id| *self.decision_variables()[&id].value(),
            row.violation(),
            self.feasibility_atol(),
        ))
    }

    /// Diagnose an SOS1 constraint using the stored member values, on demand.
    ///
    /// Retains one member of largest absolute value (ties retain the largest ID)
    /// and returns the other values whose absolute value exceeds
    /// [`Self::feasibility_atol`]. The retained member is one possible repair
    /// choice, not a uniquely intended selection. The map is empty exactly when
    /// this row is feasible at that tolerance. Includes inactive constraints
    /// regardless of feasibility participation. Results are not cached.
    ///
    /// # Errors
    /// Returns an error if `constraint_id` is not a retained SOS1 constraint.
    pub fn sos1_violating_values(
        &self,
        constraint_id: Sos1ConstraintID,
    ) -> crate::Result<BTreeMap<VariableID, f64>> {
        let row = self
            .evaluated_sos1_constraints()
            .get(&constraint_id)
            .ok_or_else(|| crate::error!("Unknown SOS1 constraint ID: {constraint_id}"))?;
        if crate::constraint_type::violation_is_feasible(row.violation(), self.feasibility_atol()) {
            return Ok(BTreeMap::new());
        }
        Ok(sos1_values(
            &row.variables,
            |id| *self.decision_variables()[&id].value(),
            self.feasibility_atol(),
        ))
    }
}

impl SampleSet {
    /// Diagnose one OneHot constraint in one sample, without extracting a Solution.
    ///
    /// Uses only the requested constraint's member values from this sample.
    /// Includes inactive constraints. Like [`Solution::one_hot_violation_details`],
    /// returns `None` exactly when the row is feasible at [`Self::feasibility_atol`].
    /// Details are computed per call and are not cached.
    ///
    /// # Errors
    /// Returns an error for an unknown OneHot constraint or sample ID. Unknown
    /// samples produce [`crate::SampleSetError::UnknownSampleID`].
    pub fn one_hot_violation_details(
        &self,
        constraint_id: OneHotConstraintID,
        sample_id: SampleID,
    ) -> crate::Result<Option<OneHotViolation>> {
        let row = self
            .one_hot_constraints()
            .get(&constraint_id)
            .ok_or_else(|| crate::error!("Unknown OneHot constraint ID: {constraint_id}"))?;
        let violation = *row
            .stage
            .violations
            .get(sample_id)
            .ok_or(crate::SampleSetError::UnknownSampleID { id: sample_id })?;
        Ok(diagnose(
            &row.variables,
            |id| {
                *self.decision_variables()[&id]
                    .samples()
                    .get(sample_id)
                    .expect("validated sample IDs")
            },
            violation,
            self.feasibility_atol(),
        ))
    }

    /// Diagnose one SOS1 constraint in one sample, without extracting a Solution.
    ///
    /// See [`Solution::sos1_violating_values`] for the repair choice and tolerance
    /// contract. Uses only this constraint's members in the requested sample,
    /// includes inactive constraints, and does not cache the returned map.
    ///
    /// # Errors
    /// Returns an error for an unknown SOS1 constraint or sample ID. Unknown
    /// samples produce [`crate::SampleSetError::UnknownSampleID`].
    pub fn sos1_violating_values(
        &self,
        constraint_id: Sos1ConstraintID,
        sample_id: SampleID,
    ) -> crate::Result<BTreeMap<VariableID, f64>> {
        let row = self
            .sos1_constraints()
            .get(&constraint_id)
            .ok_or_else(|| crate::error!("Unknown SOS1 constraint ID: {constraint_id}"))?;
        let violation = *row
            .stage
            .violations
            .get(sample_id)
            .ok_or(crate::SampleSetError::UnknownSampleID { id: sample_id })?;
        if crate::constraint_type::violation_is_feasible(violation, self.feasibility_atol()) {
            return Ok(BTreeMap::new());
        }
        Ok(sos1_values(
            &row.variables,
            |id| {
                *self.decision_variables()[&id]
                    .samples()
                    .get(sample_id)
                    .expect("validated sample IDs")
            },
            self.feasibility_atol(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v1::State;
    use crate::{Evaluate, EvaluatedConstraintBehavior};
    use proptest::prelude::*;

    fn instance(n: usize) -> crate::Instance {
        let ids: VariableIDSet = (0..n as u64).map(Into::into).collect();
        crate::Instance::builder()
            .sense(crate::Sense::Minimize)
            .objective(crate::Function::Zero)
            .active_constraints(BTreeMap::new())
            .decision_variables(
                ids.iter()
                    .map(|&id| (id, crate::DecisionVariable::binary()))
                    .collect(),
            )
            .active_one_hot_constraints(
                [(0.into(), crate::OneHotConstraint::new(ids.clone()).unwrap())].into(),
            )
            .active_sos1_constraints([(1.into(), crate::Sos1Constraint::new(ids).unwrap())].into())
            .build()
            .unwrap()
    }

    fn evaluate(values: [f64; 3], atol: f64) -> Solution {
        instance(3)
            .evaluate(
                &crate::v1::State::from_iter(
                    values
                        .into_iter()
                        .enumerate()
                        .map(|(id, value)| (id as u64, value)),
                ),
                ATol::new(atol).unwrap(),
            )
            .unwrap()
    }

    #[test]
    fn missing_selection_uses_canonical_stored_values() {
        for values in [[0.0, 0.0, -0.0], [0.0625, -0.0625, 0.0]] {
            let result = evaluate(values, 0.125);
            let Some(OneHotViolation::MissingSelection { values: retained }) =
                result.one_hot_violation_details(0.into()).unwrap()
            else {
                panic!("expected a missing selection");
            };
            // Instance evaluation canonicalizes binary values within tolerance.
            assert_eq!(
                retained,
                [(0.into(), 0.0), (1.into(), 0.0), (2.into(), 0.0)].into()
            );
            for (&id, &value) in &retained {
                assert_eq!(value, *result.decision_variables()[&id].value());
            }
        }
    }

    #[test]
    fn multiple_selections_preserve_other_value_errors() {
        let result = evaluate([1.0, 0.9375, -0.5], 0.125);
        assert_eq!(result.one_hot_constraint_violation(0.into()).unwrap(), 1.0);
        assert_eq!(
            result.one_hot_violation_details(0.into()).unwrap(),
            Some(OneHotViolation::MultipleSelections {
                selected_variables: [0.into(), 1.into()].into(),
                values: [(0.into(), 1.0), (1.into(), 1.0), (2.into(), -0.5)].into(),
            })
        );
    }

    #[test]
    fn value_errors_report_absolute_changes_and_a_deterministic_target() {
        let result = evaluate([0.875, 0.25, 0.0], 0.0625);
        assert_eq!(result.one_hot_constraint_violation(0.into()).unwrap(), 0.25);
        assert_eq!(
            result.one_hot_violation_details(0.into()).unwrap(),
            Some(OneHotViolation::ValueErrors {
                target_variable: 0.into(),
                errors: [(0.into(), 0.125), (1.into(), 0.25)].into(),
                values: [(0.into(), 0.875), (1.into(), 0.25), (2.into(), 0.0)].into(),
            })
        );
        let Some(OneHotViolation::ValueErrors {
            target_variable,
            errors,
            ..
        }) = evaluate([0.5, 0.5, 0.0], 0.125)
            .one_hot_violation_details(0.into())
            .unwrap()
        else {
            panic!()
        };
        assert_eq!(target_variable, 1.into());
        assert_eq!(errors, [(0.into(), 0.5), (1.into(), 0.5)].into());
        // Select by value, not absolute value; negative outliers remain errors.
        let Some(OneHotViolation::ValueErrors {
            target_variable,
            errors,
            ..
        }) = evaluate([-3.0, 0.875, 0.0], 0.0625)
            .one_hot_violation_details(0.into())
            .unwrap()
        else {
            panic!()
        };
        assert_eq!(target_variable, 1.into());
        assert_eq!(errors, [(0.into(), 3.0), (1.into(), 0.125)].into());
    }

    #[test]
    fn inclusive_tolerance_and_overlapping_neighborhoods_follow_feasibility() {
        assert_eq!(
            evaluate([0.875, 0.125, 0.0], 0.125)
                .one_hot_violation_details(0.into())
                .unwrap(),
            None
        );
        assert_eq!(
            evaluate([0.0, 0.0, 0.0], 1.0)
                .one_hot_violation_details(0.into())
                .unwrap(),
            None
        );
        assert_eq!(
            evaluate([1.0, 1.0, 0.0], 1.0)
                .one_hot_violation_details(0.into())
                .unwrap(),
            None
        );
        let result = evaluate([0.875, 0.25, 0.0], 0.125);
        let before = result.one_hot_violation_details(0.into()).unwrap();
        assert!(
            result.evaluated_one_hot_constraints()[&0.into()].is_feasible(ATol::new(0.25).unwrap())
        );
        assert_eq!(result.one_hot_violation_details(0.into()).unwrap(), before);
        let Some(OneHotViolation::ValueErrors { errors, .. }) = before else {
            panic!()
        };
        assert_eq!(errors, [(1.into(), 0.25)].into());
    }
    proptest::proptest! {
        #[test]
        fn diagnostics_match_feasibility_at_any_retained_tolerance(
            values in prop::collection::vec(-16_i16..=16, 1..8),
            tolerance in 1_u16..=32,
        ) {
            let atol = ATol::new(f64::from(tolerance) / 8.0).unwrap();
            let host = instance(values.len());
            let state = State::from_iter(values.into_iter().enumerate().map(|(id, value)| (id as u64, f64::from(value) / 8.0)));
            let solution = host.evaluate(&state, atol).unwrap();
            let one_hot = &solution.evaluated_one_hot_constraints()[&0.into()];
            let sos1 = &solution.evaluated_sos1_constraints()[&1.into()];
            let one_hot_details = solution.one_hot_violation_details(0.into()).unwrap();
            let sos1_details = solution.sos1_violating_values(1.into()).unwrap();
            let samples = host.evaluate_samples(&crate::Sampled::new([vec![2.into(), 7.into()]], [state.clone()]).unwrap(), atol).unwrap();
            let restored = SampleSet::from_v2_bytes(&samples.to_v2_bytes()).unwrap();
            for samples in [&samples, &restored] {
                for sid in [2.into(), 7.into()] {
                    prop_assert_eq!(samples.one_hot_violation_details(0.into(), sid).unwrap(), one_hot_details.clone());
                    prop_assert_eq!(samples.sos1_violating_values(1.into(), sid).unwrap(), sos1_details.clone());
                    let extracted = samples.get(sid).unwrap();
                    prop_assert_eq!(extracted.one_hot_violation_details(0.into()).unwrap(), one_hot_details.clone());
                    prop_assert_eq!(extracted.sos1_violating_values(1.into()).unwrap(), sos1_details.clone());
                }
            }
            let restored = Solution::from_v2_bytes(&solution.to_v2_bytes()).unwrap();
            prop_assert_eq!(restored.one_hot_violation_details(0.into()).unwrap(), one_hot_details.clone());
            prop_assert_eq!(restored.sos1_violating_values(1.into()).unwrap(), sos1_details.clone());
            prop_assert_eq!(one_hot_details.is_none(), one_hot.is_feasible(atol));
            prop_assert_eq!(sos1_details.is_empty(), sos1.is_feasible(atol));
            for (id, value) in sos1_details {
                prop_assert_eq!(*solution.decision_variables()[&id].value(), value);
                prop_assert!(value.abs() > *atol);
            }
            if let Some(crate::OneHotViolation::ValueErrors { errors, target_variable, values }) = one_hot_details {
                prop_assert!(!errors.is_empty());
                prop_assert_eq!(errors.values().copied().fold(0.0_f64, f64::max), one_hot.violation());
                for (id, error) in errors {
                    prop_assert!(error > *atol);
                    prop_assert_eq!(error, (values[&id] - if id == target_variable { 1.0 } else { 0.0 }).abs());
                }
            }
        }
    }
}
