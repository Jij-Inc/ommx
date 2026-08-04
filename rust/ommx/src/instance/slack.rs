use super::Instance;
use crate::{
    constraint::{Equality, RemovedReason},
    ATol, Bound, Bounds, Coefficient, Constraint, ConstraintID, DecisionVariable,
    DecisionVariableError, Evaluate, InfeasibleDetected, Kind, Linear, LinearMonomial,
    ModelingLabel, VariableID,
};
use anyhow::{bail, Context, Result};
use num::traits::Inv;
use std::collections::BTreeMap;

const MAX_APPROXIMATE_INTEGER_SLACK_RANGE: u64 = 1 << 53;

enum IntegerSlackRowPlan {
    Relax {
        constraint: Constraint,
        reason: &'static str,
    },
    Add {
        constraint: Constraint,
        slack_bound: Bound,
        slack_coefficient: Option<Coefficient>,
        equality: Equality,
        residual_coefficient: Option<f64>,
    },
}

impl IntegerSlackRowPlan {
    fn residual_coefficient(&self) -> Option<f64> {
        match self {
            Self::Relax { .. } => None,
            Self::Add {
                residual_coefficient,
                ..
            } => *residual_coefficient,
        }
    }
}

struct PlannedSlackDecisionVariable {
    id: VariableID,
    row: DecisionVariable,
    label: ModelingLabel,
    atol: ATol,
}

struct IntegerSlackMutationPlan {
    decision_variables: Vec<PlannedSlackDecisionVariable>,
    replacements: BTreeMap<ConstraintID, Constraint>,
    removals: BTreeMap<ConstraintID, (Constraint, RemovedReason)>,
    residual_coefficients: Vec<Option<f64>>,
}

/// Signal returned when exact integer-slack conversion is structurally valid
/// but cannot construct an exact finite encoding.
///
/// Callers may recover by selecting an explicitly approximate transformation.
/// Missing constraints, unsupported variable kinds, and other contract or
/// materialization failures are not classified as this signal.
#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub struct ExactIntegerSlackUnavailable(ExactIntegerSlackFailure);

#[derive(Debug, thiserror::Error)]
enum ExactIntegerSlackFailure {
    #[error("Cannot normalize the coefficients to integers: constraint={id:?}")]
    CoefficientsNotNormalizable { id: ConstraintID },

    #[error(
        "The range of the slack variable exceeds the limit: evaluated({evaluated}) > limit({limit})"
    )]
    RangeTooLarge {
        id: ConstraintID,
        evaluated: f64,
        limit: u64,
    },
}

impl ExactIntegerSlackUnavailable {
    fn coefficients_not_normalizable(id: ConstraintID) -> Self {
        Self(ExactIntegerSlackFailure::CoefficientsNotNormalizable { id })
    }

    fn range_too_large(id: ConstraintID, evaluated: f64, limit: u64) -> Self {
        Self(ExactIntegerSlackFailure::RangeTooLarge {
            id,
            evaluated,
            limit,
        })
    }
}

impl Instance {
    /// Convert an inequality $f(x) \leq 0$ to an equality $a f(x) + s = 0$ with a
    /// newly introduced integer slack variable $s$, where $a$ is the minimal positive
    /// factor that makes every coefficient of $f(x)$ integer.
    ///
    /// # Errors
    ///
    /// Returns [`ExactIntegerSlackUnavailable`] when exact coefficient
    /// normalization or the configured finite slack range is unavailable, and
    /// [`InfeasibleDetected`] when variable bounds prove the inequality
    /// infeasible. Missing constraints, unsupported variable kinds, allocation,
    /// and coefficient-arithmetic failures retain their ordinary error types.
    pub fn convert_inequality_to_equality_with_integer_slack(
        &mut self,
        constraint_id: u64,
        max_integer_range: u64,
        atol: ATol,
    ) -> Result<()> {
        let constraint_id = ConstraintID::from(constraint_id);
        let bounds = self.bounds();
        let kinds = self.kinds();
        let plan =
            self.plan_exact_integer_slack(constraint_id, max_integer_range, atol, &bounds, &kinds)?;
        let mutation = self.plan_integer_slack_mutation([(constraint_id, plan)], atol)?;
        self.commit_integer_slack_mutation(mutation);
        Ok(())
    }

