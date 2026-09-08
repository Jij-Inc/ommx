use super::{ExactIntegerSlackUnavailable, Instance, SpecialConstraintKinds};
use crate::{
    ATol, ConstraintID, Equality, IndicatorConstraintID, InstanceClass,
    InstanceClassMembershipReport, InstanceClassMismatch, Sense,
};
use std::collections::{BTreeMap, BTreeSet};

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
        /// Absolute tolerance used when bounding tolerance-sensitive Indicator bodies.
        atol: ATol,
    },
}

/// Preparation of the active objective used by the solver-facing formulation.
///
/// The conversion itself remains owned by [`Instance::convert_active_objective`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObjectivePreparation {
    /// Target optimization sense passed to [`Instance::convert_active_objective`].
    pub target: Sense,
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
    ///
    /// Integer slack operations require this to be less than `0.5`.
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

/// Preparation that reduces powers of active Binary decision variables.
///
/// This unit phase invokes [`Instance::reduce_binary_power`], which owns the
/// corresponding output-objective update.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BinaryPowerPreparation;

/// Fixed-weight penalty preparation of active regular constraints.
///
/// Exactly one fixed-weight penalty owner operation is selected. Weight and
/// constraint-ID validation and output-objective preservation remain owned by
/// that operation.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub enum FixedPenaltyPreparation {
    /// Invoke [`Instance::penalty_method_with_fixed_weights`].
    PenaltyMethodWithFixedWeights {
        /// Constraint-ID-keyed weights passed unchanged to the owner operation.
        weights: BTreeMap<ConstraintID, f64>,
        /// Absolute tolerance passed to the owner operation for weight validation.
        atol: ATol,
    },
    /// Invoke [`Instance::uniform_penalty_method_with_fixed_weight`].
    UniformPenaltyMethodWithFixedWeight {
        /// Weight passed unchanged to the owner operation.
        weight: f64,
        /// Absolute tolerance passed to the owner operation for weight validation.
        atol: ATol,
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
/// canonical order: special constraints, active objective, Integer slack,
/// fixed penalty, Integer encoding, then Binary-power reduction. It checks
/// whole-class membership before and after each selected phase and stops as
/// soon as the target class contains the instance.
///
/// # Invariants
///
/// Every Preparation phase is disabled by default.
///
/// ```
/// use ommx::PreparationPolicy;
///
/// let policy = PreparationPolicy::default();
/// assert!(policy.special_constraints.is_none());
/// assert!(policy.objective.is_none());
/// assert!(policy.integer_slack.is_none());
/// assert!(policy.integer_encoding.is_none());
/// assert!(policy.fixed_penalty.is_none());
/// assert!(policy.binary_power_reduction.is_none());
/// ```
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PreparationPolicy {
    /// Optional special-constraint phase.
    pub special_constraints: Option<SpecialConstraintPreparation>,
    /// Optional active-objective phase.
    pub objective: Option<ObjectivePreparation>,
    /// Optional Integer slack phase. `None` does not introduce Integer slack.
    pub integer_slack: Option<IntegerSlackPreparation>,
    /// Optional used-Integer encoding phase.
    pub integer_encoding: Option<IntegerEncodingPreparation>,
    /// Optional fixed-weight penalty phase.
    pub fixed_penalty: Option<FixedPenaltyPreparation>,
    /// Optional Binary-power reduction phase.
    pub binary_power_reduction: Option<BinaryPowerPreparation>,
}

impl PreparationPolicy {
    /// Return a fresh default policy for reaching [`InstanceClass::qubo`].
    ///
    /// # Postconditions
    ///
    /// The returned QUBO policy enables every default preparation phase.
    ///
    /// ```
    /// use ommx::{
    ///     BinaryPowerPreparation, FixedPenaltyPreparation,
    ///     IntegerEncodingPreparation, ObjectivePreparation, PreparationPolicy,
    ///     Sense, SpecialConstraintPreparation,
    /// };
    ///
    /// let policy = PreparationPolicy::for_qubo();
    /// assert!(matches!(
    ///     policy.special_constraints,
    ///     Some(SpecialConstraintPreparation::LowerSpecialConstraints { .. }),
    /// ));
    /// assert_eq!(
    ///     policy.objective,
    ///     Some(ObjectivePreparation { target: Sense::Minimize }),
    /// );
    /// let slack = policy.integer_slack.unwrap();
    /// assert_eq!(slack.max_integer_range, 31);
    /// assert_eq!(slack.slack_upper_bound, Some(31));
    /// assert!(matches!(
    ///     policy.integer_encoding,
    ///     Some(IntegerEncodingPreparation::LogEncodeAllUsedIntegers { .. }),
    /// ));
    /// assert!(matches!(
    ///     policy.fixed_penalty,
    ///     Some(FixedPenaltyPreparation::UniformPenaltyMethodWithFixedWeight {
    ///         weight: 1.0,
    ///         ..
    ///     }),
    /// ));
    /// assert_eq!(policy.binary_power_reduction, Some(BinaryPowerPreparation));
    /// ```
    pub fn for_qubo() -> Self {
        Self::for_binary_polynomial_format(Some(BinaryPowerPreparation))
    }

    /// Return a fresh default policy for reaching [`InstanceClass::hubo`].
    ///
    /// # Postconditions
    ///
    /// The returned HUBO policy leaves Binary-power reduction disabled.
    ///
    /// ```
    /// use ommx::{ObjectivePreparation, PreparationPolicy, Sense};
    ///
    /// let policy = PreparationPolicy::for_hubo();
    /// assert!(policy.special_constraints.is_some());
    /// assert_eq!(
    ///     policy.objective,
    ///     Some(ObjectivePreparation { target: Sense::Minimize }),
    /// );
    /// assert!(policy.integer_slack.is_some());
    /// assert!(policy.integer_encoding.is_some());
    /// assert!(policy.fixed_penalty.is_some());
    /// assert!(policy.binary_power_reduction.is_none());
    /// ```
    pub fn for_hubo() -> Self {
        Self::for_binary_polynomial_format(None)
    }

    fn for_binary_polynomial_format(
        binary_power_reduction: Option<BinaryPowerPreparation>,
    ) -> Self {
        Self {
            special_constraints: Some(SpecialConstraintPreparation::LowerSpecialConstraints {
                kinds: [
                    super::SpecialConstraintKind::Indicator,
                    super::SpecialConstraintKind::OneHot,
                    super::SpecialConstraintKind::Sos1,
                ]
                .into_iter()
                .collect(),
                atol: ATol::default(),
            }),
            objective: Some(ObjectivePreparation {
                target: Sense::Minimize,
            }),
            integer_slack: Some(IntegerSlackPreparation {
                max_integer_range: 31,
                atol: ATol::default(),
                slack_upper_bound: Some(31),
            }),
            integer_encoding: Some(IntegerEncodingPreparation::LogEncodeAllUsedIntegers {
                atol: ATol::default(),
            }),
            fixed_penalty: Some(
                FixedPenaltyPreparation::UniformPenaltyMethodWithFixedWeight {
                    weight: 1.0,
                    atol: ATol::default(),
                },
            ),
            binary_power_reduction,
        }
    }
}

/// Signal returned when configured Preparation phases are exhausted before the
/// target [`InstanceClass`] contains the [`Instance`].
///
/// Callers can inspect [`Self::report`] to select additional Preparation
/// phases, revise the target class, or report the remaining mismatches. The
/// instance may retain changes committed by configured phases before this
/// signal is returned.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("Preparation did not reach the target InstanceClass:\n{report}{powi_expansion_hint}")]
pub struct PreparationTargetNotReached {
    report: InstanceClassMembershipReport,
    powi_expansion_hint: PowiExpansionHint,
}

impl PreparationTargetNotReached {
    /// Return the final membership report for the prepared instance.
    pub fn report(&self) -> &InstanceClassMembershipReport {
        &self.report
    }
}

/// Operation-specific recovery guidance derived from the final instance and
/// its structural membership report.
///
/// The public report remains limited to instance-class facts. Preparation owns
/// this hint because it can still inspect the functions that produced those
/// facts and suggest a caller action without changing the mismatch API.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct PowiExpansionHint {
    objective: bool,
    regular_constraint_ids: BTreeSet<ConstraintID>,
    indicator_constraint_ids: BTreeSet<IndicatorConstraintID>,
}

