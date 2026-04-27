use super::Instance;
use crate::{
    constraint::Equality, ATol, Bound, Bounds, Coefficient, ConstraintID, Evaluate,
    InfeasibleDetected, Kind, Linear, LinearMonomial, VariableID,
};
use anyhow::{bail, Context, Result};
use num::traits::Inv;

impl Instance {
    /// Convert an inequality $f(x) \leq 0$ to an equality $a f(x) + s = 0$ with a
    /// newly introduced integer slack variable $s$, where $a$ is the minimal positive
    /// factor that makes every coefficient of $f(x)$ integer.
    pub fn convert_inequality_to_equality_with_integer_slack(
        &mut self,
        constraint_id: u64,
        max_integer_range: u64,
        atol: ATol,
    ) -> Result<()> {
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

        let a = function
            .content_factor()
            .context("Cannot normalize the coefficients to integers")?;
        let af = function.clone() * a;

        let af_bound = af.evaluate_bound(&bounds);
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
            self.relax_constraint(
                constraint_id,
                "ommx.Instance.convert_inequality_to_equality_with_integer_slack".to_string(),
                [],
            )?;
            return Ok(());
        }

        let slack_bound = Bound::new(0.0, -af_bound.lower()).unwrap();
        if slack_bound.width() > max_integer_range as f64 {
            bail!(
                "The range of the slack variable exceeds the limit: evaluated({}) > limit({})",
                slack_bound.width(),
                max_integer_range
            );
        }

        let slack = self.new_decision_variable(Kind::Integer, slack_bound, None, atol)?;
        let slack_id = slack.id();
        // Drop borrow before writing to the metadata store on `self`.
        let _ = slack;
        let metadata = self.variable_metadata_mut();
        metadata.set_name(slack_id, "ommx.slack");
        metadata.set_subscripts(slack_id, vec![constraint_id.into_inner() as i64]);

        let slack_term = Linear::single_term(LinearMonomial::Variable(slack_id), a.inv());
        let new_function = function + slack_term;

        let constraint = self
            .constraint_collection
            .active_mut()
            .get_mut(&constraint_id)
            .expect("constraint presence was verified above");
        *constraint.function_mut() = new_function;
        constraint.equality = Equality::EqualToZero;

