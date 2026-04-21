use super::Instance;
use crate::{
    constraint::{ConstraintID, Equality, Provenance, RemovedReason},
    indicator_constraint::IndicatorConstraintID,
    Bounds, Coefficient, Constraint, Evaluate, Function, Kind, Linear, LinearMonomial, VariableID,
};
use anyhow::{bail, Context, Result};
use std::collections::BTreeMap;

/// Big-M sides planned for a single indicator constraint.
#[derive(Debug, Clone)]
struct IndicatorPlan {
    indicator_variable: VariableID,
    function: Function,
    /// Upper-side Big-M coefficient $u$ (strictly positive). Emits `f(x) + u y - u <= 0`.
    /// `None` means the upper side is redundant (upper bound of $f$ is $\leq 0$) and skipped.
    upper_big_m: Option<Coefficient>,
    /// Lower-side Big-M coefficient $l$ (strictly negative). Emits `-f(x) - l y + l <= 0`.
    /// `None` means the lower side is redundant (lower bound of $f$ is $\geq 0$) or the
    /// constraint is an inequality (no lower side to emit).
    lower_big_m: Option<Coefficient>,
}

impl Instance {
    #[cfg_attr(doc, katexit::katexit)]
    /// Convert an indicator constraint to regular constraints using the Big-M method.
    ///
    /// An indicator constraint `$y = 1 \Rightarrow f(x) \leq 0$` (or `$= 0$`) with
    /// binary $y$ is encoded with upper / lower Big-M sides computed from the interval
    /// bounds of $f(x)$:
    ///
    /// $$
    /// f(x) + u y - u \leq 0, \qquad -f(x) - l y + l \leq 0,
    /// $$
    ///
    /// where $u \geq \sup f(x)$ and $l \leq \inf f(x)$ are taken from
    /// [`Function::evaluate_bound`] over the decision variables' bounds.
    ///
    /// Side emission rules:
    /// - For [`Equality::LessThanOrEqualToZero`], only the upper side is considered.
    ///   It is emitted only if $u > 0$; otherwise $f(x) \leq 0$ is already implied by
    ///   the variable bounds and the constraint is redundant.
    /// - For [`Equality::EqualToZero`], both sides are considered independently.
    ///   The upper side is emitted iff $u > 0$ and the lower side iff $l < 0$.
    ///
    /// When an equality side is skipped, the remaining constraints still enforce the
    /// implication correctly because the skipped inequality is already implied by the
    /// variable bounds:
    /// - If $u \leq 0$, the bound $f(x) \leq u \leq 0$ substitutes for the skipped
    ///   upper side. If $l < 0$ is also emitted, it gives $f(x) \geq 0$ at $y = 1$,
    ///   which combined with $f(x) \leq u$ forces $f(x) = 0$ when $u = 0$ or renders
    ///   $y = 1$ infeasible when $u < 0$ (correctly reflecting that $f(x) = 0$ has no
    ///   solution under the given bounds).
    /// - Symmetrically for $l \geq 0$ with the lower side skipped.
    /// - If both $u = 0$ and $l = 0$, the interval bound says $f(x) \equiv 0$, so the
    ///   equality holds vacuously for every $y$ and nothing needs to be emitted.
    ///
    /// Returns the [`ConstraintID`]s of the newly created regular constraints in
    /// insertion order (upper side first if emitted, then lower side).
    ///
    /// Errors if the function's bound is non-finite on a side that would need to be
    /// emitted: `upper` must be finite, and additionally `lower` must be finite for
    /// equality indicators. Also errors if $f(x)$ references a variable of kind
    /// [`Kind::SemiInteger`] or [`Kind::SemiContinuous`]: their split domain
    /// $\{0\} \cup [l, u]$ is not uniformly implemented across the codebase, and
    /// [`Function::evaluate_bound`] over $[l, u]$ alone can under-bound $\sup f$
    /// when $0 \notin [l, u]$. The instance is not mutated on error — all validation
    /// happens before any constraints are inserted.
    ///
    /// If the indicator variable $y$ itself appears in $f(x)$, the interval bound
    /// treats it as a free binary in $[0, 1]$; the resulting Big-M is still a valid
    /// (possibly loose) over-approximation and the implication is preserved.
    ///
    /// The original indicator constraint is moved to
    /// [`Instance::removed_indicator_constraints`] with
    /// `reason = "ommx.Instance.convert_indicator_to_constraint"` and a
    /// `constraint_ids` parameter listing the new regular constraint IDs
    /// (comma-separated in insertion order; empty when nothing is emitted).
    pub fn convert_indicator_to_constraint(
        &mut self,
        id: IndicatorConstraintID,
    ) -> Result<Vec<ConstraintID>> {
        let plan = self.plan_indicator_conversion(id)?;
        Ok(self.apply_indicator_conversion(id, plan))
    }

