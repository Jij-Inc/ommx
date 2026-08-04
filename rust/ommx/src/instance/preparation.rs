//! Policy-driven construction of adapter-ready [`Instance`] values.
//!
//! Preparation is a thin interpreter over existing [`Instance`] operations.
//! The prepared [`Instance`] itself retains everything needed to evaluate
//! solver states and samples.

use super::{Instance, SpecialConstraintKinds};
use crate::{ATol, ConstraintID, InstanceClass};
use anyhow::Context;
use std::collections::BTreeMap;

/// Caller-owned permissions and parameters interpreted by [`Instance::prepare`].
///
/// This value is not an executable recipe. The SDK applies the permitted
/// existing [`Instance`] operations in a fixed order and stops as soon as the
/// input belongs to [`Self::input_class`]. Result-affecting numeric settings,
/// including [`Self::atol`], are captured by the Policy; execution does not
/// consult an ambient tolerance default.
#[derive(Debug, Clone)]
pub struct PreparationPolicy {
    input_class: InstanceClass,
    allowed_special_constraint_lowerings: SpecialConstraintKinds,
    allow_integer_log_encoding: bool,
    allow_sense_normalization: bool,
    inequality_integer_slack_max_range: Option<u64>,
    inequality_integer_slack_max_error: Option<f64>,
    uniform_penalty_weight: Option<f64>,
    penalty_weights: Option<BTreeMap<ConstraintID, f64>>,
    atol: ATol,
}

impl PreparationPolicy {
    /// Construct permissions and parameters for [`Instance::prepare`].
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        input_class: InstanceClass,
        allowed_special_constraint_lowerings: SpecialConstraintKinds,
        allow_integer_log_encoding: bool,
        allow_sense_normalization: bool,
        inequality_integer_slack_max_range: Option<u64>,
        inequality_integer_slack_max_error: Option<f64>,
        uniform_penalty_weight: Option<f64>,
        penalty_weights: Option<BTreeMap<ConstraintID, f64>>,
        atol: ATol,
    ) -> crate::Result<Self> {
        if inequality_integer_slack_max_range == Some(0) {
            crate::bail!("inequality integer slack max range must be positive");
        }
        if inequality_integer_slack_max_error.is_some()
            && inequality_integer_slack_max_range.is_none()
        {
            crate::bail!(
                "inequality integer slack max error requires an inequality integer slack max range"
            );
        }
        if let Some(max_error) = inequality_integer_slack_max_error {
            ensure_positive_finite("inequality integer slack max error", max_error)?;
        }
        if uniform_penalty_weight.is_some() && penalty_weights.is_some() {
            crate::bail!("uniform and per-constraint penalty weights are mutually exclusive");
        }
        if let Some(weight) = uniform_penalty_weight {
            ensure_positive_finite("penalty weight", weight)?;
        }
        if let Some(weights) = &penalty_weights {
            for (&constraint_id, &weight) in weights {
                ensure_positive_finite("penalty weight", weight).with_context(|| {
                    format!("invalid penalty weight for constraint {constraint_id:?}")
                })?;
            }
        }

        Ok(Self {
            input_class,
            allowed_special_constraint_lowerings,
            allow_integer_log_encoding,
            allow_sense_normalization,
            inequality_integer_slack_max_range,
            inequality_integer_slack_max_error,
            uniform_penalty_weight,
            penalty_weights,
            atol,
        })
    }

    /// The Adapter input class that the Instance must belong to after preparation.
    pub fn input_class(&self) -> &InstanceClass {
        &self.input_class
    }

    /// Special-constraint families that may be lowered.
    pub fn allowed_special_constraint_lowerings(&self) -> &SpecialConstraintKinds {
        &self.allowed_special_constraint_lowerings
    }

    /// Whether bounded Integer variables may be log encoded.
    pub fn allow_integer_log_encoding(&self) -> bool {
        self.allow_integer_log_encoding
    }

    /// Whether a maximization objective may be normalized to minimization.
    pub fn allow_sense_normalization(&self) -> bool {
        self.allow_sense_normalization
    }

    /// Maximum range for an Integer slack variable, when slack is permitted.
    pub fn inequality_integer_slack_max_range(&self) -> Option<u64> {
        self.inequality_integer_slack_max_range
    }

    /// Maximum residual introduced by approximate Integer slack.
    ///
    /// `None` permits only exact Integer slack conversion.
    pub fn inequality_integer_slack_max_error(&self) -> Option<f64> {
        self.inequality_integer_slack_max_error
    }

    /// Uniform finite penalty weight, when selected.
    pub fn uniform_penalty_weight(&self) -> Option<f64> {
        self.uniform_penalty_weight
    }

    /// Per-constraint finite penalty weights, when selected.
    pub fn penalty_weights(&self) -> Option<&BTreeMap<ConstraintID, f64>> {
        self.penalty_weights.as_ref()
    }

    /// Absolute tolerance used by result-affecting preparation operations.
    pub fn atol(&self) -> ATol {
        self.atol
    }
}

