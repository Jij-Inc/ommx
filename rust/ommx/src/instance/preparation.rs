//! Policy-driven construction of [`InstanceClass`] members.
//!
//! Preparation is a thin interpreter over existing [`Instance`] operations.
//! The prepared [`Instance`] itself retains everything needed to evaluate
//! solver states and samples.

use super::{Instance, SpecialConstraintKinds};
use crate::{ATol, ConstraintID, Equality, InstanceClass};
use std::collections::BTreeMap;

/// Arguments for existing [`Instance`] operations used by [`Instance::prepare`].
///
/// Each `Option` field stores the arguments for one existing [`Instance`]
/// operation. `None` means that operation is not invoked by preparation.
/// `as_minimization_problem` selects its no-argument operation with a Boolean.
/// The target [`InstanceClass`] is supplied separately to
/// [`Instance::prepare`].
#[derive(Debug, Clone)]
pub struct PreparationPolicy {
    lower_special_constraints: Option<SpecialConstraintKinds>,
    log_encode_used_integers: Option<ATol>,
    as_minimization_problem: bool,
    convert_inequality_to_equality_with_integer_slack: Option<(u64, ATol)>,
    add_integer_slack_to_inequality: Option<u64>,
    uniform_penalty_method_with_weight: Option<f64>,
    penalty_method_with_weights: Option<BTreeMap<ConstraintID, f64>>,
}

impl PreparationPolicy {
    /// Store the optional arguments for the existing [`Instance`] operations
    /// interpreted by [`Instance::prepare`].
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        lower_special_constraints: Option<SpecialConstraintKinds>,
        log_encode_used_integers: Option<ATol>,
        as_minimization_problem: bool,
        convert_inequality_to_equality_with_integer_slack: Option<(u64, ATol)>,
        add_integer_slack_to_inequality: Option<u64>,
        uniform_penalty_method_with_weight: Option<f64>,
        penalty_method_with_weights: Option<BTreeMap<ConstraintID, f64>>,
    ) -> Self {
        Self {
            lower_special_constraints,
            log_encode_used_integers,
            as_minimization_problem,
            convert_inequality_to_equality_with_integer_slack,
            add_integer_slack_to_inequality,
            uniform_penalty_method_with_weight,
            penalty_method_with_weights,
        }
    }

    /// Arguments for [`Instance::lower_special_constraints`].
    pub fn lower_special_constraints(&self) -> Option<&SpecialConstraintKinds> {
        self.lower_special_constraints.as_ref()
    }

    /// Argument for [`Instance::log_encode_used_integers`].
    pub fn log_encode_used_integers(&self) -> Option<ATol> {
        self.log_encode_used_integers
    }

    /// Whether [`Instance::as_minimization_problem`] is invoked.
    pub fn as_minimization_problem(&self) -> bool {
        self.as_minimization_problem
    }

    /// Arguments after `constraint_id` for
    /// [`Instance::convert_inequality_to_equality_with_integer_slack`].
    pub fn convert_inequality_to_equality_with_integer_slack(&self) -> Option<(u64, ATol)> {
        self.convert_inequality_to_equality_with_integer_slack
    }

    /// Argument after `constraint_id` for
    /// [`Instance::add_integer_slack_to_inequality`].
    pub fn add_integer_slack_to_inequality(&self) -> Option<u64> {
        self.add_integer_slack_to_inequality
    }

    /// Argument for [`Instance::uniform_penalty_method_with_weight`].
    pub fn uniform_penalty_method_with_weight(&self) -> Option<f64> {
        self.uniform_penalty_method_with_weight
    }

    /// Arguments for [`Instance::penalty_method_with_weights`].
    pub fn penalty_method_with_weights(&self) -> Option<&BTreeMap<ConstraintID, f64>> {
        self.penalty_method_with_weights.as_ref()
    }
}