    fn plan_exact_integer_slack(
        &self,
        constraint_id: ConstraintID,
        max_integer_range: u64,
        atol: ATol,
        bounds: &Bounds,
        kinds: &fnv::FnvHashMap<VariableID, Kind>,
    ) -> Result<IntegerSlackRowPlan> {
        let constraint = self.integer_inequality(constraint_id, kinds)?;

        let a = constraint.function().content_factor().map_err(|source| {
            source.context(ExactIntegerSlackUnavailable::coefficients_not_normalizable(
                constraint_id,
            ))
        })?;
        let af = (constraint.function().clone() * a)?;

        let af_bound = af.evaluate_bound(bounds);
        let af_bound = af_bound.as_integer_bound(atol).ok_or(
            InfeasibleDetected::InequalityConstraintBound {
                id: constraint_id,
                bound: af_bound,
            },
        )?;
        if af_bound.lower() > 0.0 {
            bail!(InfeasibleDetected::InequalityConstraintBound {
                id: constraint_id,
                bound: af_bound,
            });
        }
        if af_bound.upper() <= 0.0 {
            return Ok(IntegerSlackRowPlan::Relax {
                constraint,
                reason: "ommx.Instance.convert_inequality_to_equality_with_integer_slack",
            });
        }

        let slack_bound = Bound::new(0.0, -af_bound.lower()).unwrap();
        if slack_bound.width() > max_integer_range as f64 {
            return Err(ExactIntegerSlackUnavailable::range_too_large(
                constraint_id,
                slack_bound.width(),
                max_integer_range,
            )
            .into());
        }

        Ok(IntegerSlackRowPlan::Add {
            constraint,
            slack_bound,
            slack_coefficient: Some(a.inv()?),
            equality: Equality::EqualToZero,
            residual_coefficient: None,
        })
    }

    /// Convert an inequality $f(x) \leq 0$ to $f(x) + b s \leq 0$ with an integer
    /// slack variable $s \in [0, \text{slack\_upper\_bound}]$.
    ///
    /// For $A = -\mathrm{lower}(f(x)) > 0$ and
    /// $R = \text{slack\_upper\_bound}$, the returned coefficient is the
    /// round-to-nearest value of $A / R$, advanced to the next representable
    /// `f64` only when that rounded value is below $A / R$. Thus $bR \geq A$,
    /// and the discretization residual is at most $b$. The caller-provided
    /// `max_error` is an inclusive upper bound on $b$ and must be positive and
    /// finite. A true zero value of $A$ needs no slack coefficient.
    /// `slack_upper_bound` must be in `1..=2^53` so its conversion to `f64` is
    /// exact.
    ///
    /// Returns `None` if the constraint was trivially satisfied and was moved
    /// to `removed_constraints`.
    ///
    /// # Errors
    ///
    /// Returns an ordinary error if `max_error` is not positive and finite or
    /// if the planned residual coefficient exceeds it. Contract, allocation,
    /// and arithmetic failures from constructing the slack variable are also
    /// propagated as ordinary errors. Every failure leaves the instance
    /// unchanged.
    pub fn add_integer_slack_to_inequality(
        &mut self,
        constraint_id: u64,
        slack_upper_bound: u64,
        max_error: f64,
    ) -> Result<Option<f64>> {
        anyhow::ensure!(
            max_error.is_finite() && max_error > 0.0,
            "maximum Integer slack approximation error must be positive and finite: {max_error}"
        );
        let constraint_id = ConstraintID::from(constraint_id);
        let bounds = self.bounds();
        let kinds = self.kinds();
        let plan =
            self.plan_approximate_integer_slack(constraint_id, slack_upper_bound, &bounds, &kinds)?;
        if let Some(residual_coefficient) = plan
            .residual_coefficient()
            .filter(|&coefficient| coefficient > max_error)
        {
            crate::bail!(
                "approximate Integer slack residual coefficient {residual_coefficient} exceeds maximum error {max_error} for constraint {constraint_id:?}"
            );
        }
        let mutation =
            self.plan_integer_slack_mutation([(constraint_id, plan)], ATol::default())?;
        Ok(self
            .commit_integer_slack_mutation(mutation)
            .into_iter()
            .next()
            .expect("one planned constraint produces one residual result"))
    }

