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

/// Preparation of active special constraints.
///
/// Each variant selects one existing [`Instance`] owner operation and retains
/// its argument names and meanings.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpecialConstraintPreparation {
    /// Invoke [`Instance::lower_special_constraints`].
    LowerSpecialConstraints {
        /// Special-constraint kinds passed to the owner operation.
        kinds: SpecialConstraintKinds,
    },
}

/// Preparation of the optimization sense.
///
/// Each variant selects one existing [`Instance`] owner operation.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SensePreparation {
    /// Invoke [`Instance::as_minimization_problem`].
    AsMinimizationProblem,
}

/// Preparation that introduces Integer slack variables into active regular
/// inequalities.
///
/// Exactly one primary owner operation is selected. Exact conversion may carry
/// an approximate fallback, but that fallback is invoked only for
/// [`ExactIntegerSlackUnavailable`]. Every other owner error is propagated
/// unchanged. Numeric and Instance-dependent validation remains owned by the
/// selected [`Instance`] operations.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntegerSlackPreparation {
    /// Invoke
    /// [`Instance::convert_inequality_to_equality_with_integer_slack`] for
    /// every active regular inequality.
    ConvertInequalityToEqualityWithIntegerSlack {
        /// Arguments passed to exact Integer slack conversion.
        arguments: ConvertInequalityToEqualityWithIntegerSlackArguments,
        /// Arguments passed to [`Instance::add_integer_slack_to_inequality`]
        /// only when exact conversion returns
        /// [`ExactIntegerSlackUnavailable`].
        on_exact_integer_slack_unavailable: Option<AddIntegerSlackToInequalityArguments>,
    },
    /// Invoke [`Instance::add_integer_slack_to_inequality`] directly for every
    /// active regular inequality.
    AddIntegerSlackToInequality(AddIntegerSlackToInequalityArguments),
}

/// Preparation of currently used Integer decision variables.
///
/// Exactly one encoding owner operation is selected. Validation and mutation
/// semantics remain owned by that operation.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntegerEncodingPreparation {
    /// Invoke [`Instance::log_encode_all_used_integers`].
    LogEncodeAllUsedIntegers {
        /// Absolute tolerance passed to the owner operation.
        atol: ATol,
    },
}

/// Fixed-weight penalty preparation of active regular constraints.
///
/// Exactly one fixed-weight penalty owner operation is selected. Weight and
/// constraint-ID validation remains owned by that operation.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub enum FixedPenaltyPreparation {
    /// Invoke [`Instance::penalty_method_with_fixed_weights`].
    PenaltyMethodWithFixedWeights {
        /// Constraint-ID-keyed weights passed unchanged to the owner operation.
        weights: BTreeMap<ConstraintID, f64>,
    },
    /// Invoke [`Instance::uniform_penalty_method_with_fixed_weight`].
    UniformPenaltyMethodWithFixedWeight {
        /// Weight passed unchanged to the owner operation.
        weight: f64,
    },
}

/// Optional Preparation phases interpreted by [`Instance::prepare`].
///
/// Each public field independently selects at most one well-formed phase.
/// Fields may be combined freely, but not every combination is guaranteed to
/// succeed for every [`Instance`] and target [`InstanceClass`]. Owner-operation
/// validation is not duplicated by this table.
///
/// This struct is non-exhaustive because additional Preparation phases may be
/// added in future releases. Downstream callers should construct it with
/// [`PreparationPolicy::default`] and assign the desired public fields. Every
/// phase is disabled by default, including fields added in future releases.
///
/// [`Instance::prepare`] applies each selected phase at most once in this
/// canonical order: special constraints, optimization sense, Integer slack,
/// Integer encoding, then fixed penalty. It checks whole-class membership
/// before and after each selected phase and stops as soon as the target class
/// contains the instance.
///
/// # Examples
///
/// ```
/// use ommx::{
///     AddIntegerSlackToInequalityArguments, ATol,
///     ConvertInequalityToEqualityWithIntegerSlackArguments, FixedPenaltyPreparation,
///     IntegerSlackPreparation, PreparationPolicy, SensePreparation,
/// };
///
/// let mut policy = PreparationPolicy::default();
/// policy.sense = Some(SensePreparation::AsMinimizationProblem);
/// policy.integer_slack = Some(
///     IntegerSlackPreparation::ConvertInequalityToEqualityWithIntegerSlack {
///         arguments: ConvertInequalityToEqualityWithIntegerSlackArguments {
///             max_integer_range: 32,
///             atol: ATol::default(),
///         },
///         on_exact_integer_slack_unavailable: Some(
///             AddIntegerSlackToInequalityArguments {
///                 slack_upper_bound: 32,
///             },
///         ),
///     },
/// );
/// policy.fixed_penalty = Some(
///     FixedPenaltyPreparation::UniformPenaltyMethodWithFixedWeight { weight: 2.0 },
/// );
/// ```
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PreparationPolicy {
    /// Optional special-constraint phase.
    pub special_constraints: Option<SpecialConstraintPreparation>,
    /// Optional optimization-sense phase.
    pub sense: Option<SensePreparation>,
    /// Optional Integer slack phase.
    pub integer_slack: Option<IntegerSlackPreparation>,
    /// Optional used-Integer encoding phase.
    pub integer_encoding: Option<IntegerEncodingPreparation>,
    /// Optional fixed-weight penalty phase.
    pub fixed_penalty: Option<FixedPenaltyPreparation>,
}

