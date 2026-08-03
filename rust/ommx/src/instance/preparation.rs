//! Policy-driven construction of adapter-ready [`Instance`] values.
//!
//! Preparation is a thin interpreter over existing [`Instance`] operations.
//! The prepared [`Instance`] itself retains everything needed to evaluate
//! solver states and samples.

use super::{Instance, SpecialConstraintKinds};
use crate::{v1, ATol, ConstraintID, Equality, InstanceClass, Kind, Sense};
use anyhow::Context;
use std::collections::{BTreeMap, BTreeSet, HashMap};

/// Caller-owned permissions and parameters interpreted by [`Instance::prepare`].
///
/// This value is not an executable recipe. The SDK applies the permitted
/// existing [`Instance`] operations in a fixed order and stops as soon as the
/// input belongs to [`Self::acceptable_instance_class`].
#[derive(Debug, Clone)]
pub struct PreparationPolicy {
    acceptable_instance_class: InstanceClass,
    allowed_special_constraint_lowerings: SpecialConstraintKinds,
    allow_integer_log_encoding: bool,
    allow_sense_normalization: bool,
    inequality_integer_slack_max_range: Option<u64>,
    allow_approximate_integer_slack: bool,
    uniform_penalty_weight: Option<f64>,
    penalty_weights: Option<BTreeMap<ConstraintID, f64>>,
}

impl PreparationPolicy {
    /// Construct permissions and parameters for [`Instance::prepare`].
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        acceptable_instance_class: InstanceClass,
        allowed_special_constraint_lowerings: SpecialConstraintKinds,
        allow_integer_log_encoding: bool,
        allow_sense_normalization: bool,
        inequality_integer_slack_max_range: Option<u64>,
        allow_approximate_integer_slack: bool,
        uniform_penalty_weight: Option<f64>,
        penalty_weights: Option<BTreeMap<ConstraintID, f64>>,
    ) -> crate::Result<Self> {
        if inequality_integer_slack_max_range == Some(0) {
            crate::bail!("inequality integer slack max range must be positive");
        }
        if allow_approximate_integer_slack && inequality_integer_slack_max_range.is_none() {
            crate::bail!(
                "approximate integer slack requires an inequality integer slack max range"
            );
        }
        if uniform_penalty_weight.is_some() && penalty_weights.is_some() {
            crate::bail!("uniform and per-constraint penalty weights are mutually exclusive");
        }
        if let Some(weight) = uniform_penalty_weight {
            ensure_positive_finite_penalty(weight)?;
        }
        if let Some(weights) = &penalty_weights {
            for (&constraint_id, &weight) in weights {
                ensure_positive_finite_penalty(weight).with_context(|| {
                    format!("invalid penalty weight for constraint {constraint_id:?}")
                })?;
            }
        }

        Ok(Self {
            acceptable_instance_class,
            allowed_special_constraint_lowerings,
            allow_integer_log_encoding,
            allow_sense_normalization,
            inequality_integer_slack_max_range,
            allow_approximate_integer_slack,
            uniform_penalty_weight,
            penalty_weights,
        })
    }

    /// The class that the Instance must belong to after preparation.
    pub fn acceptable_instance_class(&self) -> &InstanceClass {
        &self.acceptable_instance_class
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

    /// Whether approximate Integer slack may be used when exact slack is unavailable.
    pub fn allow_approximate_integer_slack(&self) -> bool {
        self.allow_approximate_integer_slack
    }

    /// Uniform finite penalty weight, when selected.
    pub fn uniform_penalty_weight(&self) -> Option<f64> {
        self.uniform_penalty_weight
    }

    /// Per-constraint finite penalty weights, when selected.
    pub fn penalty_weights(&self) -> Option<&BTreeMap<ConstraintID, f64>> {
        self.penalty_weights.as_ref()
    }
}