    fn plan_approximate_integer_slack(
        &self,
        constraint_id: ConstraintID,
        slack_upper_bound: u64,
        bounds: &Bounds,
        kinds: &fnv::FnvHashMap<VariableID, Kind>,
    ) -> Result<IntegerSlackRowPlan> {
        anyhow::ensure!(
            (1..=MAX_APPROXIMATE_INTEGER_SLACK_RANGE).contains(&slack_upper_bound),
            "Integer slack upper bound must be in 1..=2^53: {slack_upper_bound}"
        );

        let constraint = self.integer_inequality(constraint_id, kinds)?;

        let f_bound = constraint.function().evaluate_bound(bounds);
        if f_bound.lower() > 0.0 {
            bail!(InfeasibleDetected::InequalityConstraintBound {
                id: constraint_id,
                bound: f_bound,
            });
        }
        if f_bound.upper() <= 0.0 {
            return Ok(IntegerSlackRowPlan::Relax {
                constraint,
                reason: "add_integer_slack_to_inequality",
            });
        }

        let lower_magnitude = -f_bound.lower();
        let range = slack_upper_bound as f64;
        let b = if lower_magnitude == 0.0 {
            0.0
        } else {
            let rounded = lower_magnitude / range;
            if rounded.mul_add(range, -lower_magnitude).is_sign_negative() {
                rounded.next_up()
            } else {
                rounded
            }
        };
        let slack_bound = Bound::new(0.0, range).unwrap();

        // The upper bound is exactly representable. A negative fused residual
        // (including -0) means round-to-nearest landed below the quotient, so
        // moving up once yields directed rounding toward +infinity. Only an
        // actually zero lower magnitude may omit the slack term; quotient
        // underflow is repaired by `next_up()`.
        let b_coeff = match Coefficient::try_from(b) {
            Ok(c) => Some(c),
            Err(crate::CoefficientError::Zero) if lower_magnitude == 0.0 => None,
            Err(crate::CoefficientError::Zero) => unreachable!("next_up(0.0) is non-zero"),
            Err(e) => return Err(e).context("Slack coefficient must be finite"),
        };

        Ok(IntegerSlackRowPlan::Add {
            constraint,
            slack_bound,
            slack_coefficient: b_coeff,
            equality: Equality::LessThanOrEqualToZero,
            residual_coefficient: Some(b),
        })
    }

    fn integer_inequality(
        &self,
        constraint_id: ConstraintID,
        kinds: &fnv::FnvHashMap<VariableID, Kind>,
    ) -> Result<Constraint> {
        let constraint = self
            .constraint_collection
            .active()
            .get(&constraint_id)
            .cloned()
            .with_context(|| format!("Constraint ID {constraint_id:?} not found"))?;

        if constraint.equality != Equality::LessThanOrEqualToZero {
            bail!("The constraint is not inequality: ID={constraint_id:?}");
        }

        for id in constraint.function().required_ids() {
            let kind = kinds
                .get(&id)
                .with_context(|| format!("Decision variable ID {id:?} not found"))?;
            if !matches!(kind, Kind::Binary | Kind::Integer) {
                bail!("The constraint contains continuous decision variables: ID={id:?}");
            }
        }
        Ok(constraint)
    }