impl PowiExpansionHint {
    fn new(instance: &Instance, report: &InstanceClassMembershipReport) -> Self {
        let mut hint = Self::default();
        for clause in report.clause_reports() {
            for mismatch in clause.mismatches() {
                match mismatch {
                    InstanceClassMismatch::ObjectiveFunctionNotPolynomial => {
                        hint.objective |= instance.objective().contains_powi_operation();
                    }
                    InstanceClassMismatch::RegularConstraintFunctionNotPolynomial {
                        constraint_ids,
                        ..
                    } => {
                        hint.regular_constraint_ids
                            .extend(constraint_ids.iter().copied().filter(|id| {
                                instance.constraints().get(id).is_some_and(|constraint| {
                                    constraint.function().contains_powi_operation()
                                })
                            }));
                    }
                    InstanceClassMismatch::IndicatorBodyFunctionNotPolynomial {
                        constraint_ids,
                        ..
                    } => {
                        hint.indicator_constraint_ids.extend(
                            constraint_ids.iter().copied().filter(|id| {
                                instance
                                    .indicator_constraints()
                                    .get(id)
                                    .is_some_and(|constraint| {
                                        constraint.function().contains_powi_operation()
                                    })
                            }),
                        );
                    }
                    _ => {}
                }
            }
        }
        hint
    }

