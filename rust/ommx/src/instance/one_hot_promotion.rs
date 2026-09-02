//! Checked promotion of exact regular equalities to one-hot constraints.
//!
//! A promotion request is deliberately untrusted. It carries only stable IDs
//! and claimed one-hot membership; the current [`Instance`] remains the source
//! of truth for the regular row and decision-variable domains. Promotion is
//! allowed only when the active source is exactly a non-zero scalar multiple of
//! `sum(variables) - 1 = 0` over binary variables.

use super::Instance;
use crate::{
    Coefficient, ConstraintContext, ConstraintID, Equality, Kind, LinearMonomial, OneHotConstraint,
    OneHotConstraintID, RemovedReason, VariableID, VariableIDSet,
};
use std::collections::BTreeMap;

const PROMOTION_REASON: &str = "promoted validated one-hot equality";
const TARGET_ID_PARAMETER: &str = "one_hot_constraint_id";

/// Untrusted stable-ID request for one one-hot promotion.
///
/// The request claims that `source_constraint_id` identifies an active regular
/// equality whose exact support is `variables`. Callers may construct invalid
/// requests directly; [`Instance::promote_one_hot`] checks the source row,
/// relation, coefficients, and current variable domains before mutation.
///
/// No target ID is supplied. A fresh [`OneHotConstraintID`] is allocated from
/// the current [`Instance`] only after the request has been verified.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OneHotPromotionRequest {
    /// Active regular constraint claimed to encode one-hot membership.
    pub source_constraint_id: ConstraintID,
    /// Claimed one-hot members.
    pub variables: VariableIDSet,
}

impl OneHotPromotionRequest {
    /// Convert one legacy v1 one-hot hint into an untrusted request.
    ///
    /// This conversion is independent of any [`Instance`]. It validates only
    /// the wire-level set shape: the member list must be non-empty and contain
    /// no duplicate IDs. Domain and source-row validation is deferred to
    /// [`Instance::promote_one_hot`]. Constraint ID `0` is a valid stable ID.
    pub fn from_v1_hint(hint: &crate::v1::OneHot) -> crate::Result<Self> {
        if hint.decision_variables.is_empty() {
            crate::bail!("Legacy v1 one-hot hint must contain at least one member");
        }

        let mut variables = VariableIDSet::new();
        for &raw_id in &hint.decision_variables {
            let id = VariableID::from(raw_id);
            if !variables.insert(id) {
                crate::bail!(
                    { ?id },
                    "Legacy v1 one-hot hint repeats member {id:?}"
                );
            }
        }

        Ok(Self {
            source_constraint_id: ConstraintID::from(hint.constraint_id),
            variables,
        })
    }
}

/// Result of one checked one-hot promotion.
///
/// The source regular constraint remains in the same [`Instance`] as removed
/// history, and the new one-hot constraint receives a copy of its context.
#[must_use = "the result identifies the promoted constraint and retained source"]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OneHotPromotion {
    one_hot_constraint_id: OneHotConstraintID,
    variables: VariableIDSet,
    relaxed_constraint_id: ConstraintID,
}

impl OneHotPromotion {
    /// ID allocated to the promoted one-hot constraint.
    pub fn one_hot_constraint_id(&self) -> OneHotConstraintID {
        self.one_hot_constraint_id
    }

    /// Verified one-hot members.
    pub fn variables(&self) -> &VariableIDSet {
        &self.variables
    }

    /// Regular source constraint moved from active to removed.
    pub fn relaxed_constraint_id(&self) -> ConstraintID {
        self.relaxed_constraint_id
    }
}

#[derive(Debug)]
struct OneHotPromotionPlan {
    result: OneHotPromotion,
    one_hot_constraint: OneHotConstraint,
    context: ConstraintContext,
}

