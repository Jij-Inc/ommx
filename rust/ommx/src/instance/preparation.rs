use super::{ExactIntegerSlackUnavailable, Instance, SpecialConstraintKinds};
use crate::{ATol, ConstraintID, Equality, InstanceClass};
use std::collections::BTreeMap;

/// Arguments passed to
/// [`Instance::convert_inequality_to_equality_with_integer_slack`] by
/// [`Instance::prepare`].
///
/// Preparation supplies each active inequality's `constraint_id`; the remaining
/// arguments retain the names and meanings defined by the owner operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConvertInequalityToEqualityWithIntegerSlackArguments {
    /// Maximum finite range accepted for an exact Integer slack variable.
    pub max_integer_range: u64,
    /// Absolute tolerance used to normalize bounds to integers.
    pub atol: ATol,
}

/// Arguments passed to [`Instance::add_integer_slack_to_inequality`] by
/// [`Instance::prepare`].
///
/// Preparation supplies each active inequality's `constraint_id`; the remaining
/// argument retains the name and meaning defined by the owner operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AddIntegerSlackToInequalityArguments {
    /// Upper bound passed to the approximate Integer slack operation.
    pub slack_upper_bound: u64,
}

/// Optional arguments for the [`Instance`] owner operations interpreted by
/// [`Instance::prepare`].
///
/// Every field is named after the owner operation it configures. `None` means
/// that operation is not invoked. The policy does not contain an
/// [`InstanceClass`], redefine any mathematical argument, or add validation on
/// top of the configured owner APIs.
///
/// Preparation interprets configured operations in one canonical order:
/// special-constraint lowering, minimization normalization, exact Integer slack
/// with the configured approximate fallback, used-Integer log encoding, keyed
/// fixed penalties, then uniform fixed penalties. Whole-class membership is
/// checked before and between owner operations, so interpretation stops as soon
/// as the target class contains the instance.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PreparationPolicy {
    /// Arguments for [`Instance::lower_special_constraints`].
    pub lower_special_constraints: Option<SpecialConstraintKinds>,
    /// Invoke [`Instance::as_minimization_problem`] when this is `Some(())`.
    pub as_minimization_problem: Option<()>,
    /// Arguments for exact Integer slack conversion of every active inequality.
    pub convert_inequality_to_equality_with_integer_slack:
        Option<ConvertInequalityToEqualityWithIntegerSlackArguments>,
    /// Arguments for approximate Integer slack conversion.
    ///
    /// When exact conversion is configured, this is used only after
    /// [`ExactIntegerSlackUnavailable`]. When exact conversion is absent, it is
    /// applied directly to every active inequality.
    pub add_integer_slack_to_inequality: Option<AddIntegerSlackToInequalityArguments>,
    /// Absolute tolerance passed to [`Instance::log_encode_all_used_integers`].
    pub log_encode_all_used_integers: Option<ATol>,
    /// Constraint-ID-keyed weights passed unchanged to
    /// [`Instance::penalty_method_with_fixed_weights`].
    pub penalty_method_with_fixed_weights: Option<BTreeMap<ConstraintID, f64>>,
    /// Weight passed unchanged to
    /// [`Instance::uniform_penalty_method_with_fixed_weight`].
    pub uniform_penalty_method_with_fixed_weight: Option<f64>,
}