impl Instance {
    /// Prepare this Instance in place to satisfy `input_class`.
    ///
    /// Preparation invokes the configured [`Instance`] operations in a fixed
    /// order until `input_class.contains(self)` is true. It does not evaluate
    /// Adapter-owned preconditions or add a composite mathematical guarantee
    /// beyond the semantics of the invoked operations.
    ///
    /// This method is not transactional across transformations. If it returns
    /// an error, changes made by earlier transformations remain applied.
    #[tracing::instrument(skip_all)]
    pub fn prepare(
        &mut self,
        input_class: &InstanceClass,
        policy: &PreparationPolicy,
    ) -> crate::Result<()> {
        macro_rules! return_if_in_input_class {
            () => {
                if input_class.contains(self) {
                    return Ok(());
                }
            };
        }

        return_if_in_input_class!();

        if let Some(kinds) = &policy.lower_special_constraints {
            self.lower_special_constraints(kinds)?;
            return_if_in_input_class!();
        }

        if let Some(atol) = policy.log_encode_used_integers {
            self.log_encode_used_integers(atol)?;
            return_if_in_input_class!();
        }

        if policy.as_minimization_problem {
            self.as_minimization_problem();
            return_if_in_input_class!();
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

            for constraint_id in inequality_ids {
                match policy.convert_inequality_to_equality_with_integer_slack {
                    Some((max_integer_range, atol)) => {
                        match self.convert_inequality_to_equality_with_integer_slack(
                            constraint_id.into_inner(),
                            max_integer_range,
                            atol,
                        ) {
                            Ok(()) => {}
                            Err(error)
                                if error.is::<crate::ExactIntegerSlackUnavailable>()
                                    && policy.add_integer_slack_to_inequality.is_some() =>
                            {
                                self.add_integer_slack_to_inequality(
                                    constraint_id.into_inner(),
                                    policy
                                        .add_integer_slack_to_inequality
                                        .expect("presence was checked above"),
                                )?;
                            }
                            Err(error) => return Err(error),
                        }
                    }
                    None => {
                        if let Some(slack_upper_bound) = policy.add_integer_slack_to_inequality {
                            self.add_integer_slack_to_inequality(
                                constraint_id.into_inner(),
                                slack_upper_bound,
                            )?;
                        }
                    }
                }
            }
            return_if_in_input_class!();
        }

        if let Some(weight) = policy.uniform_penalty_method_with_weight {
            self.uniform_penalty_method_with_weight(weight)?;
            return_if_in_input_class!();
        }

        if let Some(weights) = policy.penalty_method_with_weights.as_ref() {
            self.penalty_method_with_weights(weights)?;
            return_if_in_input_class!();
        }

        if let Some(atol) = policy.log_encode_used_integers {
            self.log_encode_used_integers(atol)?;
            return_if_in_input_class!();
        }

        let membership = input_class.check_membership(self);
        crate::bail!("Preparation did not reach the input class:\n{membership}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        coeff, linear, v1, Bound, Constraint, DecisionVariable, DegreeBound, Equality, Evaluate,
        Function, IndicatorConstraint, IndicatorConstraintID, InstanceClassClause, Kind,
        OneHotConstraint, OneHotConstraintID, Sense, Sos1Constraint, Sos1ConstraintID,
        SpecialConstraintKind, VariableID,
    };
    use std::collections::{BTreeSet, HashMap};

    fn highs_class() -> InstanceClass {
        InstanceClass::new(vec![InstanceClassClause::new(
            "linear-mip",
            BTreeSet::from([Kind::Binary, Kind::Integer, Kind::Continuous]),
            DegreeBound::at_most(1),
            BTreeSet::from([Sense::Minimize, Sense::Maximize]),
        )
        .with_regular_constraint(Equality::EqualToZero, DegreeBound::at_most(1))
        .with_regular_constraint(
            Equality::LessThanOrEqualToZero,
            DegreeBound::at_most(1),
        )])
    }

    fn openjij_class() -> InstanceClass {
        InstanceClass::new(vec![InstanceClassClause::new(
            "binary-quadratic-unconstrained-minimization",
            BTreeSet::from([Kind::Binary]),
            DegreeBound::at_most(2),
            BTreeSet::from([Sense::Minimize]),
        )])
    }

    fn zero_state_for(instance: &Instance) -> v1::State {
        v1::State::from(
            instance
                .used_decision_variable_ids()
                .into_iter()
                .map(|id| (id.into_inner(), 0.0))
                .collect::<HashMap<_, _>>(),
        )
    }

    fn integer_inequality_instance() -> Instance {
        let x = VariableID::from(1);
        Instance::builder()
            .sense(Sense::Maximize)
            .objective(Function::from(linear!(x)))
            .decision_variables(BTreeMap::from([(
                x,
                DecisionVariable::new(
                    Kind::Integer,
                    Bound::new(0.0, 3.0).unwrap(),
                    ATol::default(),
                )
                .unwrap(),
            )]))
            .constraints(BTreeMap::from([(
                ConstraintID::from(10),
                Constraint::less_than_or_equal_to_zero(Function::from(linear!(x) + coeff!(-2.0))),
            )]))
            .build()
            .unwrap()
    }

    fn integer_linear_equality_class() -> InstanceClass {
        InstanceClass::new(vec![InstanceClassClause::new(
            "integer-linear-equality",
            BTreeSet::from([Kind::Integer]),
            DegreeBound::at_most(1),
            BTreeSet::from([Sense::Minimize, Sense::Maximize]),
        )
        .with_regular_constraint(Equality::EqualToZero, DegreeBound::at_most(1))])
    }

    #[test]
    fn policy_stores_owner_operation_arguments_without_interpreting_them() {
        let special_constraint_kinds = BTreeSet::from([
            SpecialConstraintKind::Indicator,
            SpecialConstraintKind::Sos1,
        ]);
        let log_encode_atol = ATol::new(0.2).unwrap();
        let exact_slack_atol = ATol::new(0.1).unwrap();
        let penalty_weights = BTreeMap::from([(ConstraintID::from(10), 0.0)]);
        let policy = PreparationPolicy::new(
            Some(special_constraint_kinds.clone()),
            Some(log_encode_atol),
            true,
            Some((0, exact_slack_atol)),
            Some(0),
            Some(0.0),
            Some(penalty_weights.clone()),
        );

        assert_eq!(
            policy.lower_special_constraints(),
            Some(&special_constraint_kinds)
        );
        assert_eq!(policy.log_encode_used_integers(), Some(log_encode_atol));
        assert!(policy.as_minimization_problem());
        assert_eq!(
            policy.convert_inequality_to_equality_with_integer_slack(),
            Some((0, exact_slack_atol))
        );
        assert_eq!(policy.add_integer_slack_to_inequality(), Some(0));
        assert_eq!(policy.uniform_penalty_method_with_weight(), Some(0.0));
        assert_eq!(policy.penalty_method_with_weights(), Some(&penalty_weights));
    }

    #[test]
    fn identity_preparation_does_not_interpret_unused_parameters() {
        let mut instance = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::Zero)
            .decision_variables(BTreeMap::new())
            .constraints(BTreeMap::new())
            .build()
            .unwrap();
        let expected = instance.clone();
        let input_class = InstanceClass::new(vec![InstanceClassClause::new(
            "empty",
            BTreeSet::new(),
            DegreeBound::at_most(0),
            BTreeSet::from([Sense::Minimize]),
        )]);
        instance
            .prepare(
                &input_class,
                &PreparationPolicy::new(
                    None,
                    None,
                    false,
                    None,
                    None,
                    None,
                    Some(BTreeMap::from([(ConstraintID::from(99), 1.0)])),
                ),
            )
            .unwrap();

        assert_eq!(instance, expected);
    }