    fn plan_integer_slack_mutation(
        &self,
        plans: impl IntoIterator<Item = (ConstraintID, IntegerSlackRowPlan)>,
        atol: ATol,
    ) -> Result<IntegerSlackMutationPlan> {
        let plans = plans.into_iter().collect::<Vec<_>>();
        let slack_count = plans
            .iter()
            .filter(|(_, plan)| matches!(plan, IntegerSlackRowPlan::Add { .. }))
            .count();

        let first_slack_id = if slack_count == 0 {
            None
        } else {
            let first = self.next_variable_id()?;
            let additional =
                u64::try_from(slack_count - 1).map_err(|_| DecisionVariableError::NoAvailableID)?;
            first
                .into_inner()
                .checked_add(additional)
                .ok_or(DecisionVariableError::NoAvailableID)?;
            Some(first.into_inner())
        };

        let mut decision_variables = Vec::with_capacity(slack_count);
        let mut replacements = BTreeMap::new();
        let mut removals = BTreeMap::new();
        let mut residual_coefficients = Vec::with_capacity(plans.len());
        let mut slack_offset = 0_u64;

        for (constraint_id, plan) in plans {
            residual_coefficients.push(plan.residual_coefficient());
            match plan {
                IntegerSlackRowPlan::Relax { constraint, reason } => {
                    let previous = removals.insert(
                        constraint_id,
                        (
                            constraint,
                            RemovedReason {
                                reason: reason.to_string(),
                                parameters: Default::default(),
                            },
                        ),
                    );
                    debug_assert!(previous.is_none());
                }
                IntegerSlackRowPlan::Add {
                    mut constraint,
                    slack_bound,
                    slack_coefficient,
                    equality,
                    ..
                } => {
                    let slack_id = VariableID::from(
                        first_slack_id
                            .expect("an Add plan contributes to the preflighted slack count")
                            .checked_add(slack_offset)
                            .expect("the complete fresh-ID interval was preflighted"),
                    );
                    slack_offset += 1;

                    let row = DecisionVariable::new(Kind::Integer, slack_bound, atol)?;
                    let label = ModelingLabel {
                        name: Some("ommx.slack".to_string()),
                        subscripts: vec![constraint_id.into_inner() as i64],
                        ..Default::default()
                    };
                    decision_variables.push(PlannedSlackDecisionVariable {
                        id: slack_id,
                        row,
                        label,
                        atol,
                    });

                    if let Some(coefficient) = slack_coefficient {
                        let slack_term =
                            Linear::single_term(LinearMonomial::Variable(slack_id), coefficient);
                        *constraint.function_mut() = (constraint.function().clone() + slack_term)?;
                    }
                    constraint.equality = equality;
                    let previous = replacements.insert(constraint_id, constraint);
                    debug_assert!(previous.is_none());
                }
            }
        }

        Ok(IntegerSlackMutationPlan {
            decision_variables,
            replacements,
            removals,
            residual_coefficients,
        })
    }

    fn commit_integer_slack_mutation(
        &mut self,
        mutation: IntegerSlackMutationPlan,
    ) -> Vec<Option<f64>> {
        let IntegerSlackMutationPlan {
            decision_variables,
            replacements,
            removals,
            residual_coefficients,
        } = mutation;

        for planned in decision_variables {
            self.decision_variables
                .insert(planned.id, planned.row, planned.label, None, planned.atol)
                .expect("fresh slack decision-variable rows were fully validated while planning");
        }
        self.constraint_collection
            .replace_and_remove_active_rows(replacements, removals)
            .expect("active constraint rows were fully validated while planning");

        residual_coefficients
    }

    /// Snapshot of bounds for every decision variable.
    fn bounds(&self) -> Bounds {
        self.decision_variables
            .iter()
            .map(|(id, dv)| (*id, dv.bound()))
            .collect()
    }