impl Instance {
    /// Prepare this Instance in place to satisfy the Policy's input class.
    ///
    /// Preparation applies the transformations permitted by `policy` in a fixed
    /// order. On success, `policy.input_class().contains(self)` is true.
    ///
    /// This method is not transactional across transformations. If it returns
    /// an error, changes made by earlier transformations remain applied.
    #[tracing::instrument(skip_all)]
    pub fn prepare(&mut self, policy: &PreparationPolicy) -> crate::Result<()> {
        macro_rules! return_if_in_input_class {
            () => {
                if policy.input_class.contains(self) {
                    return Ok(());
                }
            };
        }

        return_if_in_input_class!();

        self.lower_special_constraints(&policy.allowed_special_constraint_lowerings)?;
        return_if_in_input_class!();

        if policy.allow_integer_log_encoding {
            self.log_encode_used_integers(policy.atol)?;
            return_if_in_input_class!();
        }

        if policy.allow_sense_normalization {
            self.as_minimization_problem();
            return_if_in_input_class!();
        }

        if let Some(max_range) = policy.inequality_integer_slack_max_range {
            self.convert_inequalities_with_integer_slack(
                max_range,
                policy.inequality_integer_slack_max_error,
                policy.atol,
            )?;
            return_if_in_input_class!();
        }

        if !self.constraints().is_empty()
            && (policy.uniform_penalty_weight.is_some() || policy.penalty_weights.is_some())
        {
            match (
                policy.uniform_penalty_weight,
                policy.penalty_weights.as_ref(),
            ) {
                (Some(weight), None) => self.uniform_penalty_method_with_weight(weight)?,
                (None, Some(weights)) => self.penalty_method_with_weights(weights)?,
                (None, None) => unreachable!("penalty presence was checked above"),
                (Some(_), Some(_)) => unreachable!("PreparationPolicy validates penalty modes"),
            }
            return_if_in_input_class!();
        }

        if policy.allow_integer_log_encoding {
            self.log_encode_used_integers(policy.atol)?;
            return_if_in_input_class!();
        }

        let membership = policy.input_class.check_membership(self);
        crate::bail!("Preparation did not reach the input class:\n{membership}")
    }
}