impl Instance {
    /// Promote an exact regular equality to a one-hot constraint.
    ///
    /// The request is untrusted. This method verifies against the current
    /// instance that:
    ///
    /// - the claimed member set is non-empty;
    /// - the source is an active regular equality;
    /// - its function is exactly linear;
    /// - its support is exactly the claimed member set;
    /// - every member is a binary decision variable; and
    /// - for one common non-zero coefficient `c`, the row is exactly
    ///   `c * (sum(variables) - 1) = 0`.
    ///
    /// On success the source moves to `removed_constraints`, its context is
    /// copied to the new active one-hot constraint, and the removal reason
    /// records the allocated target ID. The operation is atomic: any returned
    /// error leaves this instance unchanged.
    pub fn promote_one_hot(
        &mut self,
        request: &OneHotPromotionRequest,
    ) -> crate::Result<OneHotPromotion> {
        let plan = self.plan_one_hot_promotion(request)?;
        let result = plan.result.clone();

        // Commit to a rollback copy so even a storage-layer invariant failure
        // cannot expose a partially promoted Instance.
        let mut staged = self.clone();
        staged
            .constraint_collection
            .move_active_rows_to_removed_with_reasons(BTreeMap::from([(
                result.relaxed_constraint_id,
                RemovedReason {
                    reason: PROMOTION_REASON.to_string(),
                    parameters: [(
                        TARGET_ID_PARAMETER.to_string(),
                        result.one_hot_constraint_id.into_inner().to_string(),
                    )]
                    .into_iter()
                    .collect(),
                },
            )]))?;
        staged
            .one_hot_constraint_collection
            .insert_active_with_context(
                result.one_hot_constraint_id,
                plan.one_hot_constraint,
                plan.context,
            )?;
        *self = staged;

        Ok(result)
    }

