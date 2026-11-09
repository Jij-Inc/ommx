use super::Instance;
use crate::{
    constraint::{Equality, Provenance, RemovedReason},
    ATol, Bound, Bounds, Coefficient, Constraint, ConstraintID, DecisionVariable, Evaluate,
    Function, InfeasibleDetected, Kind, Linear, LinearMonomial, VariableID,
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
    /// The original row, ID, labels, and context are retained in
    /// [`Self::removed_constraints`]. The generated equality has a fresh ID
    /// recorded as `constraint_id` in the source's removal parameters, and
    /// appends [`Provenance::Constraint`] to its inherited context. Subsequent
    /// transformations must select the generated ID, not the removed source ID.
    /// If the inequality is already satisfied throughout its domain, only the
    /// source is removed and no generated row is created. Failures leave the
    /// instance unchanged.
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

        self.introduce_integer_slack(
            constraint_id,
            function,
            slack_bound,
            Some(a.inv()?),
            Equality::EqualToZero,
            "ommx.Instance.convert_inequality_to_equality_with_integer_slack",
            atol,
        )
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
    /// The source row, ID, labels, and context remain in Interpretation Context;
    /// the generated inequality has a fresh ID linked through the source's
    /// `constraint_id` removal parameter and [`Provenance::Constraint`].
    /// All validation and arithmetic complete before any mutation.
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

        self.introduce_integer_slack(
            constraint_id,
            function,
            slack_bound,
            b_coeff,
            Equality::LessThanOrEqualToZero,
            "ommx.Instance.add_integer_slack_to_inequality",
            atol,
        )?;
        Ok(Some(b))
    }

    /// Stage the generated row and variable before publishing either one.
    #[allow(clippy::too_many_arguments)]
    fn introduce_integer_slack(
        &mut self,
        source_id: ConstraintID,
        function: Function,
        slack_bound: Bound,
        coefficient: Option<Coefficient>,
        equality: Equality,
        reason: &str,
        atol: ATol,
    ) -> Result<()> {
        let generated_id = self.next_constraint_number()?.into();
        let slack_id = self.next_variable_id()?;
        let slack = DecisionVariable::new(Kind::Integer, slack_bound, atol)?;
        let function = match coefficient {
            Some(c) => {
                let slack_term = Linear::single_term(LinearMonomial::Variable(slack_id), c);
                (function + slack_term)?
            }
            None => function,
        };

        let mut constraint = Constraint::equal_to_zero(function);
        constraint.equality = equality;
        let mut context = self.constraint_collection.context().collect_for(source_id);
        context.provenance.push(Provenance::Constraint(source_id));

        self.decision_variables
            .insert(
                slack_id,
                slack,
                crate::ModelingLabel {
                    name: Some("ommx.slack".to_string()),
                    subscripts: vec![source_id.into_inner() as i64],
                    ..Default::default()
                },
                None,
                atol,
            )
            .expect("fresh ID and integer slack domain were validated before mutation");
        self.constraint_collection
            .insert_active_with_context(generated_id, constraint, context)
            .expect("fresh constraint ID was allocated before mutation");
        self.constraint_collection
            .relax(
                source_id,
                RemovedReason {
                    reason: reason.to_owned(),
                    parameters: [
                        (
                            "constraint_id".to_owned(),
                            generated_id.into_inner().to_string(),
                        ),
                        (
                            "slack_variable_id".to_owned(),
                            slack_id.into_inner().to_string(),
                        ),
                    ]
                    .into_iter()
                    .collect(),
                },
            )
            .expect("source row was validated before mutation");
        Ok(())
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

    fn source_instance() -> Instance {
        Instance::new(
            Sense::Minimize,
            Function::Zero,
            btreemap! { 0.into() => DecisionVariable::new(
                Kind::Integer, Bound::new(0.0, 2.0).unwrap(), ATol::default()
            ).unwrap() },
            btreemap! { 0.into() => Constraint::less_than_or_equal_to_zero(
                Function::from(linear!(0) + coeff!(-1.0))
            ) },
        )
        .unwrap()
    }

    #[test]
    fn slack_preserves_source_context_and_separates_source_feasibility() {
        use crate::{ConstraintContext, ModelingLabel, Parse};
        for exact in [true, false] {
            let mut instance = source_instance();
            let source = instance.constraints()[&0.into()].clone();
            let context = ConstraintContext {
                label: ModelingLabel {
                    name: Some("capacity".into()),
                    subscripts: vec![4],
                    ..Default::default()
                },
                provenance: vec![Provenance::OneHotConstraint(7.into())],
            };
            instance
                .set_constraint_context(0.into(), context.clone())
                .unwrap();
            if exact {
                instance
                    .convert_inequality_to_equality_with_integer_slack(0, 10, ATol::default())
                    .unwrap();
            } else {
                instance
                    .add_integer_slack_to_inequality(0, 2, ATol::default())
                    .unwrap();
            }
            let (retained, reason) = &instance.removed_constraints()[&0.into()];
            assert_eq!(retained, &source);
            assert_eq!(reason.parameters["constraint_id"], "1");
            assert_eq!(reason.parameters["slack_variable_id"], "1");
            assert_eq!(instance.constraint_context().collect_for(0.into()), context);
            let mut generated_context = context;
            generated_context
                .provenance
                .push(Provenance::Constraint(0.into()));
            assert_eq!(
                instance.constraint_context().collect_for(1.into()),
                generated_context
            );
            // The original source condition is feasible, independently of whether
            // the chosen auxiliary value satisfies the generated row.
            let state = crate::v1::State::from_iter([(0, 1.0), (1, 1.0)]);
            assert!(retained
                .evaluate(&state, ATol::default())
                .unwrap()
                .is_feasible(ATol::default()));
            assert!(!instance.constraints()[&1.into()]
                .evaluate(&state, ATol::default())
                .unwrap()
                .is_feasible(ATol::default()));
            let raw: crate::v2::Instance = instance.clone().into();
            assert_eq!(raw.parse(&()).unwrap(), instance);
        }
    }

    #[test]
    fn slack_chains_retain_each_source_and_append_lineage() {
        let mut instance = source_instance();
        instance
            .add_integer_slack_to_inequality(0, 1, ATol::default())
            .unwrap();
        let intermediate = instance.constraints()[&1.into()].clone();
        instance
            .convert_inequality_to_equality_with_integer_slack(1, 10, ATol::default())
            .unwrap();
        assert_eq!(instance.removed_constraints()[&1.into()].0, intermediate);
        assert_eq!(
            instance.removed_constraints()[&0.into()].1.parameters["constraint_id"],
            "1"
        );
        assert_eq!(
            instance.removed_constraints()[&1.into()].1.parameters["constraint_id"],
            "2"
        );
        assert_eq!(
            instance.constraint_context().provenance(2.into()),
            &[
                Provenance::Constraint(0.into()),
                Provenance::Constraint(1.into())
            ]
        );
        assert_eq!(instance.constraints().len(), 1);
        instance
            .log_encode_all_used_integers(ATol::default())
            .unwrap();
        assert_eq!(instance.removed_constraints()[&1.into()].0, intermediate);
    }

    #[test]
    fn slack_v1_retains_source_and_reverse_links() {
        for exact in [true, false] {
            let mut instance = source_instance();
            instance
                .set_constraint_context(
                    0.into(),
                    crate::ConstraintContext {
                        label: crate::ModelingLabel {
                            name: Some("capacity".into()),
                            subscripts: vec![7],
                            ..Default::default()
                        },
                        provenance: vec![],
                    },
                )
                .unwrap();
            if exact {
                instance
                    .convert_inequality_to_equality_with_integer_slack(0, 10, ATol::default())
                    .unwrap();
            } else {
                instance
                    .add_integer_slack_to_inequality(0, 2, ATol::default())
                    .unwrap();
            }
            let restored = Instance::from_v1_bytes(&instance.to_v1_bytes().unwrap()).unwrap();
            assert_eq!(restored.constraints(), instance.constraints());
            assert_eq!(
                restored.removed_constraints(),
                instance.removed_constraints()
            );
            assert_eq!(
                restored.constraint_context().collect_for(0.into()),
                instance.constraint_context().collect_for(0.into())
            );
            assert_eq!(
                restored.constraint_context().collect_for(1.into()).label,
                instance.constraint_context().collect_for(1.into()).label
            );
            assert!(restored
                .constraint_context()
                .provenance(1.into())
                .is_empty());
        }
    }

    #[test]
    fn slack_ids_account_for_active_and_removed_special_constraints() {
        for exact in [true, false] {
            for removed in [true, false] {
                let mut instance = source_instance();
                let binary = instance
                    .new_binary(
                        Bound::new(0.0, 1.0).unwrap(),
                        Default::default(),
                        None,
                        ATol::default(),
                    )
                    .unwrap();
                instance
                    .one_hot_constraint_collection
                    .insert_active_with_context(
                        50.into(),
                        crate::OneHotConstraint::new([binary].into_iter().collect()).unwrap(),
                        Default::default(),
                    )
                    .unwrap();
                if removed {
                    instance
                        .one_hot_constraint_collection
                        .relax(
                            50.into(),
                            RemovedReason {
                                reason: "test".into(),
                                parameters: Default::default(),
                            },
                        )
                        .unwrap();
                }
                if exact {
                    instance
                        .convert_inequality_to_equality_with_integer_slack(0, 10, ATol::default())
                        .unwrap();
                } else {
                    instance
                        .add_integer_slack_to_inequality(0, 2, ATol::default())
                        .unwrap();
                }
                assert!(instance.constraints().contains_key(&51.into()));
                assert_eq!(
                    instance.removed_constraints()[&0.into()].1.parameters["constraint_id"],
                    "51"
                );
            }
        }
    }

    #[test]
    fn slack_allocation_failure_is_atomic_for_active_and_removed_ids() {
        for exact in [true, false] {
            for removed in [true, false] {
                let mut instance = source_instance();
                instance
                    .insert_constraint(u64::MAX.into(), Constraint::equal_to_zero(Function::Zero))
                    .unwrap();
                if removed {
                    instance
                        .relax_constraint(u64::MAX.into(), "test".into(), [])
                        .unwrap();
                }
                let before = instance.clone();
                let result = if exact {
                    instance.convert_inequality_to_equality_with_integer_slack(
                        0,
                        10,
                        ATol::default(),
                    )
                } else {
                    instance
                        .add_integer_slack_to_inequality(0, 2, ATol::default())
                        .map(|_| ())
                };
                assert!(result.is_err());
                assert_eq!(instance, before);
            }
        }
    }

    proptest::proptest! {
        #[test]
        fn slack_provenance_roundtrips(
            mut instance in proptest::arbitrary::any::<Instance>(),
            exact in proptest::bool::ANY, upper in 2_u32..100,
        ) {
            use crate::Parse;
            let variable = instance.new_integer(Bound::new(0.0, 2.0).unwrap(), Default::default(), None, ATol::default()).unwrap();
            let source_id = instance.add_constraint(
                Constraint::less_than_or_equal_to_zero(Function::from(linear!(variable) + coeff!(-1.0))),
                Default::default(),
            ).unwrap();
            if exact {
                instance.convert_inequality_to_equality_with_integer_slack(source_id.into_inner(), u64::from(upper), ATol::default()).unwrap();
            } else {
                instance.add_integer_slack_to_inequality(source_id.into_inner(), u64::from(upper), ATol::default()).unwrap();
            }
            let raw: crate::v2::Instance = instance.clone().into();
            proptest::prop_assert_eq!(raw.parse(&()).unwrap(), instance);
        }
    }

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
            .get(&ConstraintID::from(1))
            .expect("generated constraint should be present");
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
        let constraint = &instance.constraints()[&ConstraintID::from(1)];
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
        assert!(feasible.is_feasible(atol));
        assert!(!infeasible.is_feasible(atol));
    }

    #[test]
    fn inequality_preserving_slack_uses_constraint_atol_at_a_positive_lower_bound() {
        let atol = ATol::new(0.25).unwrap();
        let mut instance = tolerance_boundary_instance();

        let coefficient = instance
            .add_integer_slack_to_inequality(0, 1, atol)
            .unwrap();
        assert_eq!(coefficient, Some(0.0));

        let constraint = &instance.constraints()[&ConstraintID::from(1)];
        let feasible = constraint
            .evaluate(&crate::v1::State::from_iter([(1, 0.0)]), atol)
            .unwrap();
        let infeasible = constraint
            .evaluate(&crate::v1::State::from_iter([(1, 1.0)]), atol)
            .unwrap();
        assert!(feasible.is_feasible(atol));
        assert!(!infeasible.is_feasible(atol));
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
            .get(&ConstraintID::from(1))
            .expect("generated constraint should be present");
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