trait PreparationStep {
    /// Apply one configured phase completely unless an owner operation fails.
    fn apply(&self, instance: &mut Instance) -> crate::Result<()>;
}

impl PreparationStep for SpecialConstraintPreparation {
    fn apply(&self, instance: &mut Instance) -> crate::Result<()> {
        match self {
            Self::LowerSpecialConstraints { kinds } => {
                instance.lower_special_constraints(kinds)?;
            }
        }
        Ok(())
    }
}

impl PreparationStep for SensePreparation {
    fn apply(&self, instance: &mut Instance) -> crate::Result<()> {
        match self {
            Self::AsMinimizationProblem => {
                instance.as_minimization_problem();
            }
        }
        Ok(())
    }
}

impl PreparationStep for IntegerSlackPreparation {
    fn apply(&self, instance: &mut Instance) -> crate::Result<()> {
        let inequality_ids = instance
            .constraints()
            .iter()
            .filter_map(|(&id, constraint)| {
                (constraint.equality == Equality::LessThanOrEqualToZero).then_some(id)
            })
            .collect::<Vec<_>>();

        for id in inequality_ids {
            match self {
                Self::ConvertInequalityToEqualityWithIntegerSlack {
                    arguments,
                    on_exact_integer_slack_unavailable,
                } => {
                    match instance.convert_inequality_to_equality_with_integer_slack(
                        id.into_inner(),
                        arguments.max_integer_range,
                        arguments.atol,
                    ) {
                        Ok(()) => {}
                        Err(error) if error.is::<ExactIntegerSlackUnavailable>() => {
                            let Some(arguments) = on_exact_integer_slack_unavailable else {
                                return Err(error);
                            };
                            instance.add_integer_slack_to_inequality(
                                id.into_inner(),
                                arguments.slack_upper_bound,
                            )?;
                        }
                        Err(error) => return Err(error),
                    }
                }
                Self::AddIntegerSlackToInequality(arguments) => {
                    instance.add_integer_slack_to_inequality(
                        id.into_inner(),
                        arguments.slack_upper_bound,
                    )?;
                }
            }
        }
        Ok(())
    }
}

impl PreparationStep for IntegerEncodingPreparation {
    fn apply(&self, instance: &mut Instance) -> crate::Result<()> {
        match self {
            Self::LogEncodeAllUsedIntegers { atol } => {
                instance.log_encode_all_used_integers(*atol)?;
            }
        }
        Ok(())
    }
}

impl PreparationStep for FixedPenaltyPreparation {
    fn apply(&self, instance: &mut Instance) -> crate::Result<()> {
        match self {
            Self::PenaltyMethodWithFixedWeights { weights } => {
                instance.penalty_method_with_fixed_weights(weights)?;
            }
            Self::UniformPenaltyMethodWithFixedWeight { weight } => {
                instance.uniform_penalty_method_with_fixed_weight(*weight)?;
            }
        }
        Ok(())
    }
}

fn apply_preparation_step<S: PreparationStep>(
    instance: &mut Instance,
    input_class: &InstanceClass,
    step: Option<&S>,
) -> crate::Result<bool> {
    match step {
        Some(step) => {
            step.apply(instance)?;
            Ok(input_class.contains(instance))
        }
        None => Ok(false),
    }
}