        Ok(())
    }

    /// Convert an inequality $f(x) \leq 0$ to $f(x) + b s \leq 0$ with an integer
    /// slack variable $s \in [0, \text{slack\_upper\_bound}]$.
    ///
    /// Returns the coefficient $b = -\mathrm{lower}(f(x)) / \text{slack\_upper\_bound}$.
    /// Returns `None` if the constraint was trivially satisfied and was moved to
    /// removed_constraints.
    pub fn add_integer_slack_to_inequality(
        &mut self,
        constraint_id: u64,
        slack_upper_bound: u64,
    ) -> Result<Option<f64>> {
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

        let f_bound = function.evaluate_bound(&bounds);
        if f_bound.lower() > 0.0 {
            bail!(InfeasibleDetected::InequalityConstraintBound {
                id: constraint_id,
                bound: f_bound,
            });
        }
        if f_bound.upper() <= 0.0 {
            self.relax_constraint(
                constraint_id,
                "add_integer_slack_to_inequality".to_string(),
                [],
            )?;
            return Ok(None);
        }

        let b = -f_bound.lower() / slack_upper_bound as f64;
        let slack_bound = Bound::new(0.0, slack_upper_bound as f64).unwrap();

        // Validate the slack coefficient before mutating the instance so failures
        // (e.g. `slack_upper_bound == 0` giving `b = inf`) do not leave an orphan
        // slack decision variable behind. `b` is non-negative in this branch (we
        // bailed on lower > 0 and relaxed when upper <= 0). `b == 0` only when
        // `f_bound.lower() == 0`, which is a boundary case; adding a zero-coefficient
        // slack term is mathematically a no-op so we skip the term in that case,
        // matching v1 observable behavior (where a zero coefficient is dropped on
        // insertion).
        let b_coeff = match Coefficient::try_from(b) {
            Ok(c) => Some(c),
            Err(crate::CoefficientError::Zero) => None,
            Err(e) => return Err(e).context("Slack coefficient must be finite"),
        };

        let slack =
            self.new_decision_variable(Kind::Integer, slack_bound, None, ATol::default())?;
        let slack_id = slack.id();
        let _ = slack;
        let metadata = self.variable_metadata_mut();
        metadata.set_name(slack_id, "ommx.slack");
        metadata.set_subscripts(slack_id, vec![constraint_id.into_inner() as i64]);

        let new_function = match b_coeff {
            Some(c) => {
                let slack_term = Linear::single_term(LinearMonomial::Variable(slack_id), c);
                function + slack_term
            }
            None => function,
        };

        let constraint = self
            .constraint_collection
            .active_mut()
            .get_mut(&constraint_id)
            .expect("constraint presence was verified above");
        *constraint.function_mut() = new_function;

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

    #[test]
    fn converts_integer_inequality_to_equality_with_slack() {
        // min x1 + x2 s.t. x1 + x2 - 4 <= 0, with x1, x2 integer in [0, 3]
        let dv = btreemap! {
            VariableID::from(1) => DecisionVariable::new(
                VariableID::from(1), Kind::Integer, Bound::new(0.0, 3.0).unwrap(), None, ATol::default()
            ).unwrap(),
            VariableID::from(2) => DecisionVariable::new(
                VariableID::from(2), Kind::Integer, Bound::new(0.0, 3.0).unwrap(), None, ATol::default()
            ).unwrap(),
        };
        let objective = Function::from(linear!(1)) + Function::from(linear!(2));
        let constraint_fn = Function::from(linear!(1)) + Function::from(linear!(2)) + coeff!(-4.0);
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
        let store = instance.variable_metadata();
        assert!(instance
            .decision_variables
            .keys()
            .any(|id| store.name(*id) == Some("ommx.slack")));
    }

    #[test]
    fn add_integer_slack_updates_function_but_keeps_inequality() {
        // min x1 s.t. x1 - 2 <= 0, x1 integer in [0, 3]
        let dv = btreemap! {
            VariableID::from(1) => DecisionVariable::new(
                VariableID::from(1), Kind::Integer, Bound::new(0.0, 3.0).unwrap(), None, ATol::default()
            ).unwrap(),
        };
        let objective = Function::from(linear!(1));
        let constraint_fn = Function::from(linear!(1)) + coeff!(-2.0);
        let constraints = btreemap! {
            ConstraintID::from(0) => crate::Constraint::less_than_or_equal_to_zero(constraint_fn,
            ),
        };
        let mut instance = Instance::new(Sense::Minimize, objective, dv, constraints).unwrap();

        let b = instance
            .add_integer_slack_to_inequality(0, 2)
            .unwrap()
            .expect("constraint should still be active");
        assert!(b > 0.0);

        let constraint = instance
            .constraints()
            .get(&ConstraintID::from(0))
            .expect("constraint should still be present");
        assert_eq!(constraint.equality, Equality::LessThanOrEqualToZero);
        let store = instance.variable_metadata();
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
                VariableID::from(1), Kind::Integer, Bound::new(0.0, 3.0).unwrap(), None, ATol::default()
            ).unwrap(),
        };
        let objective = Function::from(linear!(1));
        let constraint_fn = Function::from(linear!(1)) + coeff!(-10.0);
        let constraints = btreemap! {
            ConstraintID::from(0) => crate::Constraint::less_than_or_equal_to_zero(constraint_fn,
            ),
        };
        let mut instance = Instance::new(Sense::Minimize, objective, dv, constraints).unwrap();

        let result = instance.add_integer_slack_to_inequality(0, 2).unwrap();
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
                VariableID::from(1), Kind::Integer, Bound::new(0.0, 3.0).unwrap(), None, ATol::default()
            ).unwrap(),
        };
        let objective = Function::from(linear!(1));
        let constraint_fn = Function::from(linear!(1)) + coeff!(-2.0);
        let constraints = btreemap! {
            ConstraintID::from(0) => crate::Constraint::less_than_or_equal_to_zero(constraint_fn,
            ),
        };
        let mut instance = Instance::new(Sense::Minimize, objective, dv, constraints).unwrap();
        let before = instance.decision_variables.len();

        let err = instance.add_integer_slack_to_inequality(0, 0).unwrap_err();
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
                VariableID::from(1), Kind::Integer, Bound::new(0.0, 3.0).unwrap(), None, ATol::default()
            ).unwrap(),
        };
        let objective = Function::from(linear!(1));
        let constraint_fn = Function::from(linear!(1)) + coeff!(-2.0);
        let constraints = btreemap! {
            ConstraintID::from(0) => crate::Constraint::equal_to_zero(constraint_fn,
            ),
        };
        let mut instance = Instance::new(Sense::Minimize, objective, dv, constraints).unwrap();

        let err = instance
            .convert_inequality_to_equality_with_integer_slack(0, 32, ATol::default())
            .unwrap_err();
        assert!(err.to_string().contains("not inequality"));
    }

    #[test]
    fn rejects_constraint_with_continuous_variable() {
        let dv = btreemap! {
            VariableID::from(1) => DecisionVariable::continuous(VariableID::from(1)),
        };
        let objective = Function::from(linear!(1));
        let constraint_fn = Function::from(linear!(1)) + coeff!(-2.0);
        let constraints = btreemap! {
            ConstraintID::from(0) => crate::Constraint::less_than_or_equal_to_zero(constraint_fn,
            ),
        };
        let mut instance = Instance::new(Sense::Minimize, objective, dv, constraints).unwrap();

        let err = instance
            .convert_inequality_to_equality_with_integer_slack(0, 32, ATol::default())
            .unwrap_err();
        assert!(err.to_string().contains("continuous decision variables"));
    }
}
