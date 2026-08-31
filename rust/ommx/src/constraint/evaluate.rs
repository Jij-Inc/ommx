use super::*;
use crate::{Evaluate, VariableIDSet};

impl Constraint<Created> {
    /// Prepare an intrinsic row replacement for the `Instance`-owned atomic
    /// partial-evaluation plan while preserving this constraint's equality.
    pub(crate) fn partial_evaluate_replacement(
        &self,
        state: &crate::v1::State,
        atol: crate::ATol,
    ) -> crate::Result<Option<Self>> {
        self.stage
            .function
            .partial_evaluate_replacement(state, atol)
            .map(|replacement| {
                replacement.map(|function| Self {
                    equality: self.equality,
                    stage: CreatedData { function },
                })
            })
    }
}

impl Evaluate for Constraint<Created> {
    type Output = EvaluatedConstraint;
    type SampledOutput = SampledConstraint;

    fn evaluate(
        &self,
        solution: &crate::v1::State,
        atol: crate::ATol,
    ) -> crate::Result<Self::Output> {
        let evaluated_value = self.stage.function.evaluate(solution, atol)?;
        let used_decision_variable_ids = self.stage.function.required_ids();

        let feasible = self.equality.is_satisfied(evaluated_value, atol);

        Ok(EvaluatedConstraint {
            equality: self.equality,
            stage: EvaluatedData {
                evaluated_value,
                dual_variable: None,
                feasible,
                used_decision_variable_ids,
            },
        })
    }

    fn evaluate_samples(
        &self,
        samples: &crate::Sampled<crate::v1::State>,
        atol: crate::ATol,
    ) -> crate::Result<Self::SampledOutput> {
        let evaluated_values = self.stage.function.evaluate_samples(samples, atol)?;

        let feasible: std::collections::BTreeMap<crate::SampleID, bool> = evaluated_values
            .iter()
            .map(|(sample_id, evaluated_value)| {
                (
                    *sample_id,
                    self.equality.is_satisfied(*evaluated_value, atol),
                )
            })
            .collect();

        Ok(SampledConstraint {
            equality: self.equality,
            stage: SampledData {
                evaluated_values,
                dual_variables: None,
                feasible,
                used_decision_variable_ids: self.stage.function.required_ids(),
            },
        })
    }

    fn partial_evaluate(
        &mut self,
        state: &crate::v1::State,
        atol: crate::ATol,
    ) -> crate::Result<()> {
        self.stage.function.partial_evaluate(state, atol)
    }

    fn required_ids(&self) -> VariableIDSet {
        self.stage.function.required_ids()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{constraint_type::SampledConstraintBehavior, random::*, Sampled};
    use proptest::prelude::*;

    #[test]
    fn scalar_and_sample_feasibility_include_the_atol_boundary() {
        let atol = crate::ATol::new(0.125).unwrap();
        let outside = f64::from_bits(0.125_f64.to_bits() + 1);
        let sample_id = crate::SampleID::from(0);
        let samples = Sampled::from((sample_id, crate::v1::State::default()));

        assert!(Equality::LessThanOrEqualToZero.is_satisfied(-10.0, atol));

        for equality in [Equality::EqualToZero, Equality::LessThanOrEqualToZero] {
            let constraint = Constraint {
                equality,
                stage: CreatedData {
                    function: Function::Constant(crate::Coefficient::try_from(*atol).unwrap()),
                },
            };
            let evaluated = constraint
                .evaluate(&crate::v1::State::default(), atol)
                .unwrap();
            let sampled = constraint.evaluate_samples(&samples, atol).unwrap();

            assert!(evaluated.stage.feasible);
            assert_eq!(sampled.is_feasible(sample_id, atol), Some(true));
            assert!(sampled.feasible_ids(atol).contains(&sample_id));

            let outside_constraint = Constraint {
                equality,
                stage: CreatedData {
                    function: Function::Constant(crate::Coefficient::try_from(outside).unwrap()),
                },
            };
            let evaluated = outside_constraint
                .evaluate(&crate::v1::State::default(), atol)
                .unwrap();
            let sampled = outside_constraint.evaluate_samples(&samples, atol).unwrap();

            assert!(!evaluated.stage.feasible);
            assert_eq!(sampled.is_feasible(sample_id, atol), Some(false));
            assert!(sampled.infeasible_ids(atol).contains(&sample_id));
        }
    }

    fn constraint_and_samples(
    ) -> impl Strategy<Value = (Constraint<Created>, Sampled<crate::v1::State>)> {
        Constraint::arbitrary()
            .prop_flat_map(|c| {
                let ids = c.stage.function.required_ids();
                let state = arbitrary_state(ids);
                let samples = arbitrary_samples(SamplesParameters::default(), state);
                (Just(c), samples)
            })
            .boxed()
    }

    proptest! {
        #[test]
        fn test_evaluate_samples((c, samples) in constraint_and_samples()) {
            let atol = crate::ATol::default();
            match c.evaluate_samples(&samples, atol) {
                Ok(evaluated) => {
                    for (sample_id, state) in samples.iter() {
                        let expected = c.evaluate(state, atol).unwrap();
                        let extracted = evaluated.get(*sample_id).unwrap();
                        prop_assert_eq!(extracted, expected);
                    }
                }
                Err(_) => {
                    prop_assert!(
                        samples
                            .iter()
                            .any(|(_, state)| c.evaluate(state, atol).is_err()),
                        "sample evaluation failed although every scalar evaluation succeeded",
                    );
                }
            }
        }
    }
}