    /// Convert every active indicator constraint to regular constraints using Big-M.
    ///
    /// See [`Self::convert_indicator_to_constraint`] for the conversion rule.
    ///
    /// This is atomic: every active indicator is validated up front, and only once
    /// all validations succeed are the conversions applied. If any indicator fails
    /// validation (non-finite bound on a required side), no mutation happens and
    /// the instance is left untouched.
    ///
    /// Returns a map from each original [`IndicatorConstraintID`] to the IDs of the
    /// regular constraints it produced.
    pub fn convert_all_indicators_to_constraints(
        &mut self,
    ) -> Result<BTreeMap<IndicatorConstraintID, Vec<ConstraintID>>> {
        let ids: Vec<_> = self
            .indicator_constraint_collection
            .active()
            .keys()
            .copied()
            .collect();
        let mut all_plans: Vec<(IndicatorConstraintID, IndicatorPlan)> =
            Vec::with_capacity(ids.len());
        for id in ids {
            let plan = self.plan_indicator_conversion(id)?;
            all_plans.push((id, plan));
        }
        let mut result = BTreeMap::new();
        for (id, plan) in all_plans {
            result.insert(id, self.apply_indicator_conversion(id, plan));
        }
        Ok(result)
    }

    /// Validate a single indicator constraint and build its conversion plan.
    ///
    /// Read-only: never mutates `self`. Errors before producing any plan if the
    /// indicator is missing or if `Function::evaluate_bound` returns a non-finite
    /// bound on a side that would need to be emitted.
    fn plan_indicator_conversion(&self, id: IndicatorConstraintID) -> Result<IndicatorPlan> {
        let ic = self
            .indicator_constraint_collection
            .active()
            .get(&id)
            .with_context(|| format!("Indicator constraint with ID {id:?} not found"))?;
        let function = ic.function().clone();
        let equality = ic.equality;
        let indicator_variable = ic.indicator_variable;

        // Semi-continuous / semi-integer variables carry a split domain `{0} ∪ [l, u]`
        // that the rest of the codebase does not yet treat uniformly. `evaluate_bound`
        // would compute `sup/inf f` over the `[l, u]` piece only, silently missing the
        // `{0}` piece when `0 ∉ [l, u]`; this can under-bound `u` (e.g. `f = -x + 0.5`
        // with `x ∈ {0} ∪ [2, 5]` gives interval `u = -1.5` but true `sup f = 0.5`),
        // causing the upper Big-M to be wrongly skipped. Reject these kinds explicitly,
        // matching `convert_sos1_to_constraints`.
        for var_id in function.required_ids() {
            let dv = self.decision_variables.get(&var_id).with_context(|| {
                format!(
                    "Decision variable {var_id:?} referenced by indicator constraint {id:?} not found"
                )
            })?;
            if matches!(dv.kind(), Kind::SemiInteger | Kind::SemiContinuous) {
                bail!(
                    "Cannot convert indicator constraint {id:?} with Big-M: variable {var_id:?} has kind {:?}; semi-continuous / semi-integer variables are not supported",
                    dv.kind()
                );
            }
        }

        let bounds: Bounds = self
            .decision_variables
            .iter()
            .map(|(v, dv)| (*v, dv.bound()))
            .collect();
        let fbound = function.evaluate_bound(&bounds);

        // Upper side is always considered. Require a finite upper bound.
        let upper_val = fbound.upper();
        if !upper_val.is_finite() {
            bail!(
                "Cannot convert indicator constraint {id:?} with Big-M: function has non-finite upper bound {upper_val}"
            );
        }
        let upper_big_m = if upper_val > 0.0 {
            Some(Coefficient::try_from(upper_val).expect("finite positive upper bound"))
        } else {
            None
        };

        // Lower side is only relevant for equality indicators.
        let lower_big_m = match equality {
            Equality::EqualToZero => {
                let lower_val = fbound.lower();
                if !lower_val.is_finite() {
                    bail!(
                        "Cannot convert indicator constraint {id:?} with Big-M: function has non-finite lower bound {lower_val}"
                    );
                }
                if lower_val < 0.0 {
                    Some(Coefficient::try_from(lower_val).expect("finite negative lower bound"))
                } else {
                    None
                }
            }
            Equality::LessThanOrEqualToZero => None,
        };

        Ok(IndicatorPlan {
            indicator_variable,
            function,
            upper_big_m,
            lower_big_m,
        })
    }