    fn is_empty(&self) -> bool {
        !self.objective
            && self.regular_constraint_ids.is_empty()
            && self.indicator_constraint_ids.is_empty()
    }
}

impl std::fmt::Display for PowiExpansionHint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_empty() {
            return Ok(());
        }

        let mut locations = Vec::new();
        if self.objective {
            locations.push("the objective function".to_owned());
        }
        if !self.regular_constraint_ids.is_empty() {
            locations.push(format!(
                "regular constraint functions for IDs {:?}",
                self.regular_constraint_ids
            ));
        }
        if !self.indicator_constraint_ids.is_empty() {
            locations.push(format!(
                "indicator body functions for IDs {:?}",
                self.indicator_constraint_ids
            ));
        }

        write!(
            f,
            "\nHint: Found `powi` (`**` in Python) in {}. OMMX does not expand `powi` automatically into polynomial terms. If expansion is intended, write a positive integer power of a polynomial as repeated multiplication (for example, `g * g` instead of `g ** 2`).",
            locations.join(", ")
        )
    }
}

trait PreparationStep {
    /// Apply one configured phase completely unless an owner operation fails.
    fn apply(&self, instance: &mut Instance) -> crate::Result<()>;
}

impl PreparationStep for SpecialConstraintPreparation {
    fn apply(&self, instance: &mut Instance) -> crate::Result<()> {
        match self {
            Self::LowerSpecialConstraints { kinds, atol } => {
                instance.lower_special_constraints(kinds, *atol)?;
            }
        }
        Ok(())
    }
}

impl PreparationStep for ObjectivePreparation {
    fn apply(&self, instance: &mut Instance) -> crate::Result<()> {
        instance.convert_active_objective(self.target);
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
                    instance.add_integer_slack_to_inequality(
                        id.into_inner(),
                        slack_upper_bound,
                        self.atol,
                    )?;
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
            Self::PenaltyMethodWithFixedWeights { weights, atol } => {
                instance.penalty_method_with_fixed_weights(weights, *atol)?;
            }
            Self::UniformPenaltyMethodWithFixedWeight { weight, atol } => {
                instance.uniform_penalty_method_with_fixed_weight(*weight, *atol)?;
            }
        }
        Ok(())
    }
}