impl Instance {
    /// Prepare this instance for membership in `input_class` using configured
    /// [`Instance`] owner operations.
    ///
    /// This is a partial, in-place interpreter. It checks whole-class
    /// membership before and between configured operations and returns as soon
    /// as `input_class.contains(self)` is true. Success therefore guarantees
    /// only membership in `input_class`; adapter-specific applicability remains
    /// outside this operation.
    ///
    /// Active regular inequalities are processed in ascending constraint-ID
    /// order. Exact Integer slack conversion falls back to the configured
    /// approximate operation only for [`ExactIntegerSlackUnavailable`]. All
    /// other owner errors are propagated unchanged.
    ///
    /// Preparation has no global transaction. Each owner operation retains its
    /// own mutation and failure semantics, and changes committed by earlier
    /// operations remain when a later operation fails.
    ///
    /// # Errors
    ///
    /// Returns an error from a configured owner operation, or an ordinary
    /// [`crate::Error`] with the final membership report when the configured
    /// operations are exhausted without reaching `input_class`.
    pub fn prepare(
        &mut self,
        input_class: &InstanceClass,
        policy: &PreparationPolicy,
    ) -> crate::Result<()> {
        if input_class.contains(self) {
            return Ok(());
        }

        if let Some(kinds) = &policy.lower_special_constraints {
            self.lower_special_constraints(kinds)?;
            if input_class.contains(self) {
                return Ok(());
            }
        }

        if policy.as_minimization_problem.is_some() {
            self.as_minimization_problem();
            if input_class.contains(self) {
                return Ok(());
            }
        }

        if policy
            .convert_inequality_to_equality_with_integer_slack
            .is_some()
            || policy.add_integer_slack_to_inequality.is_some()
        {
            let inequality_ids = self
                .constraints()
                .iter()
                .filter_map(|(&id, constraint)| {
                    (constraint.equality == Equality::LessThanOrEqualToZero).then_some(id)
                })
                .collect::<Vec<_>>();

            for id in inequality_ids {
                if let Some(arguments) = policy.convert_inequality_to_equality_with_integer_slack {
                    match self.convert_inequality_to_equality_with_integer_slack(
                        id.into_inner(),
                        arguments.max_integer_range,
                        arguments.atol,
                    ) {
                        Ok(()) => {}
                        Err(error) if error.is::<ExactIntegerSlackUnavailable>() => {
                            let Some(approximate) = policy.add_integer_slack_to_inequality else {
                                return Err(error);
                            };
                            self.add_integer_slack_to_inequality(
                                id.into_inner(),
                                approximate.slack_upper_bound,
                            )?;
                        }
                        Err(error) => return Err(error),
                    }
                } else if let Some(arguments) = policy.add_integer_slack_to_inequality {
                    self.add_integer_slack_to_inequality(
                        id.into_inner(),
                        arguments.slack_upper_bound,
                    )?;
                }

                if input_class.contains(self) {
                    return Ok(());
                }
            }
        }

        if let Some(atol) = policy.log_encode_all_used_integers {
            self.log_encode_all_used_integers(atol)?;
            if input_class.contains(self) {
                return Ok(());
            }
        }

        if let Some(weights) = &policy.penalty_method_with_fixed_weights {
            self.penalty_method_with_fixed_weights(weights)?;
            if input_class.contains(self) {
                return Ok(());
            }
        }

        if let Some(weight) = policy.uniform_penalty_method_with_fixed_weight {
            self.uniform_penalty_method_with_fixed_weight(weight)?;
            if input_class.contains(self) {
                return Ok(());
            }
        }

        let report = input_class.check_membership(self);
        crate::bail!("Preparation did not reach the target InstanceClass:\n{report}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        coeff, linear, Bound, Constraint, DecisionVariable, DegreeBound, Function,
        InfeasibleDetected, InstanceClassClause, Kind, OneHotConstraint, OneHotConstraintID, Sense,
        SpecialConstraintKind, VariableID,
    };
    use std::collections::BTreeSet;

    fn unconstrained_class(
        label: &str,
        allowed_variable_kinds: impl IntoIterator<Item = Kind>,
        objective_degree_bound: DegreeBound,
    ) -> InstanceClass {
        InstanceClassClause::new(
            label,
            allowed_variable_kinds.into_iter().collect(),
            objective_degree_bound,
            BTreeSet::from([Sense::Minimize]),
        )
        .into()
    }

    fn integer_inequality_instance() -> Instance {
        let variable = VariableID::from(1);
        let decision_variable = DecisionVariable::new(
            Kind::Integer,
            Bound::new(0.0, 3.0).unwrap(),
            ATol::default(),
        )
        .unwrap();
        let function = (Function::from(linear!(variable)) + coeff!(-2.0)).unwrap();
        Instance::new(
            Sense::Minimize,
            Function::from(linear!(variable)),
            BTreeMap::from([(variable, decision_variable)]),
            BTreeMap::from([(
                ConstraintID::from(1),
                Constraint::less_than_or_equal_to_zero(function),
            )]),
        )
        .unwrap()
    }

    #[test]
    fn already_member_is_exact_identity() {
        let mut instance = Instance::new(
            Sense::Minimize,
            Function::Zero,
            BTreeMap::new(),
            BTreeMap::new(),
        )
        .unwrap();
        let target = unconstrained_class("constant", [], DegreeBound::at_most(0));
        let policy = PreparationPolicy {
            penalty_method_with_fixed_weights: Some(BTreeMap::from([(
                ConstraintID::from(999),
                1.0,
            )])),
            ..Default::default()
        };
        let before = instance.clone();

        instance.prepare(&target, &policy).unwrap();

        assert_eq!(instance, before);
    }

    #[test]
    fn exact_slack_signal_is_preserved_without_approximate_fallback() {
        let mut instance = integer_inequality_instance();
        let target = InstanceClassClause::new(
            "integer equality",
            BTreeSet::from([Kind::Integer]),
            DegreeBound::at_most(1),
            BTreeSet::from([Sense::Minimize]),
        )
        .with_regular_constraint(Equality::EqualToZero, DegreeBound::at_most(1))
        .into();
        let policy = PreparationPolicy {
            convert_inequality_to_equality_with_integer_slack: Some(
                ConvertInequalityToEqualityWithIntegerSlackArguments {
                    max_integer_range: 1,
                    atol: ATol::default(),
                },
            ),
            ..Default::default()
        };
        let before = instance.clone();

        let error = instance.prepare(&target, &policy).unwrap_err();

        assert!(error.is::<ExactIntegerSlackUnavailable>());
        assert_eq!(instance, before);
    }

    #[test]
    fn configured_approximate_slack_fallback_reaches_target_membership() {
        let mut instance = integer_inequality_instance();
        let target =
            unconstrained_class("binary quadratic", [Kind::Binary], DegreeBound::at_most(2));
        let policy = PreparationPolicy {
            convert_inequality_to_equality_with_integer_slack: Some(
                ConvertInequalityToEqualityWithIntegerSlackArguments {
                    max_integer_range: 1,
                    atol: ATol::default(),
                },
            ),
            add_integer_slack_to_inequality: Some(AddIntegerSlackToInequalityArguments {
                slack_upper_bound: 2,
            }),
            log_encode_all_used_integers: Some(ATol::default()),
            uniform_penalty_method_with_fixed_weight: Some(2.0),
            ..Default::default()
        };

        instance.prepare(&target, &policy).unwrap();

        assert!(target.contains(&instance));
        assert!(instance.constraints().is_empty());
        assert!(instance.decision_variable_usage().used_integer().is_empty());
        assert!(instance
            .decision_variables()
            .keys()
            .any(|id| instance.variable_labels().name(*id) == Some("ommx.slack")));
    }

    #[test]
    fn unrelated_owner_error_preserves_earlier_commits() {
        let inequality_id = ConstraintID::from(1);
        let variables = BTreeMap::from([
            (VariableID::from(1), DecisionVariable::binary()),
            (VariableID::from(2), DecisionVariable::binary()),
        ]);
        let infeasible_function =
            Function::from(((coeff!(0.5) * linear!(1)).unwrap() + coeff!(1.0)).unwrap());
        let one_hot =
            OneHotConstraint::new(BTreeSet::from([VariableID::from(1), VariableID::from(2)]))
                .unwrap();
        let mut instance = Instance::builder()
            .sense(Sense::Maximize)
            .objective(Function::from(linear!(1)))
            .decision_variables(variables)
            .constraints(BTreeMap::from([(
                inequality_id,
                Constraint::less_than_or_equal_to_zero(infeasible_function),
            )]))
            .one_hot_constraints(BTreeMap::from([(OneHotConstraintID::from(1), one_hot)]))
            .build()
            .unwrap();
        let target =
            unconstrained_class("binary quadratic", [Kind::Binary], DegreeBound::at_most(2));
        let policy = PreparationPolicy {
            lower_special_constraints: Some(BTreeSet::from([SpecialConstraintKind::OneHot])),
            as_minimization_problem: Some(()),
            convert_inequality_to_equality_with_integer_slack: Some(
                ConvertInequalityToEqualityWithIntegerSlackArguments {
                    max_integer_range: 32,
                    atol: ATol::default(),
                },
            ),
            add_integer_slack_to_inequality: Some(AddIntegerSlackToInequalityArguments {
                slack_upper_bound: 2,
            }),
            ..Default::default()
        };

        let error = instance.prepare(&target, &policy).unwrap_err();

        assert!(!error.is::<ExactIntegerSlackUnavailable>());
        assert!(matches!(
            error.downcast_ref::<InfeasibleDetected>(),
            Some(InfeasibleDetected::InequalityConstraintBound { id, bound })
                if *id == inequality_id && *bound == Bound::new(2.0, 3.0).unwrap()
        ));
        assert_eq!(instance.sense(), Sense::Minimize);
        assert!(instance.one_hot_constraints().is_empty());
        assert_eq!(instance.removed_one_hot_constraints().len(), 1);
        assert_eq!(instance.constraints().len(), 2);
        assert!(instance.removed_constraints().is_empty());
        assert!(!target.contains(&instance));
    }

    #[test]
    fn exhausted_policy_returns_error_outside_target() {
        let variable = VariableID::from(1);
        let mut instance = Instance::new(
            Sense::Minimize,
            Function::from(linear!(variable)),
            BTreeMap::from([(variable, DecisionVariable::binary())]),
            BTreeMap::from([(
                ConstraintID::from(1),
                Constraint::equal_to_zero(Function::from(linear!(variable))),
            )]),
        )
        .unwrap();
        let target = unconstrained_class(
            "unconstrained binary",
            [Kind::Binary],
            DegreeBound::at_most(1),
        );
        let before = instance.clone();

        instance
            .prepare(&target, &PreparationPolicy::default())
            .unwrap_err();

        assert!(!target.contains(&instance));
        assert_eq!(instance, before);
    }
}