impl Instance {
    /// Apply Policy-permitted existing Instance operations in place.
    ///
    /// Operations run in canonical order and stop as soon as this Instance
    /// belongs to the acceptable class. Existing in-place operations are
    /// applied directly and retain their own failure semantics. The consuming
    /// penalty operations are the only point where the current Instance is
    /// cloned, after their read-only validation; the clone replaces `self`
    /// only after penalty materialization succeeds. Preparation is not
    /// transactional across operations, so a later failure does not roll back
    /// earlier successful in-place operations.
    /// Existing operation errors are returned unchanged. If all permitted
    /// operations are exhausted, an ordinary error reports the final
    /// [`InstanceClass`](crate::InstanceClass) membership failure.
    #[tracing::instrument(skip_all)]
    pub fn prepare(&mut self, policy: &PreparationPolicy) -> crate::Result<()> {
        macro_rules! return_if_acceptable {
            () => {
                if policy.acceptable_instance_class.contains(self) {
                    return Ok(());
                }
            };
        }

        return_if_acceptable!();

        let source_constraint_ids = policy
            .penalty_weights
            .as_ref()
            .map(|_| self.constraints().keys().copied().collect::<BTreeSet<_>>());

        self.lower_special_constraints(&policy.allowed_special_constraint_lowerings)?;
        return_if_acceptable!();

        if policy.allow_integer_log_encoding {
            log_encode_used_integers(self)?;
            return_if_acceptable!();
        }

        if policy.allow_sense_normalization {
            self.as_minimization_problem();
            return_if_acceptable!();
        }

        if let Some(max_range) = policy.inequality_integer_slack_max_range {
            add_integer_slack(self, max_range, policy.allow_approximate_integer_slack)?;
            return_if_acceptable!();
        }

        if !self.constraints().is_empty()
            && (policy.uniform_penalty_weight.is_some() || policy.penalty_weights.is_some())
        {
            apply_finite_penalty(
                self,
                policy.uniform_penalty_weight,
                policy.penalty_weights.as_ref(),
                source_constraint_ids.as_ref(),
            )?;
            return_if_acceptable!();
        }

        if policy.allow_integer_log_encoding {
            log_encode_used_integers(self)?;
            return_if_acceptable!();
        }

        let membership = policy.acceptable_instance_class.check_membership(self);
        crate::bail!("Preparation did not reach the acceptable InstanceClass:\n{membership}")
    }
}

fn ensure_positive_finite_penalty(weight: f64) -> crate::Result<()> {
    if !weight.is_finite() || weight <= 0.0 {
        crate::bail!("penalty weights must be positive and finite: {weight}");
    }
    Ok(())
}

fn log_encode_used_integers(input: &mut Instance) -> crate::Result<()> {
    let integer_ids = input
        .used_decision_variables()
        .into_iter()
        .filter_map(|(id, variable)| (variable.kind() == Kind::Integer).then_some(id))
        .collect::<BTreeSet<_>>();
    input.log_encode(integer_ids, ATol::default())?;
    Ok(())
}