    #[test]
    fn one_policy_can_be_used_with_different_input_classes() {
        let x = VariableID::from(1);
        let source = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::from(linear!(x)))
            .decision_variables(BTreeMap::from([(x, DecisionVariable::binary())]))
            .constraints(BTreeMap::new())
            .build()
            .unwrap();
        let policy = PreparationPolicy::new(None, None, false, None, None, None, None);

        let mut openjij_input = source.clone();
        openjij_input.prepare(&openjij_class(), &policy).unwrap();
        let mut highs_input = source.clone();
        highs_input.prepare(&highs_class(), &policy).unwrap();
        let continuous_class = InstanceClass::new(vec![InstanceClassClause::new(
            "continuous-linear",
            BTreeSet::from([Kind::Continuous]),
            DegreeBound::at_most(1),
            BTreeSet::from([Sense::Minimize]),
        )]);
        let mut continuous_input = source.clone();
        let error = continuous_input
            .prepare(&continuous_class, &policy)
            .unwrap_err();

        assert_eq!(openjij_input, source);
        assert_eq!(highs_input, source);
        assert_eq!(continuous_input, source);
        assert!(error
            .to_string()
            .contains("Preparation did not reach the input class"));
    }

    #[test]
    fn prepare_uses_policy_atol_for_log_encoding() {
        let x = VariableID::from(1);
        let mut instance = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::from(linear!(x)))
            .decision_variables(BTreeMap::from([(
                x,
                DecisionVariable::new(
                    Kind::Integer,
                    Bound::new(0.0, 2.0).unwrap(),
                    ATol::default(),
                )
                .unwrap(),
            )]))
            .constraints(BTreeMap::new())
            .build()
            .unwrap();
        let atol = ATol::new(0.2).unwrap();
        let policy = PreparationPolicy::new(None, Some(atol), false, None, None, None, None);

        assert_eq!(policy.log_encode_used_integers(), Some(atol));
        instance.prepare(&openjij_class(), &policy).unwrap();

        assert_eq!(instance.decision_variables().len(), 3);
        assert!(instance.decision_variable_dependency().get(&x).is_some());
        assert!(instance
            .decision_variables()
            .iter()
            .filter(|(id, _)| **id != x)
            .all(|(_, variable)| variable.kind() == Kind::Binary));
    }

    #[test]
    fn owner_operation_validates_stored_arguments_when_prepare_invokes_it() {
        let mut instance = integer_inequality_instance();
        let expected = instance.clone();
        let policy = PreparationPolicy::new(None, None, false, None, Some(0), None, None);

        let error = instance
            .prepare(&integer_linear_equality_class(), &policy)
            .unwrap_err();

        assert!(error.to_string().contains("1..=2^53"));
        assert_eq!(instance, expected);
    }

    #[test]
    fn exact_unavailable_uses_configured_approximate_slack_operation() {
        let mut instance = integer_inequality_instance();
        let source_decision_variable_count = instance.decision_variables().len();
        let policy = PreparationPolicy::new(
            None,
            None,
            false,
            Some((1, ATol::default())),
            Some(1),
            None,
            None,
        );

        let error = instance
            .prepare(&integer_linear_equality_class(), &policy)
            .unwrap_err();

        assert!(error.to_string().contains("did not reach the input class"));
        assert_eq!(
            instance.decision_variables().len(),
            source_decision_variable_count + 1
        );
        assert_eq!(
            instance.constraints()[&ConstraintID::from(10)].equality,
            Equality::LessThanOrEqualToZero
        );
    }

    #[test]
    fn ordinary_exact_error_does_not_invoke_approximate_slack() {
        let x = VariableID::from(1);
        let mut instance = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::from(linear!(x)))
            .decision_variables(BTreeMap::from([(
                x,
                DecisionVariable::new(
                    Kind::Continuous,
                    Bound::new(0.0, 3.0).unwrap(),
                    ATol::default(),
                )
                .unwrap(),
            )]))
            .constraints(BTreeMap::from([(
                ConstraintID::from(10),
                Constraint::less_than_or_equal_to_zero(Function::from(linear!(x) + coeff!(-2.0))),
            )]))
            .build()
            .unwrap();
        let expected = instance.clone();
        let policy = PreparationPolicy::new(
            None,
            None,
            false,
            Some((32, ATol::default())),
            Some(0),
            None,
            None,
        );

        let error = instance
            .prepare(&integer_linear_equality_class(), &policy)
            .unwrap_err();

        assert!(error.to_string().contains("continuous decision variables"));
        assert!(!error.to_string().contains("1..=2^53"));
        assert_eq!(instance, expected);
    }

    #[test]
    fn approximate_slack_can_be_configured_without_exact_slack() {
        let mut instance = integer_inequality_instance();
        let source_decision_variable_count = instance.decision_variables().len();
        let policy = PreparationPolicy::new(None, None, false, None, Some(2), None, None);

        let error = instance
            .prepare(&integer_linear_equality_class(), &policy)
            .unwrap_err();

        assert!(error.to_string().contains("did not reach the input class"));
        assert_eq!(
            instance.decision_variables().len(),
            source_decision_variable_count + 1
        );
        assert_eq!(
            instance.constraints()[&ConstraintID::from(10)].equality,
            Equality::LessThanOrEqualToZero
        );
    }

    #[test]
    fn lowers_selected_special_constraints_with_existing_instance_operation() {
        let x = VariableID::from(1);
        let y = VariableID::from(2);
        let z = VariableID::from(3);
        let w = VariableID::from(4);
        let mut instance = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::from((linear!(x) + linear!(w)).unwrap()))
            .decision_variables(BTreeMap::from([
                (x, DecisionVariable::binary()),
                (y, DecisionVariable::binary()),
                (z, DecisionVariable::binary()),
                (w, DecisionVariable::binary()),
            ]))
            .constraints(BTreeMap::new())
            .indicator_constraints(BTreeMap::from([(
                IndicatorConstraintID::from(10),
                IndicatorConstraint::new(
                    x,
                    Equality::LessThanOrEqualToZero,
                    Function::from(linear!(w) + coeff!(-0.5)),
                ),
            )]))
            .one_hot_constraints(BTreeMap::from([(
                OneHotConstraintID::from(20),
                OneHotConstraint::new(BTreeSet::from([y, z])).unwrap(),
            )]))
            .sos1_constraints(BTreeMap::from([(
                Sos1ConstraintID::from(30),
                Sos1Constraint::new(BTreeSet::from([x, y])).unwrap(),
            )]))
            .build()
            .unwrap();
        instance
            .prepare(
                &highs_class(),
                &PreparationPolicy::new(
                    Some(BTreeSet::from([
                        SpecialConstraintKind::Indicator,
                        SpecialConstraintKind::OneHot,
                        SpecialConstraintKind::Sos1,
                    ])),
                    None,
                    false,
                    None,
                    None,
                    None,
                    None,
                ),
            )
            .unwrap();

        assert!(instance.indicator_constraints().is_empty());
        assert!(instance.one_hot_constraints().is_empty());
        assert!(instance.sos1_constraints().is_empty());
        let solution = instance
            .evaluate(&zero_state_for(&instance), ATol::default())
            .unwrap();
        assert!(solution
            .evaluated_indicator_constraints()
            .is_removed(&IndicatorConstraintID::from(10)));
        assert!(solution
            .evaluated_one_hot_constraints()
            .is_removed(&OneHotConstraintID::from(20)));
        assert!(solution
            .evaluated_sos1_constraints()
            .is_removed(&Sos1ConstraintID::from(30)));
    }

    #[test]
    fn composes_existing_operations_for_openjij_input() {
        let mut instance = integer_inequality_instance();
        instance
            .prepare(
                &openjij_class(),
                &PreparationPolicy::new(
                    None,
                    Some(ATol::default()),
                    true,
                    Some((16, ATol::default())),
                    Some(16),
                    Some(5.0),
                    None,
                ),
            )
            .unwrap();

        assert_eq!(instance.sense(), Sense::Minimize);
        assert!(instance.constraints().is_empty());
        assert!(instance
            .decision_variable_dependency()
            .get(&VariableID::from(1))
            .is_some());
        let solution = instance
            .evaluate(&zero_state_for(&instance), ATol::default())
            .unwrap();
        assert!(solution
            .evaluated_constraints()
            .is_removed(&ConstraintID::from(10)));
    }

    #[test]
    fn per_constraint_weights_follow_recorded_constraint_ids() {
        let x = VariableID::from(1);
        let y = VariableID::from(2);
        let x_constraint = ConstraintID::from(10);
        let y_constraint = ConstraintID::from(20);
        let mut instance = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::Zero)
            .decision_variables(BTreeMap::from([
                (x, DecisionVariable::binary()),
                (y, DecisionVariable::binary()),
            ]))
            .constraints(BTreeMap::from([
                (
                    x_constraint,
                    Constraint::equal_to_zero(Function::from(linear!(x) + coeff!(-1.0))),
                ),
                (
                    y_constraint,
                    Constraint::equal_to_zero(Function::from(linear!(y) + coeff!(-1.0))),
                ),
            ]))
            .build()
            .unwrap();
        instance
            .prepare(
                &openjij_class(),
                &PreparationPolicy::new(
                    None,
                    None,
                    false,
                    None,
                    None,
                    None,
                    Some(BTreeMap::from([(x_constraint, 2.0), (y_constraint, 5.0)])),
                ),
            )
            .unwrap();
        let x_only = instance
            .evaluate(
                &v1::State::from(HashMap::from([
                    (x.into_inner(), 0.0),
                    (y.into_inner(), 1.0),
                ])),
                ATol::default(),
            )
            .unwrap();
        let y_only = instance
            .evaluate(
                &v1::State::from(HashMap::from([
                    (x.into_inner(), 1.0),
                    (y.into_inner(), 0.0),
                ])),
                ATol::default(),
            )
            .unwrap();

        assert_eq!(*x_only.objective(), 2.0);
        assert_eq!(*y_only.objective(), 5.0);
    }

    #[test]
    fn configured_penalty_operation_runs_without_active_constraints() {
        let x = VariableID::from(1);
        let y = VariableID::from(2);
        let mut instance = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::from(linear!(x) * linear!(y)))
            .decision_variables(BTreeMap::from([
                (x, DecisionVariable::binary()),
                (y, DecisionVariable::binary()),
            ]))
            .constraints(BTreeMap::new())
            .build()
            .unwrap();

        let error = instance
            .prepare(
                &highs_class(),
                &PreparationPolicy::new(None, None, false, None, None, Some(0.0), None),
            )
            .unwrap_err();

        assert!(error.to_string().contains("positive and finite"));
    }

    #[test]
    fn configured_penalty_operations_follow_fixed_order_until_target_is_reached() {
        let x = VariableID::from(1);
        let constraint_id = ConstraintID::from(10);
        let mut instance = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::Zero)
            .decision_variables(BTreeMap::from([(x, DecisionVariable::binary())]))
            .constraints(BTreeMap::from([(
                constraint_id,
                Constraint::equal_to_zero(Function::from(linear!(x))),
            )]))
            .build()
            .unwrap();

        instance
            .prepare(
                &openjij_class(),
                &PreparationPolicy::new(
                    None,
                    None,
                    false,
                    None,
                    None,
                    Some(2.0),
                    Some(BTreeMap::from([(constraint_id, 5.0)])),
                ),
            )
            .unwrap();

        let solution = instance
            .evaluate(
                &v1::State::from(HashMap::from([(x.into_inner(), 1.0)])),
                ATol::default(),
            )
            .unwrap();
        assert_eq!(*solution.objective(), 2.0);
    }

    #[test]
    fn input_class_membership_failure_is_an_ordinary_error() {
        let mut instance = integer_inequality_instance();
        let error = instance
            .prepare(
                &openjij_class(),
                &PreparationPolicy::new(None, None, false, None, None, None, None),
            )
            .unwrap_err();

        assert!(error.to_string().contains("did not reach the input class"));
        assert_eq!(instance, integer_inequality_instance());
    }

    #[test]
    fn penalty_failure_keeps_prior_in_place_operations() {
        let mut instance = integer_inequality_instance();
        let error = instance
            .prepare(
                &openjij_class(),
                &PreparationPolicy::new(None, None, true, None, None, None, Some(BTreeMap::new())),
            )
            .unwrap_err();

        assert!(error.to_string().contains("no penalty weight"));
        assert_eq!(instance.sense(), Sense::Minimize);
        assert!(instance.constraints().contains_key(&ConstraintID::from(10)));
    }

    #[test]
    fn penalty_materialization_failure_leaves_instance_unchanged() {
        let x = VariableID::from(1);
        let mut instance = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::Zero)
            .decision_variables(BTreeMap::from([(x, DecisionVariable::binary())]))
            .constraints(BTreeMap::from([(
                ConstraintID::from(10),
                Constraint::equal_to_zero(Function::from((coeff!(2.0) * linear!(x)).unwrap())),
            )]))
            .build()
            .unwrap();
        let expected = instance.clone();

        let error = instance
            .prepare(
                &openjij_class(),
                &PreparationPolicy::new(None, None, false, None, None, Some(f64::MAX), None),
            )
            .unwrap_err();

        assert!(error.is::<crate::CoefficientError>());
        assert_eq!(instance, expected);
    }
}