impl Instance {
    /// Prepare this instance for membership in `input_class` using configured
    /// [`Instance`] owner operations.
    ///
    /// This is a partial, in-place interpreter. It applies each configured
    /// phase at most once in the canonical order documented by
    /// [`PreparationPolicy`], checks whole-class membership before and after
    /// each selected phase, and returns as soon as `input_class.contains(self)`
    /// is true. Success therefore guarantees only membership in `input_class`;
    /// adapter-specific applicability remains outside this operation.
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

        if apply_preparation_step(self, input_class, policy.special_constraints.as_ref())? {
            return Ok(());
        }

        if apply_preparation_step(self, input_class, policy.sense.as_ref())? {
            return Ok(());
        }

        if apply_preparation_step(self, input_class, policy.integer_slack.as_ref())? {
            return Ok(());
        }

        if apply_preparation_step(self, input_class, policy.integer_encoding.as_ref())? {
            return Ok(());
        }

        if apply_preparation_step(self, input_class, policy.fixed_penalty.as_ref())? {
            return Ok(());
        }

        let report = input_class.check_membership(self);
        crate::bail!("Preparation did not reach the target InstanceClass:\n{report}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        coeff, linear, quadratic, Bound, Constraint, DecisionVariable, DegreeBound, Function,
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
            fixed_penalty: Some(FixedPenaltyPreparation::PenaltyMethodWithFixedWeights {
                weights: BTreeMap::from([(ConstraintID::from(999), 1.0)]),
            }),
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
            integer_slack: Some(
                IntegerSlackPreparation::ConvertInequalityToEqualityWithIntegerSlack {
                    arguments: ConvertInequalityToEqualityWithIntegerSlackArguments {
                        max_integer_range: 1,
                        atol: ATol::default(),
                    },
                    on_exact_integer_slack_unavailable: None,
                },
            ),
            ..Default::default()
        };
        let error = instance.prepare(&target, &policy).unwrap_err();