    /// Apply a pre-validated indicator conversion plan.
    ///
    /// Infallible given a plan returned by [`Self::plan_indicator_conversion`] on
    /// the current instance.
    fn apply_indicator_conversion(
        &mut self,
        id: IndicatorConstraintID,
        plan: IndicatorPlan,
    ) -> Vec<ConstraintID> {
        let mut new_ids: Vec<ConstraintID> = Vec::new();
        let y = plan.indicator_variable;

        if let Some(u) = plan.upper_big_m {
            // f(x) + u y - u <= 0
            let f = plan.function.clone()
                + Linear::single_term(LinearMonomial::Variable(y), u)
                + Linear::from(-u);
            let new_id = self.insert_indicator_generated_constraint(
                id,
                Constraint::less_than_or_equal_to_zero(f),
            );
            new_ids.push(new_id);
        }

        if let Some(l) = plan.lower_big_m {
            // -f(x) - l y + l <= 0
            let neg_l = -l;
            let f = -plan.function.clone()
                + Linear::single_term(LinearMonomial::Variable(y), neg_l)
                + Linear::from(l);
            let new_id = self.insert_indicator_generated_constraint(
                id,
                Constraint::less_than_or_equal_to_zero(f),
            );
            new_ids.push(new_id);
        }

        let mut parameters = fnv::FnvHashMap::default();
        let constraint_ids_str = new_ids
            .iter()
            .map(|id| id.into_inner().to_string())
            .collect::<Vec<_>>()
            .join(",");
        parameters.insert("constraint_ids".to_string(), constraint_ids_str);
        self.indicator_constraint_collection
            .relax(
                id,
                RemovedReason {
                    reason: "ommx.Instance.convert_indicator_to_constraint".to_string(),
                    parameters,
                },
            )
            .expect(
                "indicator id was present when the plan was built and hasn't been touched since",
            );

        new_ids
    }

    fn insert_indicator_generated_constraint(
        &mut self,
        indicator_id: IndicatorConstraintID,
        mut constraint: Constraint,
    ) -> ConstraintID {
        let new_id = self.constraint_collection.unused_id();
        constraint
            .metadata
            .provenance
            .push(Provenance::IndicatorConstraint(indicator_id));
        self.constraint_collection
            .active_mut()
            .insert(new_id, constraint);
        new_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        coeff, indicator_constraint::IndicatorConstraint, linear, ATol, Bound, DecisionVariable,
        Kind, Sense,
    };
    use ::approx::assert_abs_diff_eq;
    use maplit::btreemap;
    use std::collections::BTreeMap;