fn add_integer_slack(
    input: &mut Instance,
    max_range: u64,
    allow_approximate: bool,
) -> crate::Result<()> {
    let inequality_ids = input
        .constraints()
        .iter()
        .filter_map(|(&id, constraint)| {
            (constraint.equality == Equality::LessThanOrEqualToZero).then_some(id)
        })
        .collect::<Vec<_>>();

    for constraint_id in inequality_ids {
        match input.convert_inequality_to_equality_with_integer_slack(
            constraint_id.into_inner(),
            max_range,
            ATol::default(),
        ) {
            Ok(()) => {}
            Err(error)
                if allow_approximate
                    && error.is::<super::slack::ExactIntegerSlackUnavailable>() =>
            {
                input.add_integer_slack_to_inequality(constraint_id.into_inner(), max_range)?;
            }
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

fn apply_finite_penalty(
    input: &mut Instance,
    uniform_weight: Option<f64>,
    per_constraint_weights: Option<&BTreeMap<ConstraintID, f64>>,
    source_constraint_ids: Option<&BTreeSet<ConstraintID>>,
) -> crate::Result<()> {
    anyhow::ensure!(
        input.sense() == Sense::Minimize,
        "positive finite penalty weights require a minimization problem"
    );

    match (uniform_weight, per_constraint_weights) {
        (Some(weight), None) => {
            let parametric = input.clone().uniform_penalty_method()?;
            let parameter_id = parametric
                .parameters()
                .keys()
                .next()
                .copied()
                .context("uniform penalty method did not create a parameter")?;
            let prepared = parametric.with_parameters(v1::Parameters {
                entries: HashMap::from([(parameter_id.into_inner(), weight)]),
            })?;
            *input = prepared;
            Ok(())
        }
        (None, Some(weights)) => {
            let active_ids = input.constraints().keys().copied().collect::<Vec<_>>();
            let source_constraint_ids = source_constraint_ids
                .context("source constraint IDs were not captured for penalty weights")?;
            for constraint_id in weights.keys() {
                anyhow::ensure!(
                    source_constraint_ids.contains(constraint_id),
                    "penalty weight refers to unknown source constraint {constraint_id:?}"
                );
            }
            anyhow::ensure!(
                active_ids
                    .iter()
                    .all(|constraint_id| source_constraint_ids.contains(constraint_id)),
                "per-constraint penalty weights cannot address constraints generated during Preparation; use a uniform penalty weight"
            );
            let validated_weights = active_ids
                .into_iter()
                .map(|constraint_id| {
                    weights
                        .get(&constraint_id)
                        .copied()
                        .map(|weight| (constraint_id, weight))
                        .with_context(|| {
                            format!("no penalty weight for active constraint {constraint_id:?}")
                        })
                })
                .collect::<crate::Result<Vec<_>>>()?;
            let parametric = input.clone().penalty_method()?;
            let mut entries = HashMap::with_capacity(validated_weights.len());
            for (constraint_id, weight) in validated_weights {
                let (_, removed_reason) = parametric
                    .removed_constraints()
                    .get(&constraint_id)
                    .with_context(|| {
                        format!("penalty method did not remove constraint {constraint_id:?}")
                    })?;
                let parameter_id = removed_reason
                    .parameters
                    .get("parameter_id")
                    .with_context(|| {
                        format!(
                            "penalty method did not record a parameter for constraint {constraint_id:?}"
                        )
                    })?
                    .parse::<u64>()
                    .with_context(|| {
                        format!(
                            "penalty method recorded an invalid parameter for constraint {constraint_id:?}"
                        )
                    })?;
                entries.insert(parameter_id, weight);
            }
            let prepared = parametric.with_parameters(v1::Parameters { entries })?;
            *input = prepared;
            Ok(())
        }
        (None, None) => Ok(()),
        (Some(_), Some(_)) => unreachable!("PreparationPolicy validates penalty modes"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        coeff, linear, Bound, Constraint, DecisionVariable, DegreeBound, Evaluate, Function,
        IndicatorConstraint, IndicatorConstraintID, InstanceClassClause, OneHotConstraint,
        OneHotConstraintID, Sos1Constraint, Sos1ConstraintID, SpecialConstraintKind, VariableID,
    };

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
            false,
            None,
            None,
        )
        .is_err());
        assert!(PreparationPolicy::new(
            openjij_class(),
            SpecialConstraintKinds::new(),
            false,
            false,
            None,
            true,
            None,
            None,
        )
        .is_err());
        assert!(PreparationPolicy::new(
            openjij_class(),
            SpecialConstraintKinds::new(),
            false,
            false,
            None,
            false,
            Some(1.0),
            Some(BTreeMap::new()),
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
        let target = InstanceClass::new(vec![InstanceClassClause::new(
            "empty",
            BTreeSet::new(),
            DegreeBound::at_most(0),
            BTreeSet::from([Sense::Minimize]),
        )]);
        instance
            .prepare(
                &PreparationPolicy::new(
                    target,
                    SpecialConstraintKinds::new(),
                    false,
                    false,
                    None,
                    false,
                    None,
                    Some(BTreeMap::from([(ConstraintID::from(99), 1.0)])),
                )
                .unwrap(),
            )
            .unwrap();

        assert_eq!(instance, expected);
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
                    false,
                    None,
                    None,
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
                    false,
                    Some(5.0),
                    None,
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
        let target = InstanceClass::new(vec![InstanceClassClause::new(
            "integer-unconstrained",
            BTreeSet::from([Kind::Integer]),
            DegreeBound::at_most(2),
            BTreeSet::from([Sense::Minimize]),
        )]);
        instance
            .prepare(
                &PreparationPolicy::new(
                    target,
                    SpecialConstraintKinds::new(),
                    false,
                    false,
                    Some(1),
                    true,
                    Some(3.0),
                    None,
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
                    false,
                    None,
                    Some(BTreeMap::from([(x_constraint, 2.0), (y_constraint, 5.0)])),
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
    fn final_membership_failure_is_an_ordinary_error() {
        let mut instance = integer_inequality_instance();
        let error = instance
            .prepare(
                &PreparationPolicy::new(
                    openjij_class(),
                    SpecialConstraintKinds::new(),
                    false,
                    false,
                    None,
                    false,
                    None,
                    None,
                )
                .unwrap(),
            )
            .unwrap_err();

        assert!(error
            .to_string()
            .contains("did not reach the acceptable InstanceClass"));
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
                    false,
                    None,
                    Some(BTreeMap::new()),
                )
                .unwrap(),
            )
            .unwrap_err();

        assert!(error.to_string().contains("no penalty weight"));
        assert_eq!(instance.sense(), Sense::Minimize);
        assert!(instance.constraints().contains_key(&ConstraintID::from(10)));
    }

    #[test]
    fn penalty_materialization_failure_does_not_commit_the_clone() {
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
                    false,
                    Some(f64::MAX),
                    None,
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
        let target = InstanceClass::new(vec![InstanceClassClause::new(
            "continuous-linear-equality",
            BTreeSet::from([Kind::Continuous]),
            DegreeBound::at_most(1),
            BTreeSet::from([Sense::Minimize]),
        )
        .with_regular_constraint(Equality::EqualToZero, DegreeBound::at_most(1))]);
        let error = instance
            .prepare(
                &PreparationPolicy::new(
                    target,
                    SpecialConstraintKinds::new(),
                    false,
                    false,
                    Some(16),
                    true,
                    None,
                    None,
                )
                .unwrap(),
            )
            .unwrap_err();

        assert!(error.to_string().contains("continuous decision variables"));
        assert_eq!(instance, expected);
    }
}
