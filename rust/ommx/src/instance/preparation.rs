use super::{ExactIntegerSlackUnavailable, Instance, SpecialConstraintKinds};
use crate::{ATol, ConstraintID, Equality, InstanceClass, InstanceClassMembershipReport};
use std::collections::BTreeMap;

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
/// For every active regular inequality, Preparation first invokes
/// [`Instance::convert_inequality_to_equality_with_integer_slack`] using
/// [`Self::max_integer_range`] and [`Self::atol`].
///
/// [`Self::slack_upper_bound`] selects the required postcondition. `None`
/// requires the constraint to become an equality or be removed as trivially
/// satisfied, so [`ExactIntegerSlackUnavailable`] is propagated. `Some(value)`
/// permits the constraint to remain an inequality: only that signal selects
/// [`Instance::add_integer_slack_to_inequality`] with `value`. The latter
/// operation preserves the feasible assignments of the original variables
/// exactly; it is not an approximation. Every other owner error is propagated
/// unchanged.
///
/// Numeric and Instance-dependent validation remains owned by the corresponding
/// [`Instance`] operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IntegerSlackPreparation {
    /// Maximum finite range accepted for an exact Integer slack variable.
    pub max_integer_range: u64,
    /// Absolute tolerance used to normalize bounds to integers.
    pub atol: ATol,
    /// Optional upper bound for inequality-preserving Integer slack.
    ///
    /// `None` requires equality. `Some(value)` passes `value` unchanged to
    /// [`Instance::add_integer_slack_to_inequality`] only when exact conversion
    /// returns [`ExactIntegerSlackUnavailable`].
    pub slack_upper_bound: Option<u64>,
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
///     ATol, FixedPenaltyPreparation, IntegerSlackPreparation, PreparationPolicy,
///     SensePreparation,
/// };
///
/// let mut policy = PreparationPolicy::default();
/// policy.sense = Some(SensePreparation::AsMinimizationProblem);
/// policy.integer_slack = Some(IntegerSlackPreparation {
///     max_integer_range: 32,
///     atol: ATol::default(),
///     slack_upper_bound: None,
/// });
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
    /// Optional Integer slack phase. `None` does not introduce Integer slack.
    pub integer_slack: Option<IntegerSlackPreparation>,
    /// Optional used-Integer encoding phase.
    pub integer_encoding: Option<IntegerEncodingPreparation>,
    /// Optional fixed-weight penalty phase.
    pub fixed_penalty: Option<FixedPenaltyPreparation>,
}

/// Signal returned when configured Preparation phases are exhausted before the
/// target [`InstanceClass`] contains the [`Instance`].
///
/// Callers can inspect [`Self::report`] to select additional Preparation
/// phases, revise the target class, or report the remaining mismatches. The
/// instance may retain changes committed by configured phases before this
/// signal is returned.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("Preparation did not reach the target InstanceClass:\n{report}")]
pub struct PreparationTargetNotReached {
    report: InstanceClassMembershipReport,
}

impl PreparationTargetNotReached {
    /// Return the final membership report for the prepared instance.
    pub fn report(&self) -> &InstanceClassMembershipReport {
        &self.report
    }
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
            match instance.convert_inequality_to_equality_with_integer_slack(
                id.into_inner(),
                self.max_integer_range,
                self.atol,
            ) {
                Ok(()) => {}
                Err(error) if error.is::<ExactIntegerSlackUnavailable>() => {
                    let Some(slack_upper_bound) = self.slack_upper_bound else {
                        return Err(error);
                    };
                    instance.add_integer_slack_to_inequality(id.into_inner(), slack_upper_bound)?;
                }
                Err(error) => return Err(error),
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
    /// order. Integer slack Preparation first attempts to convert them to
    /// equalities. [`IntegerSlackPreparation::slack_upper_bound`] set to `None`
    /// requires equality and propagates [`ExactIntegerSlackUnavailable`].
    /// `Some(value)` selects the inequality-preserving owner operation with
    /// `value` only for that signal. Every other owner error is propagated
    /// unchanged.
    ///
    /// Preparation has no global transaction. Each owner operation retains its
    /// own mutation and failure semantics, and changes committed by earlier
    /// operations remain when a later operation fails.
    ///
    /// # Errors
    ///
    /// Returns an error from a configured owner operation, or
    /// [`PreparationTargetNotReached`] with the final membership report when
    /// the configured operations are exhausted without reaching
    /// `input_class`.
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
        crate::bail!(PreparationTargetNotReached { report })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        coeff, linear, quadratic, Bound, Constraint, DecisionVariable, DegreeBound,
        ExactIntegerSlackUnavailable, Function, InfeasibleDetected, InstanceClassClause, Kind,
        OneHotConstraint, OneHotConstraintID, Sense, SpecialConstraintKind, VariableID,
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
    fn equality_requirement_propagates_exact_slack_unavailable_without_an_alternative() {
        let mut instance = integer_inequality_instance();
        let before = instance.clone();
        let target = InstanceClassClause::new(
            "integer equality",
            BTreeSet::from([Kind::Integer]),
            DegreeBound::at_most(1),
            BTreeSet::from([Sense::Minimize]),
        )
        .with_regular_constraint(Equality::EqualToZero, DegreeBound::at_most(1))
        .into();
        let policy = PreparationPolicy {
            integer_slack: Some(IntegerSlackPreparation {
                max_integer_range: 1,
                atol: ATol::default(),
                slack_upper_bound: None,
            }),
            ..Default::default()
        };
        let error = instance.prepare(&target, &policy).unwrap_err();

        assert!(error.is::<ExactIntegerSlackUnavailable>());
        assert_eq!(instance, before);
    }

    #[test]
    fn equality_requirement_reaches_an_equality_target() {
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
            integer_slack: Some(IntegerSlackPreparation {
                max_integer_range: 32,
                atol: ATol::default(),
                slack_upper_bound: None,
            }),
            ..Default::default()
        };

        instance.prepare(&target, &policy).unwrap();

        assert!(target.contains(&instance));
    }

    #[test]
    fn allowing_inequality_still_prefers_an_equality_when_available() {
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
            integer_slack: Some(IntegerSlackPreparation {
                max_integer_range: 32,
                atol: ATol::default(),
                slack_upper_bound: Some(2),
            }),
            ..Default::default()
        };

        instance.prepare(&target, &policy).unwrap();

        assert!(target.contains(&instance));
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
            integer_slack: Some(IntegerSlackPreparation {
                max_integer_range: 1,
                atol: ATol::default(),
                slack_upper_bound: Some(2),
            }),
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
    fn allowing_inequality_propagates_unrelated_error_after_earlier_commits() {
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
            integer_slack: Some(IntegerSlackPreparation {
                max_integer_range: 32,
                atol: ATol::default(),
                slack_upper_bound: Some(2),
            }),
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
    fn exhausted_policy_returns_the_final_membership_report() {
        let variable = VariableID::from(1);
        let mut instance = Instance::new(
            Sense::Maximize,
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
        let policy = PreparationPolicy {
            sense: Some(SensePreparation::AsMinimizationProblem),
            ..Default::default()
        };

        let error = instance.prepare(&target, &policy).unwrap_err();
        let membership_report = target.check_membership(&instance);

        assert!(!target.contains(&instance));
        assert_eq!(instance.sense(), Sense::Minimize);
        assert_eq!(
            error
                .downcast_ref::<PreparationTargetNotReached>()
                .unwrap()
                .report(),
            &membership_report
        );
    }
}