    /// Build an instance with one binary indicator `y` (id=10) and a continuous
    /// `x` (id=1) with the given bound, plus a single indicator constraint
    /// `y = 1 → f(x) <equality> 0` (function provided by the caller).
    fn single_indicator_instance(
        x_bound: Bound,
        equality: Equality,
        function: Function,
    ) -> Instance {
        let x = DecisionVariable::new(
            VariableID::from(1),
            Kind::Continuous,
            x_bound,
            None,
            ATol::default(),
        )
        .unwrap();
        let y = DecisionVariable::binary(VariableID::from(10));

        let ic = IndicatorConstraint::new(VariableID::from(10), equality, function);

        Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::from(linear!(1)))
            .decision_variables(btreemap! {
                VariableID::from(1) => x,
                VariableID::from(10) => y,
            })
            .constraints(BTreeMap::new())
            .indicator_constraints(BTreeMap::from([(IndicatorConstraintID::from(7), ic)]))
            .build()
            .unwrap()
    }

    #[test]
    fn inequality_emits_only_upper_bigm() {
        // y=1 → (x - 2) <= 0, with x in [0, 5]: upper bound of f = x - 2 is 3, lower is -2.
        // Inequality only considers the upper side → emit 1 constraint `f + 3 y - 3 <= 0`.
        let f = Function::from(linear!(1) + coeff!(-2.0));
        let mut instance = single_indicator_instance(
            Bound::new(0.0, 5.0).unwrap(),
            Equality::LessThanOrEqualToZero,
            f,
        );

        let new_ids = instance
            .convert_indicator_to_constraint(IndicatorConstraintID::from(7))
            .unwrap();
        assert_eq!(new_ids.len(), 1);

        let c = instance.constraints().get(&new_ids[0]).unwrap();
        assert_eq!(c.equality, Equality::LessThanOrEqualToZero);
        let expected = Function::from(
            linear!(1)
                + Linear::single_term(LinearMonomial::Variable(VariableID::from(10)), coeff!(3.0))
                + coeff!(-5.0), // (-2) [original] + (-3) [big-M constant] = -5
        );
        assert_abs_diff_eq!(c.function(), &expected);
        assert_eq!(
            c.metadata.provenance,
            vec![Provenance::IndicatorConstraint(
                IndicatorConstraintID::from(7)
            )]
        );

        // Original indicator moved to removed with expected reason + constraint_ids.
        assert!(instance.indicator_constraints().is_empty());
        let (_, reason) = instance
            .removed_indicator_constraints()
            .get(&IndicatorConstraintID::from(7))
            .expect("indicator retained as removed");
        assert_eq!(
            reason.reason,
            "ommx.Instance.convert_indicator_to_constraint"
        );
        assert_eq!(
            reason.parameters.get("constraint_ids").map(String::as_str),
            Some(new_ids[0].into_inner().to_string().as_str())
        );
    }

    #[test]
    fn equality_emits_both_sides_when_bounds_straddle_zero() {
        // y=1 → (x - 2) = 0, with x in [0, 5]: upper = 3, lower = -2.
        // Equality emits upper (u=3) and lower (l=-2) sides in that order.
        let f = Function::from(linear!(1) + coeff!(-2.0));
        let mut instance =
            single_indicator_instance(Bound::new(0.0, 5.0).unwrap(), Equality::EqualToZero, f);

        let new_ids = instance
            .convert_indicator_to_constraint(IndicatorConstraintID::from(7))
            .unwrap();
        assert_eq!(new_ids.len(), 2);

        // Upper: (x - 2) + 3 y - 3 = x + 3 y - 5
        let upper = instance.constraints().get(&new_ids[0]).unwrap();
        assert_eq!(upper.equality, Equality::LessThanOrEqualToZero);
        let expected_upper = Function::from(
            linear!(1)
                + Linear::single_term(LinearMonomial::Variable(VariableID::from(10)), coeff!(3.0))
                + coeff!(-5.0),
        );
        assert_abs_diff_eq!(upper.function(), &expected_upper);

        // Lower: -(x - 2) - (-2) y + (-2) = -x + 2 + 2 y - 2 = -x + 2 y
        let lower = instance.constraints().get(&new_ids[1]).unwrap();
        assert_eq!(lower.equality, Equality::LessThanOrEqualToZero);
        let expected_lower = Function::from(
            Linear::single_term(LinearMonomial::Variable(VariableID::from(1)), coeff!(-1.0))
                + Linear::single_term(LinearMonomial::Variable(VariableID::from(10)), coeff!(2.0)),
        );
        assert_abs_diff_eq!(lower.function(), &expected_lower);
    }

    #[test]
    fn redundant_side_is_skipped() {
        // y=1 → x - 10 <= 0 with x in [0, 5]: upper = -5 <= 0, so constraint is
        // always satisfied by bounds. No big-M emitted; indicator simply relaxed.
        let f = Function::from(linear!(1) + coeff!(-10.0));
        let mut instance = single_indicator_instance(
            Bound::new(0.0, 5.0).unwrap(),
            Equality::LessThanOrEqualToZero,
            f,
        );
        let before_constraints = instance.constraints().clone();

        let new_ids = instance
            .convert_indicator_to_constraint(IndicatorConstraintID::from(7))
            .unwrap();
        assert!(
            new_ids.is_empty(),
            "redundant indicator should emit nothing"
        );
        assert_eq!(
            instance.constraints(),
            &before_constraints,
            "no new constraints added for redundant indicator"
        );
        let (_, reason) = instance
            .removed_indicator_constraints()
            .get(&IndicatorConstraintID::from(7))
            .unwrap();
        assert_eq!(
            reason.parameters.get("constraint_ids").map(String::as_str),
            Some(""),
            "constraint_ids should be empty when no big-M was emitted"
        );
    }

    #[test]
    fn infinite_bound_is_rejected_without_mutation() {
        // Continuous x with default (infinite) bound → upper side bound = +∞.
        // Conversion must bail before any mutation.
        let x = DecisionVariable::continuous(VariableID::from(1));
        let y = DecisionVariable::binary(VariableID::from(10));
        let ic = IndicatorConstraint::new(
            VariableID::from(10),
            Equality::LessThanOrEqualToZero,
            Function::from(linear!(1)),
        );
        let mut instance = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::from(linear!(1)))
            .decision_variables(btreemap! {
                VariableID::from(1) => x,
                VariableID::from(10) => y,
            })
            .constraints(BTreeMap::new())
            .indicator_constraints(BTreeMap::from([(IndicatorConstraintID::from(7), ic)]))
            .build()
            .unwrap();
        let before_vars = instance.decision_variables.clone();
        let before_constraints = instance.constraints().clone();
        let before_indicators = instance.indicator_constraints().clone();

        let err = instance
            .convert_indicator_to_constraint(IndicatorConstraintID::from(7))
            .unwrap_err();
        assert!(err.to_string().contains("non-finite"));

        assert_eq!(instance.decision_variables, before_vars);
        assert_eq!(instance.constraints(), &before_constraints);
        assert_eq!(instance.indicator_constraints(), &before_indicators);
    }

    #[test]
    fn semi_continuous_variables_in_function_are_rejected() {
        // Regression for an issue flagged in review: semi-continuous / semi-integer
        // variables have a split domain `{0} ∪ [l, u]`. Computing `evaluate_bound`
        // over `[l, u]` alone can under-bound `sup f`, silently dropping the upper
        // Big-M. Example from the review: `x ∈ {0} ∪ [2, 5]`, `f = -x + 0.5`.
        // Interval bound gives `f ∈ [-4.5, -1.5]` (upper ≤ 0 → upper side skipped),
        // but the true `sup f = 0.5` at `x = 0` means the upper Big-M is needed.
        // Since this is unsafe, planner must reject semi kinds before using
        // `evaluate_bound`, matching `convert_sos1_to_constraints`.
        let x_semi = DecisionVariable::new(
            VariableID::from(1),
            Kind::SemiContinuous,
            Bound::new(2.0, 5.0).unwrap(),
            None,
            ATol::default(),
        )
        .unwrap();
        let y = DecisionVariable::binary(VariableID::from(10));
        let ic = IndicatorConstraint::new(
            VariableID::from(10),
            Equality::LessThanOrEqualToZero,
            Function::from(Linear::single_term(
                LinearMonomial::Variable(VariableID::from(1)),
                coeff!(-1.0),
            )) + coeff!(0.5),
        );
        let mut instance = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::from(linear!(1)))
            .decision_variables(btreemap! {
                VariableID::from(1) => x_semi,
                VariableID::from(10) => y,
            })
            .constraints(BTreeMap::new())
            .indicator_constraints(BTreeMap::from([(IndicatorConstraintID::from(7), ic)]))
            .build()
            .unwrap();
        let before_constraints = instance.constraints().clone();

        let err = instance
            .convert_indicator_to_constraint(IndicatorConstraintID::from(7))
            .unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("semi-continuous") && msg.contains("not supported"),
            "expected semi-not-supported error, got: {msg}"
        );
        // No mutation on error.
        assert!(instance
            .indicator_constraints()
            .contains_key(&IndicatorConstraintID::from(7)));
        assert_eq!(instance.constraints(), &before_constraints);
    }

    #[test]
    fn missing_id_errors_without_mutating_state() {
        let f = Function::from(linear!(1) + coeff!(-2.0));
        let mut instance = single_indicator_instance(
            Bound::new(0.0, 5.0).unwrap(),
            Equality::LessThanOrEqualToZero,
            f,
        );
        let before_indicators = instance.indicator_constraints().clone();
        let before_constraints = instance.constraints().clone();

        let err = instance
            .convert_indicator_to_constraint(IndicatorConstraintID::from(999))
            .unwrap_err();
        assert!(err.to_string().contains("999"));

        assert_eq!(instance.indicator_constraints(), &before_indicators);
        assert_eq!(instance.constraints(), &before_constraints);
    }

    #[test]
    fn bulk_conversion_returns_per_indicator_ids() {
        // Two indicators on the same indicator variable, both convertible:
        // #1: y=1 → x - 2 <= 0   → 1 big-M upper
        // #2: y=1 → x - 2 = 0    → 2 big-M (upper + lower)
        let x = DecisionVariable::new(
            VariableID::from(1),
            Kind::Continuous,
            Bound::new(0.0, 5.0).unwrap(),
            None,
            ATol::default(),
        )
        .unwrap();
        let y = DecisionVariable::binary(VariableID::from(10));
        let f = || Function::from(linear!(1) + coeff!(-2.0));
        let ic_le =
            IndicatorConstraint::new(VariableID::from(10), Equality::LessThanOrEqualToZero, f());
        let ic_eq = IndicatorConstraint::new(VariableID::from(10), Equality::EqualToZero, f());

        let mut instance = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::from(linear!(1)))
            .decision_variables(btreemap! {
                VariableID::from(1) => x,
                VariableID::from(10) => y,
            })
            .constraints(BTreeMap::new())
            .indicator_constraints(BTreeMap::from([
                (IndicatorConstraintID::from(1), ic_le),
                (IndicatorConstraintID::from(2), ic_eq),
            ]))
            .build()
            .unwrap();

        let result = instance.convert_all_indicators_to_constraints().unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[&IndicatorConstraintID::from(1)].len(), 1);
        assert_eq!(result[&IndicatorConstraintID::from(2)].len(), 2);
        assert!(instance.indicator_constraints().is_empty());
        assert_eq!(instance.removed_indicator_constraints().len(), 2);
    }

    #[test]
    fn bulk_conversion_is_atomic_on_error() {
        // Two indicators: first is convertible (bounded x), second has an unbounded
        // variable in its function. The bulk call must fail without applying the
        // first one either.
        let x1 = DecisionVariable::new(
            VariableID::from(1),
            Kind::Continuous,
            Bound::new(0.0, 5.0).unwrap(),
            None,
            ATol::default(),
        )
        .unwrap();
        let x2 = DecisionVariable::continuous(VariableID::from(2)); // infinite bound
        let y = DecisionVariable::binary(VariableID::from(10));
        let ic_ok = IndicatorConstraint::new(
            VariableID::from(10),
            Equality::LessThanOrEqualToZero,
            Function::from(linear!(1) + coeff!(-2.0)),
        );
        let ic_bad = IndicatorConstraint::new(
            VariableID::from(10),
            Equality::LessThanOrEqualToZero,
            Function::from(linear!(2)),
        );

        let mut instance = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::from(linear!(1)))
            .decision_variables(btreemap! {
                VariableID::from(1) => x1,
                VariableID::from(2) => x2,
                VariableID::from(10) => y,
            })
            .constraints(BTreeMap::new())
            .indicator_constraints(BTreeMap::from([
                (IndicatorConstraintID::from(1), ic_ok),
                (IndicatorConstraintID::from(2), ic_bad),
            ]))
            .build()
            .unwrap();
        let before_indicators = instance.indicator_constraints().clone();
        let before_constraints = instance.constraints().clone();

        let err = instance
            .convert_all_indicators_to_constraints()
            .unwrap_err();
        assert!(err.to_string().contains("non-finite"));

        assert_eq!(instance.indicator_constraints(), &before_indicators);
        assert_eq!(instance.constraints(), &before_constraints);
        assert!(instance.removed_indicator_constraints().is_empty());
    }
}