    /// Snapshot of kinds for every decision variable.
    fn kinds(&self) -> fnv::FnvHashMap<VariableID, Kind> {
        self.decision_variables
            .iter()
            .map(|(id, dv)| (*id, dv.kind()))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{coeff, linear, ConstraintID, DecisionVariable, Function, Sense, VariableID};
    use maplit::btreemap;

    fn integer_inequality_instance() -> Instance {
        let id = VariableID::from(1);
        let decision_variables = btreemap! {
            id => DecisionVariable::new(
                Kind::Integer,
                Bound::new(0.0, 3.0).unwrap(),
                ATol::default(),
            ).unwrap(),
        };
        let constraint = crate::Constraint::less_than_or_equal_to_zero(Function::from(
            linear!(id) + coeff!(-2.0),
        ));
        Instance::new(
            Sense::Minimize,
            Function::from(linear!(id)),
            decision_variables,
            btreemap! { ConstraintID::from(0) => constraint },
        )
        .unwrap()
    }

    #[test]
    fn approximate_integer_slack_accepts_inclusive_error_bound() {
        let mut instance = integer_inequality_instance();

        instance.add_integer_slack_to_inequality(0, 1, 2.0).unwrap();

        assert_eq!(instance.decision_variables().len(), 2);
        assert_eq!(
            instance
                .constraints()
                .get(&ConstraintID::from(0))
                .unwrap()
                .equality,
            Equality::LessThanOrEqualToZero
        );
    }

    #[test]
    fn approximate_integer_slack_rejects_excessive_error_without_mutation() {
        let mut instance = integer_inequality_instance();
        let expected = instance.clone();

        let error = instance
            .add_integer_slack_to_inequality(0, 1, 1.0)
            .unwrap_err();

        assert!(error.to_string().contains("exceeds maximum error 1"));
        assert!(!error.is::<ExactIntegerSlackUnavailable>());
        assert_eq!(instance, expected);
    }

    #[test]
    fn approximate_integer_slack_validates_error_before_mutation() {
        for max_error in [0.0, -1.0, f64::INFINITY, f64::NAN] {
            let mut instance = integer_inequality_instance();
            let expected = instance.clone();

            let error = instance
                .add_integer_slack_to_inequality(0, 1, max_error)
                .unwrap_err();

            assert!(error.to_string().contains("must be positive and finite"));
            assert_eq!(instance, expected);
        }
    }

    #[test]
    fn approximate_integer_slack_rounds_up_an_underflowed_quotient() {
        let id = VariableID::from(1);
        let coefficient = Coefficient::try_from(f64::from_bits(1)).unwrap();
        let function = Function::from(Linear::single_term(
            LinearMonomial::Variable(id),
            coefficient,
        ));
        let mut instance = Instance::new(
            Sense::Minimize,
            Function::Zero,
            btreemap! {
                id => DecisionVariable::new(
                    Kind::Integer,
                    Bound::new(-1.0, 1.0).unwrap(),
                    ATol::default(),
                ).unwrap(),
            },
            btreemap! {
                ConstraintID::from(0) =>
                    crate::Constraint::less_than_or_equal_to_zero(function)
            },
        )
        .unwrap();

        let residual_coefficient = instance
            .add_integer_slack_to_inequality(0, 2, f64::from_bits(1))
            .unwrap()
            .expect("the inequality is not trivially satisfied");

        assert_eq!(residual_coefficient, f64::from_bits(1));
        assert!(residual_coefficient * 2.0 >= f64::from_bits(1));
        assert_eq!(instance.decision_variables().len(), 2);
    }

    #[test]
    fn converts_integer_inequality_to_equality_with_slack() {
        // min x1 + x2 s.t. x1 + x2 - 4 <= 0, with x1, x2 integer in [0, 3]
        let dv = btreemap! {
            VariableID::from(1) => DecisionVariable::new(
                Kind::Integer,
                Bound::new(0.0, 3.0).unwrap(),
                ATol::default(),
            ).unwrap(),
            VariableID::from(2) => DecisionVariable::new(
                Kind::Integer,
                Bound::new(0.0, 3.0).unwrap(),
                ATol::default(),
            ).unwrap(),
        };
        let objective = (Function::from(linear!(1)) + Function::from(linear!(2))).unwrap();
        let constraint_fn = ((Function::from(linear!(1)) + Function::from(linear!(2))).unwrap()
            + coeff!(-4.0))
        .unwrap();
        let constraints = btreemap! {
            ConstraintID::from(0) => crate::Constraint::less_than_or_equal_to_zero(constraint_fn,
            ),
        };
        let mut instance = Instance::new(Sense::Minimize, objective, dv, constraints).unwrap();

        instance
            .convert_inequality_to_equality_with_integer_slack(0, 32, ATol::default())
            .unwrap();

        let constraint = instance
            .constraints()
            .get(&ConstraintID::from(0))
            .expect("constraint should still be present");
        assert_eq!(constraint.equality, Equality::EqualToZero);
        // Slack var should have been added
        let store = instance.variable_labels();
        assert!(instance
            .decision_variables
            .keys()
            .any(|id| store.name(*id) == Some("ommx.slack")));
    }

    #[test]
    fn exact_integer_slack_range_limit_is_a_recoverable_signal() {
        let id = VariableID::from(1);
        let dv = btreemap! {
            id => DecisionVariable::new(
                Kind::Integer,
                Bound::new(0.0, 3.0).unwrap(),
                ATol::default(),
            ).unwrap(),
        };
        let objective = Function::from(linear!(1));
        let constraint_fn = (Function::from(linear!(1)) + coeff!(-2.0)).unwrap();
        let constraints = btreemap! {
            ConstraintID::from(0) => crate::Constraint::less_than_or_equal_to_zero(constraint_fn),
        };
        let mut instance = Instance::new(Sense::Minimize, objective, dv, constraints).unwrap();

        let err = instance
            .convert_inequality_to_equality_with_integer_slack(0, 1, ATol::default())
            .unwrap_err();

        assert!(err.is::<ExactIntegerSlackUnavailable>());
        assert!(instance.constraints().contains_key(&ConstraintID::from(0)));

        instance.add_integer_slack_to_inequality(0, 1, 2.0).unwrap();
        assert_eq!(instance.decision_variables().len(), 2);
        assert_eq!(
            instance
                .constraints()
                .get(&ConstraintID::from(0))
                .unwrap()
                .equality,
            Equality::LessThanOrEqualToZero
        );
    }

    #[test]
    fn exact_integer_slack_preserves_content_factor_signal() {
        let id = VariableID::from(1);
        let dv = btreemap! {
            id => DecisionVariable::new(
                Kind::Integer,
                Bound::new(0.0, 1.0).unwrap(),
                ATol::default(),
            ).unwrap(),
        };
        let constraint_fn = Function::from(Linear::single_term(
            LinearMonomial::Variable(id),
            Coefficient::try_from(f64::MAX).unwrap(),
        ));
        let constraints = btreemap! {
            ConstraintID::from(0) => crate::Constraint::less_than_or_equal_to_zero(constraint_fn),
        };
        let mut instance = Instance::new(Sense::Minimize, Function::Zero, dv, constraints).unwrap();

        let err = instance
            .convert_inequality_to_equality_with_integer_slack(0, 32, ATol::default())
            .unwrap_err();

        assert!(err.is::<ExactIntegerSlackUnavailable>());
        assert!(matches!(
            err.downcast_ref::<crate::ContentFactorError>(),
            Some(crate::ContentFactorError::CannotApproximateCoefficient)
        ));
        assert_eq!(instance.decision_variables().len(), 1);
        assert!(instance.constraints().contains_key(&ConstraintID::from(0)));
    }

    #[test]
    fn add_integer_slack_updates_function_but_keeps_inequality() {
        // min x1 s.t. x1 - 2 <= 0, x1 integer in [0, 3]
        let dv = btreemap! {
            VariableID::from(1) => DecisionVariable::new(
                Kind::Integer,
                Bound::new(0.0, 3.0).unwrap(),
                ATol::default(),
            ).unwrap(),
        };
        let objective = Function::from(linear!(1));
        let constraint_fn = (Function::from(linear!(1)) + coeff!(-2.0)).unwrap();
        let constraints = btreemap! {
            ConstraintID::from(0) => crate::Constraint::less_than_or_equal_to_zero(constraint_fn,
            ),
        };
        let mut instance = Instance::new(Sense::Minimize, objective, dv, constraints).unwrap();

        let b = instance
            .add_integer_slack_to_inequality(0, 2, 1.0)
            .unwrap()
            .expect("constraint should still be active");
        assert!(b > 0.0);

        let constraint = instance
            .constraints()
            .get(&ConstraintID::from(0))
            .expect("constraint should still be present");
        assert_eq!(constraint.equality, Equality::LessThanOrEqualToZero);
        let store = instance.variable_labels();
        assert!(instance
            .decision_variables
            .keys()
            .any(|id| store.name(*id) == Some("ommx.slack")));
    }

    #[test]
    fn approximate_integer_slack_rounds_the_coefficient_up_only_when_needed() {
        let id = VariableID::from(1);
        let mut instance = Instance::new(
            Sense::Minimize,
            Function::Zero,
            btreemap! {
                id => DecisionVariable::new(
                    Kind::Integer,
                    Bound::new(0.0, 2.0).unwrap(),
                    ATol::default(),
                ).unwrap(),
            },
            btreemap! {
                ConstraintID::from(0) => crate::Constraint::less_than_or_equal_to_zero(
                    Function::from(linear!(id) + coeff!(-1.0))
                ),
            },
        )
        .unwrap();

        let coefficient = instance
            .add_integer_slack_to_inequality(0, 3, 0.34)
            .unwrap()
            .expect("the inequality is not trivially satisfied");

        assert_eq!(coefficient, (1.0_f64 / 3.0).next_up());
        assert!(!coefficient.mul_add(3.0, -1.0).is_sign_negative());

        let mut exactly_divisible = integer_inequality_instance();
        let coefficient = exactly_divisible
            .add_integer_slack_to_inequality(0, 1, 2.0)
            .unwrap()
            .expect("the inequality is not trivially satisfied");
        assert_eq!(coefficient, 2.0);
    }

    #[test]
    fn always_satisfied_inequality_is_relaxed() {
        // x1 - 10 <= 0 with x1 in [0, 3] is always satisfied
        let dv = btreemap! {
            VariableID::from(1) => DecisionVariable::new(
                Kind::Integer,
                Bound::new(0.0, 3.0).unwrap(),
                ATol::default(),
            ).unwrap(),
        };
        let objective = Function::from(linear!(1));
        let constraint_fn = (Function::from(linear!(1)) + coeff!(-10.0)).unwrap();
        let constraints = btreemap! {
            ConstraintID::from(0) => crate::Constraint::less_than_or_equal_to_zero(constraint_fn,
            ),
        };
        let mut instance = Instance::new(Sense::Minimize, objective, dv, constraints).unwrap();

        let result = instance.add_integer_slack_to_inequality(0, 2, 1.0).unwrap();
        assert!(result.is_none());
        assert!(instance.constraints().is_empty());
        assert_eq!(instance.removed_constraints().len(), 1);
    }

    #[test]
    fn rejects_zero_slack_upper_bound_without_mutating_instance() {
        let dv = btreemap! {
            VariableID::from(1) => DecisionVariable::new(
                Kind::Integer,
                Bound::new(0.0, 3.0).unwrap(),
                ATol::default(),
            ).unwrap(),
        };
        let objective = Function::from(linear!(1));
        let constraint_fn = (Function::from(linear!(1)) + coeff!(-2.0)).unwrap();
        let constraints = btreemap! {
            ConstraintID::from(0) => crate::Constraint::less_than_or_equal_to_zero(constraint_fn,
            ),
        };
        let mut instance = Instance::new(Sense::Minimize, objective, dv, constraints).unwrap();
        let expected = instance.clone();

        let err = instance
            .add_integer_slack_to_inequality(0, 0, 1.0)
            .unwrap_err();
        assert!(err.to_string().contains("1..=2^53"));
        assert_eq!(instance, expected);
    }

    #[test]
    fn rejects_inexact_slack_upper_bound_without_mutating_instance() {
        let mut instance = integer_inequality_instance();
        let expected = instance.clone();

        let err = instance
            .add_integer_slack_to_inequality(0, MAX_APPROXIMATE_INTEGER_SLACK_RANGE + 1, 1.0)
            .unwrap_err();

        assert!(err.to_string().contains("1..=2^53"));
        assert_eq!(instance, expected);
    }

    #[test]
    fn convert_inequality_rejects_equality_constraint() {
        // An `EqualToZero` constraint must be rejected by
        // `convert_inequality_to_equality_with_integer_slack`, which names the
        // contract in its identifier. Matches the guard in the sibling
        // `add_integer_slack_to_inequality`.
        let dv = btreemap! {
            VariableID::from(1) => DecisionVariable::new(
                Kind::Integer,
                Bound::new(0.0, 3.0).unwrap(),
                ATol::default(),
            ).unwrap(),
        };
        let objective = Function::from(linear!(1));
        let constraint_fn = (Function::from(linear!(1)) + coeff!(-2.0)).unwrap();
        let constraints = btreemap! {
            ConstraintID::from(0) => crate::Constraint::equal_to_zero(constraint_fn,
            ),
        };
        let mut instance = Instance::new(Sense::Minimize, objective, dv, constraints).unwrap();

        let err = instance
            .convert_inequality_to_equality_with_integer_slack(0, 32, ATol::default())
            .unwrap_err();
        assert!(!err.is::<ExactIntegerSlackUnavailable>());
        assert!(err.to_string().contains("not inequality"));
    }

    #[test]
    fn rejects_constraint_with_continuous_variable() {
        let dv = btreemap! {
            VariableID::from(1) => DecisionVariable::continuous(),
        };
        let objective = Function::from(linear!(1));
        let constraint_fn = (Function::from(linear!(1)) + coeff!(-2.0)).unwrap();
        let constraints = btreemap! {
            ConstraintID::from(0) => crate::Constraint::less_than_or_equal_to_zero(constraint_fn,
            ),
        };
        let mut instance = Instance::new(Sense::Minimize, objective, dv, constraints).unwrap();

        let err = instance
            .convert_inequality_to_equality_with_integer_slack(0, 32, ATol::default())
            .unwrap_err();
        assert!(!err.is::<ExactIntegerSlackUnavailable>());
        assert!(err.to_string().contains("continuous decision variables"));
    }
}