    fn plan_one_hot_promotion(
        &self,
        request: &OneHotPromotionRequest,
    ) -> crate::Result<OneHotPromotionPlan> {
        if request.variables.is_empty() {
            crate::bail!("One-hot promotion request must contain at least one member");
        }

        let source = self
            .constraint_collection
            .active()
            .get(&request.source_constraint_id)
            .ok_or_else(|| {
                crate::error!(
                    { source_constraint_id = ?request.source_constraint_id },
                    "Active regular constraint {:?} was not found",
                    request.source_constraint_id
                )
            })?;
        if source.equality != Equality::EqualToZero {
            crate::bail!(
                { source_constraint_id = ?request.source_constraint_id },
                "Regular constraint {:?} is not an equality-to-zero constraint",
                request.source_constraint_id
            );
        }

        let linear = source.function().as_linear().ok_or_else(|| {
            crate::error!(
                { source_constraint_id = ?request.source_constraint_id },
                "Regular constraint {:?} is not exactly linear",
                request.source_constraint_id
            )
        })?;
        let coefficients: BTreeMap<VariableID, Coefficient> = linear.linear_terms().collect();
        let support: VariableIDSet = coefficients.keys().copied().collect();
        if support != request.variables {
            crate::bail!(
                {
                    source_constraint_id = ?request.source_constraint_id,
                    ?support,
                    claimed_variables = ?request.variables
                },
                "Regular constraint {:?} has linear support {support:?}, expected {:?}",
                request.source_constraint_id,
                request.variables
            );
        }

        for &variable_id in &request.variables {
            let variable = self.decision_variables.get(&variable_id).ok_or_else(|| {
                crate::error!(
                    { ?variable_id },
                    "One-hot promotion member {variable_id:?} is not a decision variable"
                )
            })?;
            if variable.kind() != Kind::Binary {
                crate::bail!(
                    { ?variable_id, kind = ?variable.kind() },
                    "One-hot promotion member {variable_id:?} must be binary"
                );
            }
        }

        let common_coefficient = coefficients
            .values()
            .next()
            .copied()
            .expect("non-empty verified support has a coefficient");
        if coefficients
            .values()
            .any(|&coefficient| coefficient != common_coefficient)
        {
            crate::bail!(
                { source_constraint_id = ?request.source_constraint_id },
                "Regular constraint {:?} does not use one common coefficient for all one-hot members",
                request.source_constraint_id
            );
        }

        let constant = linear.get(&LinearMonomial::Constant);
        if constant != Some(-common_coefficient) {
            crate::bail!(
                {
                    source_constraint_id = ?request.source_constraint_id,
                    ?constant,
                    expected = ?(-common_coefficient)
                },
                "Regular constraint {:?} has constant coefficient {constant:?}, expected {:?}",
                request.source_constraint_id,
                -common_coefficient
            );
        }

        self.one_hot_constraint_collection
            .ensure_unused_id_capacity(1)?;
        let one_hot_constraint_id = self.one_hot_constraint_collection.unused_id();
        let one_hot_constraint = OneHotConstraint::new(request.variables.clone())?;
        let context = self
            .constraint_collection
            .context()
            .collect_for(request.source_constraint_id);

        Ok(OneHotPromotionPlan {
            result: OneHotPromotion {
                one_hot_constraint_id,
                variables: request.variables.clone(),
                relaxed_constraint_id: request.source_constraint_id,
            },
            one_hot_constraint,
            context,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        coeff, linear, quadratic, Constraint, ConstraintContext, DecisionVariable, Function,
        Linear, ModelingLabel, Sense,
    };

    fn variables(ids: impl IntoIterator<Item = u64>) -> VariableIDSet {
        ids.into_iter().map(VariableID::from).collect()
    }

    fn exact_scaled_one_hot(ids: &[u64], coefficient: f64) -> Function {
        let coefficient = Coefficient::try_from(coefficient).unwrap();
        let linear = ids.iter().copied().fold(
            Linear::single_term(LinearMonomial::Constant, -coefficient),
            |sum, id| (sum + Linear::single_term(LinearMonomial::from(id), coefficient)).unwrap(),
        );
        Function::from(linear)
    }

    fn instance_with_source(source: Constraint) -> Instance {
        Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::Zero)
            .decision_variables(BTreeMap::from([
                (VariableID::from(1), DecisionVariable::binary()),
                (VariableID::from(2), DecisionVariable::binary()),
                (VariableID::from(3), DecisionVariable::binary()),
                (VariableID::from(4), DecisionVariable::integer()),
            ]))
            .constraints(BTreeMap::from([(ConstraintID::from(10), source)]))
            .build()
            .unwrap()
    }

    fn request(ids: impl IntoIterator<Item = u64>) -> OneHotPromotionRequest {
        OneHotPromotionRequest {
            source_constraint_id: ConstraintID::from(10),
            variables: variables(ids),
        }
    }

    fn assert_atomic_rejection(
        mut instance: Instance,
        request: &OneHotPromotionRequest,
        message: &str,
    ) {
        let before = instance.clone();
        let error = instance.promote_one_hot(request).unwrap_err();
        assert!(
            error.to_string().contains(message),
            "unexpected error: {error:#}"
        );
        assert_eq!(instance, before);
    }

    #[test]
    fn converts_v1_hint_without_instance_and_rejects_invalid_member_lists() {
        let hint = crate::v1::OneHot {
            constraint_id: 0,
            decision_variables: vec![2, 1],
        };
        assert_eq!(
            OneHotPromotionRequest::from_v1_hint(&hint).unwrap(),
            OneHotPromotionRequest {
                source_constraint_id: ConstraintID::from(0),
                variables: variables([1, 2]),
            }
        );

        for (members, expected) in [
            (vec![], "at least one member"),
            (vec![1, 1], "repeats member"),
        ] {
            let error = OneHotPromotionRequest::from_v1_hint(&crate::v1::OneHot {
                constraint_id: 7,
                decision_variables: members,
            })
            .unwrap_err();
            assert!(error.to_string().contains(expected));
        }
    }

    #[test]
    fn promotes_exact_scaled_equality_and_preserves_history_and_context() {
        for coefficient in [1.0, 2.5, -3.0] {
            let mut instance = instance_with_source(Constraint::equal_to_zero(
                exact_scaled_one_hot(&[1, 2, 3], coefficient),
            ));
            let context = ConstraintContext {
                label: ModelingLabel {
                    name: Some("choose".to_string()),
                    subscripts: vec![4, 2],
                    description: Some("exactly one".to_string()),
                    ..Default::default()
                },
                provenance: vec![crate::Provenance::Sos1Constraint(
                    crate::Sos1ConstraintID::from(9),
                )],
            };
            instance
                .set_constraint_context(ConstraintID::from(10), context.clone())
                .unwrap();

            let promotion = instance.promote_one_hot(&request([1, 2, 3])).unwrap();

            assert_eq!(
                promotion.one_hot_constraint_id(),
                OneHotConstraintID::from(0)
            );
            assert_eq!(promotion.variables(), &variables([1, 2, 3]));
            assert_eq!(promotion.relaxed_constraint_id(), ConstraintID::from(10));
            assert!(instance.constraints().is_empty());
            assert_eq!(
                instance.one_hot_constraints()[&OneHotConstraintID::from(0)].variables,
                variables([1, 2, 3])
            );

            let (_, reason) = &instance.removed_constraints()[&ConstraintID::from(10)];
            assert_eq!(reason.reason, PROMOTION_REASON);
            assert_eq!(
                reason
                    .parameters
                    .get(TARGET_ID_PARAMETER)
                    .map(String::as_str),
                Some("0")
            );
            assert_eq!(
                instance
                    .constraint_collection()
                    .context()
                    .collect_for(ConstraintID::from(10)),
                context
            );
            assert_eq!(
                instance
                    .one_hot_constraint_context()
                    .collect_for(OneHotConstraintID::from(0)),
                context
            );
        }
    }

    #[test]
    fn rejects_missing_wrong_relation_support_or_variable_domain_atomically() {
        let exact = Constraint::equal_to_zero(exact_scaled_one_hot(&[1, 2, 3], 1.0));
        assert_atomic_rejection(
            instance_with_source(exact.clone()),
            &OneHotPromotionRequest {
                source_constraint_id: ConstraintID::from(99),
                variables: variables([1, 2, 3]),
            },
            "was not found",
        );
        assert_atomic_rejection(
            instance_with_source(Constraint::less_than_or_equal_to_zero(
                exact_scaled_one_hot(&[1, 2, 3], 1.0),
            )),
            &request([1, 2, 3]),
            "not an equality-to-zero",
        );
        assert_atomic_rejection(
            instance_with_source(exact.clone()),
            &request([]),
            "at least one member",
        );
        assert_atomic_rejection(
            instance_with_source(exact),
            &request([1, 2]),
            "linear support",
        );
        assert_atomic_rejection(
            instance_with_source(Constraint::equal_to_zero(exact_scaled_one_hot(
                &[1, 2, 4],
                1.0,
            ))),
            &request([1, 2, 4]),
            "must be binary",
        );
    }

    #[test]
    fn rejects_non_exact_coefficients_constant_and_nonlinear_function_atomically() {
        let unequal = Function::from(
            ((linear!(1) + (coeff!(2.0) * linear!(2)).unwrap()).unwrap() + coeff!(-1.0)).unwrap(),
        );
        assert_atomic_rejection(
            instance_with_source(Constraint::equal_to_zero(unequal)),
            &request([1, 2]),
            "common coefficient",
        );

        let wrong_constant =
            Function::from(((linear!(1) + linear!(2)).unwrap() + coeff!(-2.0)).unwrap());
        assert_atomic_rejection(
            instance_with_source(Constraint::equal_to_zero(wrong_constant)),
            &request([1, 2]),
            "constant coefficient",
        );

        let nonlinear = Function::from(
            (((quadratic!(1) + quadratic!(2)).unwrap() + quadratic!(1, 2)).unwrap() + coeff!(-1.0))
                .unwrap(),
        );
        assert_atomic_rejection(
            instance_with_source(Constraint::equal_to_zero(nonlinear)),
            &request([1, 2]),
            "not exactly linear",
        );
    }
}