fn ensure_positive_finite(name: &str, value: f64) -> crate::Result<()> {
    if !value.is_finite() || value <= 0.0 {
        crate::bail!("{name} must be positive and finite: {value}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        coeff, linear, v1, Bound, Coefficient, Constraint, DecisionVariable, DegreeBound, Equality,
        Evaluate, Function, IndicatorConstraint, IndicatorConstraintID, InstanceClassClause, Kind,
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

    #[test]
    fn policy_validates_only_its_own_values() {
        assert!(PreparationPolicy::new(
            openjij_class(),
            SpecialConstraintKinds::new(),
            false,
            false,
            Some(0),
            None,
            None,
            None,
            ATol::default(),
        )
        .is_err());
        assert!(PreparationPolicy::new(
            openjij_class(),
            SpecialConstraintKinds::new(),
            false,
            false,
            None,
            Some(1.0),
            None,
            None,
            ATol::default(),
        )
        .is_err());
        assert!(PreparationPolicy::new(
            openjij_class(),
            SpecialConstraintKinds::new(),
            false,
            false,
            None,
            None,
            Some(1.0),
            Some(BTreeMap::new()),
            ATol::default(),
        )
        .is_err());
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
                &PreparationPolicy::new(
                    input_class,
                    SpecialConstraintKinds::new(),
                    false,
                    false,
                    None,
                    None,
                    None,
                    Some(BTreeMap::from([(ConstraintID::from(99), 1.0)])),
                    ATol::default(),
                )
                .unwrap(),
            )
            .unwrap();

        assert_eq!(instance, expected);
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
        let policy = PreparationPolicy::new(
            openjij_class(),
            SpecialConstraintKinds::new(),
            true,
            false,
            None,
            None,
            None,
            None,
            atol,
        )
        .unwrap();

        assert_eq!(policy.atol(), atol);
        instance.prepare(&policy).unwrap();

        assert_eq!(instance.decision_variables().len(), 3);
        assert!(instance.decision_variable_dependency().get(&x).is_some());
        assert!(instance
            .decision_variables()
            .iter()
            .filter(|(id, _)| **id != x)
            .all(|(_, variable)| variable.kind() == Kind::Binary));
    }

    #[test]
    fn prepare_uses_policy_atol_for_integer_slack() {
        let x = VariableID::from(1);
        let mut instance = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::Zero)
            .decision_variables(BTreeMap::from([(
                x,
                DecisionVariable::new(
                    Kind::Integer,
                    Bound::new(0.0, 1.0).unwrap(),
                    ATol::default(),
                )
                .unwrap(),
            )]))
            .constraints(BTreeMap::from([(
                ConstraintID::from(10),
                Constraint::less_than_or_equal_to_zero(Function::from(
                    (Coefficient::try_from(0.9993582466233818).unwrap() * linear!(x)).unwrap(),
                )),
            )]))
            .build()
            .unwrap();
        let input_class = InstanceClass::new(vec![InstanceClassClause::new(
            "integer-linear-equality",
            BTreeSet::from([Kind::Integer]),
            DegreeBound::at_most(1),
            BTreeSet::from([Sense::Minimize]),
        )
        .with_regular_constraint(Equality::EqualToZero, DegreeBound::at_most(1))]);
        let atol = ATol::new(1e-17).unwrap();
        let policy = PreparationPolicy::new(
            input_class,
            SpecialConstraintKinds::new(),
            false,
            false,
            Some(4),
            None,
            None,
            None,
            atol,
        )
        .unwrap();

        let mut default_atol_result = instance.clone();
        default_atol_result
            .convert_inequalities_with_integer_slack(4, None, ATol::default())
            .unwrap();
        assert_eq!(default_atol_result.decision_variables().len(), 2);

        instance.prepare(&policy).unwrap();

        assert_eq!(instance.decision_variables().len(), 1);
        assert!(instance.constraints().is_empty());
        assert!(instance
            .removed_constraints()
            .contains_key(&ConstraintID::from(10)));
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
                &PreparationPolicy::new(
                    highs_class(),
                    BTreeSet::from([
                        SpecialConstraintKind::Indicator,
                        SpecialConstraintKind::OneHot,
                        SpecialConstraintKind::Sos1,
                    ]),
                    false,
                    false,
                    None,
                    None,
                    None,
                    None,
                    ATol::default(),
                )
                .unwrap(),
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
                &PreparationPolicy::new(
                    openjij_class(),
                    SpecialConstraintKinds::new(),
                    true,
                    true,
                    Some(16),
                    None,
                    Some(5.0),
                    None,
                    ATol::default(),
                )
                .unwrap(),
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
    fn approximate_slack_falls_back_only_from_exact_unavailable() {
        let mut instance = integer_inequality_instance();
        instance.as_minimization_problem();
        let source_decision_variable_count = instance.decision_variables().len();
        let input_class = InstanceClass::new(vec![InstanceClassClause::new(
            "integer-unconstrained",
            BTreeSet::from([Kind::Integer]),
            DegreeBound::at_most(2),
            BTreeSet::from([Sense::Minimize]),
        )]);
        instance
            .prepare(
                &PreparationPolicy::new(
                    input_class,
                    SpecialConstraintKinds::new(),
                    false,
                    false,
                    Some(1),
                    Some(2.0),
                    Some(3.0),
                    None,
                    ATol::default(),
                )
                .unwrap(),
            )
            .unwrap();

        assert!(instance.constraints().is_empty());
        assert_eq!(
            instance.decision_variables().len(),
            source_decision_variable_count + 1
        );
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
                &PreparationPolicy::new(
                    openjij_class(),
                    SpecialConstraintKinds::new(),
                    false,
                    false,
                    None,
                    None,
                    None,
                    Some(BTreeMap::from([(x_constraint, 2.0), (y_constraint, 5.0)])),
                    ATol::default(),
                )
                .unwrap(),
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
    fn input_class_membership_failure_is_an_ordinary_error() {
        let mut instance = integer_inequality_instance();
        let error = instance
            .prepare(
                &PreparationPolicy::new(
                    openjij_class(),
                    SpecialConstraintKinds::new(),
                    false,
                    false,
                    None,
                    None,
                    None,
                    None,
                    ATol::default(),
                )
                .unwrap(),
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
                &PreparationPolicy::new(
                    openjij_class(),
                    SpecialConstraintKinds::new(),
                    false,
                    true,
                    None,
                    None,
                    None,
                    Some(BTreeMap::new()),
                    ATol::default(),
                )
                .unwrap(),
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
                &PreparationPolicy::new(
                    openjij_class(),
                    SpecialConstraintKinds::new(),
                    false,
                    false,
                    None,
                    None,
                    Some(f64::MAX),
                    None,
                    ATol::default(),
                )
                .unwrap(),
            )
            .unwrap_err();

        assert!(error.is::<crate::CoefficientError>());
        assert_eq!(instance, expected);
    }

    #[test]
    fn operation_errors_propagate_without_preparation_wrapper() {
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
        let input_class = InstanceClass::new(vec![InstanceClassClause::new(
            "continuous-linear-equality",
            BTreeSet::from([Kind::Continuous]),
            DegreeBound::at_most(1),
            BTreeSet::from([Sense::Minimize]),
        )
        .with_regular_constraint(Equality::EqualToZero, DegreeBound::at_most(1))]);
        let error = instance
            .prepare(
                &PreparationPolicy::new(
                    input_class,
                    SpecialConstraintKinds::new(),
                    false,
                    false,
                    Some(16),
                    Some(1.0),
                    None,
                    None,
                    ATol::default(),
                )
                .unwrap(),
            )
            .unwrap_err();

        assert!(error.to_string().contains("continuous decision variables"));
        assert_eq!(instance, expected);
    }
}