impl PreparationStep for BinaryPowerPreparation {
    fn apply(&self, instance: &mut Instance) -> crate::Result<()> {
        instance.reduce_binary_power()?;
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
    /// # Postconditions
    ///
    /// Selected owner operations establish their own output semantics, and
    /// successful composition reaches the target class.
    ///
    /// ```
    /// use ommx::{
    ///     coeff, linear, v1::{Optimality, State}, ATol, Constraint, ConstraintID,
    ///     DecisionVariable, Evaluate, Function, Instance, InstanceClass,
    ///     PreparationPolicy, Sense, VariableID,
    /// };
    /// use std::collections::{BTreeMap, HashMap};
    ///
    /// let variable = VariableID::from(1);
    /// let constraint = Function::from((linear!(1) + coeff!(-1.0)).unwrap());
    /// let mut instance = Instance::builder()
    ///     .sense(Sense::Maximize)
    ///     .objective(Function::from(linear!(1)))
    ///     .decision_variables(BTreeMap::from([(variable, DecisionVariable::binary())]))
    ///     .constraints(BTreeMap::from([(
    ///         ConstraintID::from(1),
    ///         Constraint::equal_to_zero(constraint),
    ///     )]))
    ///     .build()
    ///     .unwrap();
    /// let target = InstanceClass::qubo();
    ///
    /// instance.prepare(&target, &PreparationPolicy::for_qubo()).unwrap();
    /// assert!(target.contains(&instance));
    /// assert_eq!(instance.sense(), Sense::Minimize);
    /// let state = State::from(HashMap::from([(1, 0.0)]));
    /// assert_eq!(instance.objective().evaluate(&state, ATol::default()).unwrap(), 1.0);
    /// let solution = instance.evaluate(&state, ATol::default()).unwrap();
    /// assert_eq!(*solution.sense(), Some(Sense::Maximize));
    /// assert_eq!(*solution.objective(), 0.0);
    /// assert_eq!(
    ///     instance.map_active_optimality(Optimality::Optimal),
    ///     Optimality::Unspecified,
    /// );
    /// ```
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

        if apply_preparation_step(self, input_class, policy.objective.as_ref())? {
            return Ok(());
        }

        if apply_preparation_step(self, input_class, policy.integer_slack.as_ref())? {
            return Ok(());
        }

        if apply_preparation_step(self, input_class, policy.fixed_penalty.as_ref())? {
            return Ok(());
        }

        if apply_preparation_step(self, input_class, policy.integer_encoding.as_ref())? {
            return Ok(());
        }

        if apply_preparation_step(self, input_class, policy.binary_power_reduction.as_ref())? {
            return Ok(());
        }

        let report = input_class.check_membership(self);
        let powi_expansion_hint = PowiExpansionHint::new(self, &report);
        crate::bail!(PreparationTargetNotReached {
            report,
            powi_expansion_hint,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        coeff, linear, quadratic, Bound, Constraint, DecisionVariable, DecisionVariableRole,
        Evaluate, ExactIntegerSlackUnavailable, Function, InfeasibleDetected, InstanceClassClause,
        Kind, MonomialDyn, OneHotConstraint, OneHotConstraintID, Polynomial, PolynomialRequirement,
        Sense, SpecialConstraintKind, VariableID,
    };
    use std::collections::BTreeSet;

    fn unconstrained_class(
        label: &str,
        allowed_variable_kinds: impl IntoIterator<Item = Kind>,
        objective_polynomial_requirement: PolynomialRequirement,
    ) -> InstanceClass {
        InstanceClassClause::new(
            label,
            allowed_variable_kinds.into_iter().collect(),
            objective_polynomial_requirement,
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

    fn cubic_binary_instance(ids: Vec<VariableID>) -> Instance {
        let variables = ids
            .iter()
            .copied()
            .map(|id| (id, DecisionVariable::binary()))
            .collect();
        let objective = Function::from(Polynomial::single_term(MonomialDyn::new(ids), coeff!(1.0)));
        Instance::new(Sense::Minimize, objective, variables, BTreeMap::new()).unwrap()
    }

    #[test]
    fn qubo_preparation_reduces_repeated_binary_power() {
        let variable = VariableID::from(1);
        let mut instance = cubic_binary_instance(vec![variable, variable, variable]);
        let source_objective = instance.objective().clone();

        instance
            .prepare(&InstanceClass::qubo(), &PreparationPolicy::for_qubo())
            .unwrap();

        assert!(InstanceClass::qubo().contains(&instance));
        assert_eq!(instance.objective().degree(), Some(crate::Degree::from(1)));
        assert_eq!(
            instance.objective().required_ids(),
            crate::VariableIDSet::from([variable])
        );
        let output = instance.output_objective().unwrap();
        assert_eq!(output.sense(), Sense::Minimize);
        assert_eq!(output.function(), &source_objective);
        assert!(output.preserves_optimality());
        instance.as_qubo_format().unwrap();
    }

    #[test]
    fn qubo_rejects_but_hubo_accepts_three_distinct_binary_variables() {
        let ids = vec![
            VariableID::from(1),
            VariableID::from(2),
            VariableID::from(3),
        ];
        let source = cubic_binary_instance(ids);

        let mut qubo = source.clone();
        let error = qubo
            .prepare(&InstanceClass::qubo(), &PreparationPolicy::for_qubo())
            .unwrap_err();
        assert!(error.is::<PreparationTargetNotReached>());
        assert!(!InstanceClass::qubo().contains(&qubo));
        assert!(qubo.output_objective().is_none());

        let mut hubo = source;
        hubo.prepare(&InstanceClass::hubo(), &PreparationPolicy::for_hubo())
            .unwrap();
        assert!(InstanceClass::hubo().contains(&hubo));
        assert!(hubo.output_objective().is_none());
        hubo.as_hubo_format().unwrap();
    }

    #[test]
    fn target_not_reached_explains_powi_without_mislabeling_other_operations() {
        let variable = VariableID::from(1);
        let functions = [
            (Function::from(linear!(variable)).powi(2), true),
            (Function::from(linear!(variable)).abs(), false),
        ];

        for (objective, expects_powi_hint) in functions {
            let mut instance = Instance::new(
                Sense::Minimize,
                objective,
                BTreeMap::from([(variable, DecisionVariable::binary())]),
                BTreeMap::new(),
            )
            .unwrap();

            let message = instance
                .prepare(&InstanceClass::qubo(), &PreparationPolicy::default())
                .unwrap_err()
                .to_string();

            assert!(message.contains(
                "objective function is not stored in the expanded polynomial form required by this clause"
            ));
            assert_eq!(
                message.contains("Found `powi` (`**` in Python) in the objective function"),
                expects_powi_hint
            );
            assert_eq!(
                message.contains("`g * g` instead of `g ** 2`"),
                expects_powi_hint
            );
        }
    }

    #[test]
    fn qubo_preparation_composes_encoding_and_penalty_output_semantics() {
        let variable = VariableID::from(1);
        let constraint_id = ConstraintID::from(7);
        let objective = Function::from(linear!(variable));
        let constraint_function = (Function::from(linear!(variable)) + coeff!(-1.0)).unwrap();
        let mut instance = Instance::new(
            Sense::Minimize,
            objective.clone(),
            BTreeMap::from([(
                variable,
                DecisionVariable::new(
                    Kind::Integer,
                    Bound::new(0.0, 3.0).unwrap(),
                    ATol::default(),
                )
                .unwrap(),
            )]),
            BTreeMap::from([(
                constraint_id,
                Constraint::equal_to_zero(constraint_function.clone()),
            )]),
        )
        .unwrap();

        instance
            .prepare(&InstanceClass::qubo(), &PreparationPolicy::for_qubo())
            .unwrap();

        assert!(InstanceClass::qubo().contains(&instance));
        let output = instance.output_objective().unwrap();
        assert_eq!(output.sense(), Sense::Minimize);
        assert_eq!(output.function(), &objective);
        assert!(!output.preserves_optimality());
        assert_eq!(
            instance.removed_constraints()[&constraint_id].0.function(),
            &constraint_function
        );
        assert_eq!(
            instance.decision_variable_role(variable),
            Some(DecisionVariableRole::Dependent)
        );
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
        let target = unconstrained_class("constant", [], PolynomialRequirement::at_most(0));
        let policy = PreparationPolicy {
            fixed_penalty: Some(FixedPenaltyPreparation::PenaltyMethodWithFixedWeights {
                weights: BTreeMap::from([(ConstraintID::from(999), 1.0)]),
                atol: ATol::default(),
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
            PolynomialRequirement::at_most(1),
            BTreeSet::from([Sense::Minimize]),
        )
        .with_regular_constraint(Equality::EqualToZero, PolynomialRequirement::at_most(1))
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
            PolynomialRequirement::at_most(1),
            BTreeSet::from([Sense::Minimize]),
        )
        .with_regular_constraint(Equality::EqualToZero, PolynomialRequirement::at_most(1))
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
            PolynomialRequirement::at_most(1),
            BTreeSet::from([Sense::Minimize]),
        )
        .with_regular_constraint(Equality::EqualToZero, PolynomialRequirement::at_most(1))
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
            PolynomialRequirement::at_most(0),
            BTreeSet::from([Sense::Minimize]),
        )
        .with_regular_constraint(
            Equality::LessThanOrEqualToZero,
            PolynomialRequirement::at_most(1),
        )
        .into();
        let policy = PreparationPolicy {
            integer_slack: Some(IntegerSlackPreparation {
                max_integer_range: 1,
                atol: ATol::default(),
                slack_upper_bound: Some(2),
            }),
            fixed_penalty: Some(FixedPenaltyPreparation::PenaltyMethodWithFixedWeights {
                weights: BTreeMap::from([(ConstraintID::from(999), 1.0)]),
                atol: ATol::default(),
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
        let target = unconstrained_class(
            "binary quadratic",
            [Kind::Binary],
            PolynomialRequirement::at_most(2),
        );
        let policy = PreparationPolicy {
            special_constraints: Some(SpecialConstraintPreparation::LowerSpecialConstraints {
                kinds: BTreeSet::from([SpecialConstraintKind::OneHot]),
                atol: ATol::default(),
            }),
            objective: Some(ObjectivePreparation {
                target: Sense::Minimize,
            }),
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
                if *id == infeasible_id && *bound == Bound::new(1.0, 1.5).unwrap()
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
            PolynomialRequirement::at_most(1),
        );
        let policy = PreparationPolicy {
            objective: Some(ObjectivePreparation {
                target: Sense::Minimize,
            }),
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
