use super::Instance;
use crate::{
    constraint::Equality, ATol, Bound, Bounds, Coefficient, ConstraintID, Evaluate,
    InfeasibleDetected, Kind, Linear, LinearMonomial, VariableID,
};
use anyhow::{bail, Context, Result};
use num::traits::Inv;

fn validate_integer_slack_atol(atol: ATol) -> Result<()> {
    if atol >= 0.5 {
        bail!(
            "Integer slack introduction requires atol < 0.5: got {}",
            atol.into_inner()
        );
    }
    Ok(())
}

/// Signal returned when exact integer-slack conversion is structurally valid
/// but cannot construct an exact finite encoding.
///
/// Callers may recover by changing the exact-conversion parameters or by
/// selecting a different operation whose postcondition does not require an
/// equality. Missing constraints, unsupported variable kinds, and other
/// contract or materialization failures are not classified as this signal.
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
    /// Convert an inequality $f(x) \leq 0$ to an equality $f(x) + s/a = 0$ with a
    /// newly introduced integer slack variable $s$, where $a$ is the minimal positive
    /// factor that makes every coefficient of $a f(x)$ integer.
    ///
    /// # Errors
    ///
    /// Returns [`ExactIntegerSlackUnavailable`] when exact coefficient
    /// normalization or the configured finite slack range is unavailable, and
    /// [`InfeasibleDetected`] when variable bounds prove the inequality
    /// infeasible. Missing constraints, unsupported variable kinds, allocation,
    /// and coefficient-arithmetic failures retain their ordinary error types.
    /// Feasibility and triviality are classified with the supplied `atol`,
    /// matching [`crate::EvaluatedConstraint`] semantics. `atol` must be less
    /// than `0.5` so its use while normalizing Integer bounds remains
    /// unambiguous.
    pub fn convert_inequality_to_equality_with_integer_slack(
        &mut self,
        constraint_id: u64,
        max_integer_range: u64,
        atol: ATol,
    ) -> Result<()> {
        validate_integer_slack_atol(atol)?;
        let constraint_id = ConstraintID::from(constraint_id);
        let bounds = self.bounds();
        let kinds = self.kinds();

        let (function, equality) = {
            let constraint = self
                .constraint_collection
                .active()
                .get(&constraint_id)
                .with_context(|| format!("Constraint ID {constraint_id:?} not found"))?;
            (constraint.function().clone(), constraint.equality)
        };

        if equality != Equality::LessThanOrEqualToZero {
            bail!("The constraint is not inequality: ID={constraint_id:?}");
        }

        for id in function.required_ids() {
            let kind = kinds
                .get(&id)
                .with_context(|| format!("Decision variable ID {id:?} not found"))?;
            if !matches!(kind, Kind::Binary | Kind::Integer) {
                bail!("The constraint contains continuous decision variables: ID={id:?}");
            }
        }

        // Classify the original (unscaled) function with the same inclusive
        // zero boundary as evaluation so coefficient normalization cannot
        // change that tolerance boundary.
        let f_bound = function.evaluate_bound(&bounds, atol)?;
        if !Equality::LessThanOrEqualToZero.is_satisfied(f_bound.lower(), atol) {
            bail!(InfeasibleDetected::InequalityConstraintBound {
                id: constraint_id,
                bound: f_bound,
            });
        }
        if Equality::LessThanOrEqualToZero.is_satisfied(f_bound.upper(), atol) {
            self.relax_constraint(
                constraint_id,
                "ommx.Instance.convert_inequality_to_equality_with_integer_slack".to_string(),
                [],
            )?;
            return Ok(());
        }

        let a = function.content_factor().map_err(|source| {
            source.context(ExactIntegerSlackUnavailable::coefficients_not_normalizable(
                constraint_id,
            ))
        })?;
        let af = (function.clone() * a)?;

        let af_bound = af.evaluate_bound(&bounds, atol)?;
        let af_bound = af_bound.as_integer_bound(atol).ok_or(
            InfeasibleDetected::InequalityConstraintBound {
                id: constraint_id,
                bound: af_bound,
            },
        )?;
        // A positive minimum can still be feasible when it is at or below `atol`.
        // In that case s=0 preserves the original tolerance classification.
        let slack_bound = Bound::new(0.0, (-af_bound.lower()).max(0.0)).unwrap();
        if slack_bound.width() > max_integer_range as f64 {
            return Err(ExactIntegerSlackUnavailable::range_too_large(
                constraint_id,
                slack_bound.width(),
                max_integer_range,
            )
            .into());
        }

        let slack_id = self.new_decision_variable_with_label(
            Kind::Integer,
            slack_bound,
            crate::ModelingLabel {
                name: Some("ommx.slack".to_string()),
                subscripts: vec![constraint_id.into_inner() as i64],
                ..Default::default()
            },
            None,
            atol,
        )?;

        let slack_term = Linear::single_term(LinearMonomial::Variable(slack_id), a.inv()?);
        let new_function = (function + slack_term)?;

        let mut constraint = self
            .constraint_collection
            .active()
            .get(&constraint_id)
            .cloned()
            .expect("constraint presence was verified above");
        *constraint.function_mut() = new_function;
        constraint.equality = Equality::EqualToZero;
        self.constraint_collection
            .replace_active_row(constraint_id, constraint)?;

        Ok(())
    }

    /// Convert an inequality $f(x) \leq 0$ to $f(x) + b s \leq 0$ with an integer
    /// slack variable $s \in [0, \text{slack\_upper\_bound}]$.
    ///
    /// This preserves the feasible assignments of the original decision
    /// variables exactly: every original feasible assignment remains feasible
    /// by choosing $s = 0$, while $b \geq 0$ and $s \geq 0$ ensure that every
    /// augmented feasible assignment still satisfies $f(x) \leq 0$. The
    /// relation remains an inequality; this operation is not an approximation
    /// of the original feasible set.
    ///
    /// Returns the coefficient $b = -\mathrm{lower}(f(x)) / \text{slack\_upper\_bound}$.
    /// Returns `None` if the constraint was trivially satisfied and was moved to
    /// removed_constraints.
    /// `atol` controls both zero-sensitive operations while computing the
    /// function bound and the inequality feasibility threshold used to classify
    /// infeasible or trivial bounds. It must be less than `0.5` so its use while
    /// normalizing Integer bounds remains unambiguous.
    pub fn add_integer_slack_to_inequality(
        &mut self,
        constraint_id: u64,
        slack_upper_bound: u64,
        atol: ATol,
    ) -> Result<Option<f64>> {
        validate_integer_slack_atol(atol)?;
        let constraint_id = ConstraintID::from(constraint_id);
        let bounds = self.bounds();
        let kinds = self.kinds();

        let (function, equality) = {
            let constraint = self
                .constraint_collection
                .active()
                .get(&constraint_id)
                .with_context(|| format!("Constraint ID {constraint_id:?} not found"))?;
            (constraint.function().clone(), constraint.equality)
        };

        if equality != Equality::LessThanOrEqualToZero {
            bail!("The constraint is not inequality: ID={constraint_id:?}");
        }

        for id in function.required_ids() {
            let kind = kinds
                .get(&id)
                .with_context(|| format!("Decision variable ID {id:?} not found"))?;
            if !matches!(kind, Kind::Binary | Kind::Integer) {
                bail!("The constraint contains continuous decision variables: ID={id:?}");
            }
        }

        let f_bound = function.evaluate_bound(&bounds, atol)?;
        if !Equality::LessThanOrEqualToZero.is_satisfied(f_bound.lower(), atol) {
            bail!(InfeasibleDetected::InequalityConstraintBound {
                id: constraint_id,
                bound: f_bound,
            });
        }
        if Equality::LessThanOrEqualToZero.is_satisfied(f_bound.upper(), atol) {
            self.relax_constraint(
                constraint_id,
                "add_integer_slack_to_inequality".to_string(),
                [],
            )?;
            return Ok(None);
        }

        let slack_magnitude = (-f_bound.lower()).max(0.0);
        let b = if slack_magnitude == 0.0 {
            0.0
        } else {
            slack_magnitude / slack_upper_bound as f64
        };
        let slack_bound = Bound::new(0.0, slack_upper_bound as f64).unwrap();

        // Validate the slack coefficient before mutating the instance so failures
        // (e.g. `slack_upper_bound == 0` giving `b = inf`) do not leave an orphan
        // slack decision variable behind. `b` is non-negative in this branch.
        // `b == 0` when every bounded value is non-negative but some are still
        // feasible under `atol`; adding a zero-coefficient slack term is a no-op,
        // so we skip the term in that case,
        // matching v1 observable behavior (where a zero coefficient is dropped on
        // insertion).
        let b_coeff = match Coefficient::try_from(b) {
            Ok(c) => Some(c),
            Err(crate::CoefficientError::Zero) => None,
            Err(e) => return Err(e).context("Slack coefficient must be finite"),
        };

        let slack_id = self.new_decision_variable_with_label(
            Kind::Integer,
            slack_bound,
            crate::ModelingLabel {
                name: Some("ommx.slack".to_string()),
                subscripts: vec![constraint_id.into_inner() as i64],
                ..Default::default()
            },
            None,
            atol,
        )?;

        let new_function = match b_coeff {
            Some(c) => {
                let slack_term = Linear::single_term(LinearMonomial::Variable(slack_id), c);
                (function + slack_term)?
            }
            None => function,
        };

        let mut constraint = self
            .constraint_collection
            .active()
            .get(&constraint_id)
            .cloned()
            .expect("constraint presence was verified above");
        *constraint.function_mut() = new_function;
        self.constraint_collection
            .replace_active_row(constraint_id, constraint)?;

        Ok(Some(b))
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
    use std::collections::BTreeMap;

    fn tolerance_boundary_instance() -> Instance {
        let id = VariableID::from(1);
        let function = ((coeff!(1.0) * linear!(id)).unwrap() + coeff!(0.25)).unwrap();
        Instance::new(
            Sense::Minimize,
            Function::Zero,
            btreemap! { id => DecisionVariable::binary() },
            btreemap! {
                ConstraintID::from(0) =>
                    crate::Constraint::less_than_or_equal_to_zero(Function::from(function)),
            },
        )
        .unwrap()
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
    fn exact_integer_slack_uses_constraint_atol_at_a_positive_lower_bound() {
        // f(0)=0.25 is feasible at the inclusive boundary and f(1)=1.25 is
        // infeasible at atol=0.25. The
        // integer-normalized bound is [1, 5], so an exact-zero check would
        // incorrectly reject the whole constraint as infeasible.
        let atol = ATol::new(0.25).unwrap();
        let mut instance = tolerance_boundary_instance();

        instance
            .convert_inequality_to_equality_with_integer_slack(0, 1, atol)
            .unwrap();

        let slack_id = instance
            .decision_variables()
            .keys()
            .copied()
            .find(|id| instance.variable_labels().name(*id) == Some("ommx.slack"))
            .unwrap();
        let constraint = &instance.constraints()[&ConstraintID::from(0)];
        let feasible = constraint
            .evaluate(
                &crate::v1::State::from_iter([(1, 0.0), (slack_id.into_inner(), 0.0)]),
                atol,
            )
            .unwrap();
        let infeasible = constraint
            .evaluate(
                &crate::v1::State::from_iter([(1, 1.0), (slack_id.into_inner(), 0.0)]),
                atol,
            )
            .unwrap();
        assert!(feasible.is_feasible_with_tolerance(atol));
        assert!(!infeasible.is_feasible_with_tolerance(atol));
    }

    #[test]
    fn inequality_preserving_slack_uses_constraint_atol_at_a_positive_lower_bound() {
        let atol = ATol::new(0.25).unwrap();
        let mut instance = tolerance_boundary_instance();

        let coefficient = instance
            .add_integer_slack_to_inequality(0, 1, atol)
            .unwrap();
        assert_eq!(coefficient, Some(0.0));

        let constraint = &instance.constraints()[&ConstraintID::from(0)];
        let feasible = constraint
            .evaluate(&crate::v1::State::from_iter([(1, 0.0)]), atol)
            .unwrap();
        let infeasible = constraint
            .evaluate(&crate::v1::State::from_iter([(1, 1.0)]), atol)
            .unwrap();
        assert!(feasible.is_feasible_with_tolerance(atol));
        assert!(!infeasible.is_feasible_with_tolerance(atol));
    }

    #[test]
    fn slack_conversion_relaxes_a_positive_but_tolerance_feasible_constraint() {
        let atol = ATol::new(0.25).unwrap();
        for exact in [true, false] {
            let mut instance = Instance::new(
                Sense::Minimize,
                Function::Zero,
                BTreeMap::new(),
                btreemap! {
                    ConstraintID::from(0) => crate::Constraint::less_than_or_equal_to_zero(
                        Function::try_from(0.25).unwrap()
                    ),
                },
            )
            .unwrap();

            if exact {
                instance
                    .convert_inequality_to_equality_with_integer_slack(0, 1, atol)
                    .unwrap();
            } else {
                assert_eq!(
                    instance
                        .add_integer_slack_to_inequality(0, 1, atol)
                        .unwrap(),
                    None
                );
            }
            assert!(instance.constraints().is_empty());
            assert!(instance
                .removed_constraints()
                .contains_key(&ConstraintID::from(0)));
        }
    }

    #[test]
    fn slack_conversion_rejects_a_constant_just_outside_atol() {
        let atol = ATol::new(0.25).unwrap();
        let outside = f64::from_bits(0.25_f64.to_bits() + 1);

        for exact in [true, false] {
            let mut instance = Instance::new(
                Sense::Minimize,
                Function::Zero,
                BTreeMap::new(),
                btreemap! {
                    ConstraintID::from(0) => crate::Constraint::less_than_or_equal_to_zero(
                        Function::try_from(outside).unwrap()
                    ),
                },
            )
            .unwrap();
            let before = instance.clone();

            let err = if exact {
                instance
                    .convert_inequality_to_equality_with_integer_slack(0, 1, atol)
                    .unwrap_err()
            } else {
                instance
                    .add_integer_slack_to_inequality(0, 1, atol)
                    .unwrap_err()
            };

            assert!(err.is::<InfeasibleDetected>());
            assert_eq!(instance, before);
        }
    }

    #[test]
    fn exact_integer_slack_rejects_large_atol_without_mutating_instance() {
        for value in [0.5, 1.0] {
            let mut instance = tolerance_boundary_instance();
            let before = instance.clone();

            let err = instance
                .convert_inequality_to_equality_with_integer_slack(0, 1, ATol::new(value).unwrap())
                .unwrap_err();

            assert!(err.to_string().contains("requires atol < 0.5"));
            assert!(!err.is::<ExactIntegerSlackUnavailable>());
            assert_eq!(instance, before);
        }
    }

    #[test]
    fn inequality_preserving_slack_rejects_large_atol_without_mutating_instance() {
        for value in [0.5, 1.0] {
            let mut instance = tolerance_boundary_instance();
            let before = instance.clone();

            let err = instance
                .add_integer_slack_to_inequality(0, 1, ATol::new(value).unwrap())
                .unwrap_err();

            assert!(err.to_string().contains("requires atol < 0.5"));
            assert_eq!(instance, before);
        }
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
        let before = instance.clone();

        let err = instance
            .convert_inequality_to_equality_with_integer_slack(0, 1, ATol::default())
            .unwrap_err();

        assert!(err.is::<ExactIntegerSlackUnavailable>());
        assert_eq!(instance, before);
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
            .add_integer_slack_to_inequality(0, 2, ATol::default())
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

        let result = instance
            .add_integer_slack_to_inequality(0, 2, ATol::default())
            .unwrap();
        assert!(result.is_none());
        assert!(instance.constraints().is_empty());
        assert_eq!(instance.removed_constraints().len(), 1);
    }

    #[test]
    fn rejects_zero_slack_upper_bound_without_mutating_instance() {
        // f(x) = x1 - 2 with x1 in [0, 3] gives a finite non-zero lower, so
        // `slack_upper_bound == 0` drives `b = inf`. The call must fail without
        // leaving behind an orphan slack decision variable.
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
        let before = instance.decision_variables.len();

        let err = instance
            .add_integer_slack_to_inequality(0, 0, ATol::default())
            .unwrap_err();
        assert!(err.to_string().to_lowercase().contains("finite"));
        // No slack variable should have been added on the failure path.
        assert_eq!(instance.decision_variables.len(), before);
        // The original constraint is still the untouched inequality.
        let constraint = instance.constraints().get(&ConstraintID::from(0)).unwrap();
        assert_eq!(constraint.equality, Equality::LessThanOrEqualToZero);
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