        assert!(error.is::<ExactIntegerSlackUnavailable>());
    }

    #[test]
    fn configured_approximate_slack_fallback_reaches_target_membership() {
        let mut instance = integer_inequality_instance();
        let target =
            unconstrained_class("binary quadratic", [Kind::Binary], DegreeBound::at_most(2));
        let policy = PreparationPolicy {
            integer_slack: Some(
                IntegerSlackPreparation::ConvertInequalityToEqualityWithIntegerSlack {
                    arguments: ConvertInequalityToEqualityWithIntegerSlackArguments {
                        max_integer_range: 1,
                        atol: ATol::default(),
                    },
                    on_exact_integer_slack_unavailable: Some(
                        AddIntegerSlackToInequalityArguments {
                            slack_upper_bound: 2,
                        },
                    ),
                },
            ),
            integer_encoding: Some(IntegerEncodingPreparation::LogEncodeAllUsedIntegers {
                atol: ATol::default(),
            }),
            fixed_penalty: Some(
                FixedPenaltyPreparation::UniformPenaltyMethodWithFixedWeight { weight: 2.0 },
            ),
            ..Default::default()
        };

        instance.prepare(&target, &policy).unwrap();

        assert!(target.contains(&instance));
        assert!(instance
            .decision_variables()
            .keys()
            .any(|id| instance.variable_labels().name(*id) == Some("ommx.slack")));
    }

    #[test]
    fn integer_slack_phase_completes_before_membership_stops_later_phases() {
        let variable = VariableID::from(1);
        let first_id = ConstraintID::from(1);
        let second_id = ConstraintID::from(2);
        let quadratic_inequality =
            Function::Quadratic((quadratic!(variable, variable) + coeff!(-10.0)).unwrap());
        let linear_inequality = (Function::from(linear!(variable)) + coeff!(-2.0)).unwrap();
        let mut instance = Instance::new(
            Sense::Minimize,
            Function::Zero,
            BTreeMap::from([(
                variable,
                DecisionVariable::new(
                    Kind::Integer,
                    Bound::new(0.0, 3.0).unwrap(),
                    ATol::default(),
                )
                .unwrap(),
            )]),
            BTreeMap::from([
                (
                    first_id,
                    Constraint::less_than_or_equal_to_zero(quadratic_inequality),
                ),
                (
                    second_id,
                    Constraint::less_than_or_equal_to_zero(linear_inequality),
                ),
            ]),
        )
        .unwrap();
        let target: InstanceClass = InstanceClassClause::new(
            "linear integer inequalities",
            BTreeSet::from([Kind::Integer]),
            DegreeBound::at_most(0),
            BTreeSet::from([Sense::Minimize]),
        )
        .with_regular_constraint(Equality::LessThanOrEqualToZero, DegreeBound::at_most(1))
        .into();
        let policy = PreparationPolicy {
            integer_slack: Some(IntegerSlackPreparation::AddIntegerSlackToInequality(
                AddIntegerSlackToInequalityArguments {
                    slack_upper_bound: 2,
                },
            )),
            fixed_penalty: Some(FixedPenaltyPreparation::PenaltyMethodWithFixedWeights {
                weights: BTreeMap::from([(ConstraintID::from(999), 1.0)]),
            }),
            ..Default::default()
        };
        let variable_count = instance.decision_variables().len();

        assert!(!target.contains(&instance));

        instance.prepare(&target, &policy).unwrap();

        assert!(target.contains(&instance));
        assert_eq!(instance.decision_variables().len(), variable_count + 1);
    }

    #[test]
    fn unrelated_owner_error_preserves_earlier_commits() {
        let converted_id = ConstraintID::from(1);
        let infeasible_id = ConstraintID::from(2);
        let variables = BTreeMap::from([
            (VariableID::from(1), DecisionVariable::binary()),
            (VariableID::from(2), DecisionVariable::binary()),
        ]);
        let converted_function = (Function::from(linear!(1)) + coeff!(-0.5)).unwrap();
        let infeasible_function =
            Function::from(((coeff!(0.5) * linear!(1)).unwrap() + coeff!(1.0)).unwrap());
        let one_hot =
            OneHotConstraint::new(BTreeSet::from([VariableID::from(1), VariableID::from(2)]))
                .unwrap();
        let mut instance = Instance::builder()
            .sense(Sense::Maximize)
            .objective(Function::from(linear!(1)))
            .decision_variables(variables)
            .constraints(BTreeMap::from([
                (
                    converted_id,
                    Constraint::less_than_or_equal_to_zero(converted_function),
                ),
                (
                    infeasible_id,
                    Constraint::less_than_or_equal_to_zero(infeasible_function),
                ),
            ]))
            .one_hot_constraints(BTreeMap::from([(OneHotConstraintID::from(1), one_hot)]))
            .build()
            .unwrap();
        let target =
            unconstrained_class("binary quadratic", [Kind::Binary], DegreeBound::at_most(2));
        let policy = PreparationPolicy {
            special_constraints: Some(SpecialConstraintPreparation::LowerSpecialConstraints {
                kinds: BTreeSet::from([SpecialConstraintKind::OneHot]),
            }),
            sense: Some(SensePreparation::AsMinimizationProblem),
            integer_slack: Some(
                IntegerSlackPreparation::ConvertInequalityToEqualityWithIntegerSlack {
                    arguments: ConvertInequalityToEqualityWithIntegerSlackArguments {
                        max_integer_range: 32,
                        atol: ATol::default(),
                    },
                    on_exact_integer_slack_unavailable: Some(
                        AddIntegerSlackToInequalityArguments {
                            slack_upper_bound: 2,
                        },
                    ),
                },
            ),
            ..Default::default()
        };

        let error = instance.prepare(&target, &policy).unwrap_err();

        assert!(matches!(
            error.downcast_ref::<InfeasibleDetected>(),
            Some(InfeasibleDetected::InequalityConstraintBound { id, bound })
                if *id == infeasible_id && *bound == Bound::new(2.0, 3.0).unwrap()
        ));
        assert_eq!(instance.sense(), Sense::Minimize);
        assert_eq!(instance.removed_one_hot_constraints().len(), 1);
        assert_eq!(
            instance.constraints()[&converted_id].equality,
            Equality::EqualToZero
        );
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
        let membership_report = target.check_membership(&instance).to_string();

        let error = instance
            .prepare(&target, &PreparationPolicy::default())
            .unwrap_err();

        assert!(!target.contains(&instance));
        assert_eq!(instance, before);
        assert!(error.to_string().contains(&membership_report));
    }
}
