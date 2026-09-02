//! Checked promotion of exact regular equalities to one-hot constraints.
//!
//! A promotion request is deliberately untrusted. It carries only stable IDs
//! and claimed one-hot membership; the current [`Instance`] remains the source
//! of truth for the regular row and decision-variable domains. Promotion is
//! allowed only when the active source is exactly a non-zero scalar multiple of
//! `sum(variables) - 1 = 0` over binary variables.
//!
//! Even the single-request API uses a private batch plan. All candidates are
//! checked against one unchanged source instance before target IDs are
//! allocated. The batch plan proves that every aggregate storage effect must
//! succeed, so applying it never clones the [`Instance`] and cannot return a
//! recoverable error.

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

/// Instance-validated OneHot formulation before target-ID allocation.
///
/// Multiple requests are checked against the same source snapshot, so target
/// IDs are deliberately absent here. Only batch planning may allocate them.
#[derive(Debug)]
struct CheckedOneHotFormulation {
    variables: VariableIDSet,
    source_constraint_id: ConstraintID,
    one_hot_constraint: OneHotConstraint,
    context: ConstraintContext,
}

/// Fully validated, instance-bound plan for one OneHot promotion.
///
/// This value is constructed only while building a
/// [`OneHotPromotionBatchPlan`] and is applied as part of that aggregate plan
/// to the same, otherwise-unmodified [`Instance`].
#[derive(Debug)]
struct OneHotPromotionPlan {
    result: OneHotPromotion,
    one_hot_constraint: OneHotConstraint,
    context: ConstraintContext,
}

/// Aggregate proof object for applying compatible OneHot promotions.
///
/// # Invariants
///
/// - every source row was active on one unchanged source [`Instance`];
/// - source row IDs are pairwise distinct;
/// - target OneHot IDs are pairwise distinct and absent from both active and
///   removed OneHot collections;
/// - every structural constraint is non-empty and all of its members are
///   registered Binary variables; and
/// - every source context was captured before mutation.
///
/// The plan is private, constructed and immediately consumed by the OneHot
/// owner module. Consequently no caller can stale it between checking and
/// applying it, and aggregate Apply cannot produce a recoverable error.
#[derive(Debug)]
struct OneHotPromotionBatchPlan {
    entries: Vec<(usize, OneHotPromotionPlan)>,
}

#[derive(Debug)]
struct OneHotPromotionBatchPlanning {
    plan: OneHotPromotionBatchPlan,
    rejections: Vec<Option<crate::Error>>,
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
        self.promote_compatible_one_hot(std::slice::from_ref(request))
            .pop()
            .expect("one request must produce one aligned OneHot promotion outcome")
    }

    /// Check, reconcile, and apply OneHot promotion requests as one batch.
    ///
    /// Results remain aligned with the input requests. Individually invalid
    /// requests and every request sharing a source row with another
    /// individually valid request are rejected. All remaining requests are
    /// applied together through one [`OneHotPromotionBatchPlan`].
    fn promote_compatible_one_hot(
        &mut self,
        requests: &[OneHotPromotionRequest],
    ) -> Vec<crate::Result<OneHotPromotion>> {
        let OneHotPromotionBatchPlanning { plan, rejections } =
            self.plan_compatible_one_hot_promotions(requests);
        let mut outcomes = rejections
            .into_iter()
            .map(|error| error.map(Err))
            .collect::<Vec<_>>();

        for (index, promotion) in self.apply_one_hot_promotion_batch(plan) {
            debug_assert!(outcomes[index].is_none());
            outcomes[index] = Some(Ok(promotion));
        }

        outcomes
            .into_iter()
            .map(|outcome| outcome.expect("every OneHot request must receive one outcome"))
            .collect()
    }

    fn apply_one_hot_promotion_batch(
        &mut self,
        plan: OneHotPromotionBatchPlan,
    ) -> Vec<(usize, OneHotPromotion)> {
        if plan.entries.is_empty() {
            return Vec::new();
        }

        let removal_reasons = plan
            .entries
            .iter()
            .map(|(_, plan)| {
                (
                    plan.result.relaxed_constraint_id,
                    RemovedReason {
                        reason: PROMOTION_REASON.to_string(),
                        parameters: [(
                            TARGET_ID_PARAMETER.to_string(),
                            plan.result.one_hot_constraint_id.into_inner().to_string(),
                        )]
                        .into_iter()
                        .collect(),
                    },
                )
            })
            .collect();
        self.constraint_collection
            .move_active_rows_to_removed_with_reasons(removal_reasons)
            .expect("source rows were validated by OneHotPromotionBatchPlan");

        let mut promotions = Vec::with_capacity(plan.entries.len());
        for (index, plan) in plan.entries {
            self.one_hot_constraint_collection
                .insert_active_with_context(
                    plan.result.one_hot_constraint_id,
                    plan.one_hot_constraint,
                    plan.context,
                )
                .expect("target IDs and member IDs were validated by OneHotPromotionBatchPlan");
            promotions.push((index, plan.result));
        }
        promotions
    }

    fn plan_compatible_one_hot_promotions(
        &self,
        requests: &[OneHotPromotionRequest],
    ) -> OneHotPromotionBatchPlanning {
        let mut checked = Vec::with_capacity(requests.len());
        let mut rejections = Vec::with_capacity(requests.len());
        for request in requests {
            match self.check_one_hot_promotion(request) {
                Ok(formulation) => {
                    checked.push(Some(formulation));
                    rejections.push(None);
                }
                Err(error) => {
                    checked.push(None);
                    rejections.push(Some(error));
                }
            }
        }

        let mut source_claimants = BTreeMap::<ConstraintID, Vec<usize>>::new();
        for (index, formulation) in checked.iter().enumerate() {
            if let Some(formulation) = formulation {
                source_claimants
                    .entry(formulation.source_constraint_id)
                    .or_default()
                    .push(index);
            }
        }
        for (source_constraint_id, claimants) in source_claimants {
            if claimants.len() <= 1 {
                continue;
            }
            for index in claimants {
                checked[index] = None;
                rejections[index] = Some(crate::error!(
                    { index, ?source_constraint_id },
                    "OneHot promotion request at index {index} conflicts with another individually valid request over source row {source_constraint_id:?}"
                ));
            }
        }

        let survivor_count = checked.iter().filter(|entry| entry.is_some()).count();
        if let Err(error) = self
            .one_hot_constraint_collection
            .ensure_unused_id_capacity(survivor_count)
        {
            let message = error.to_string();
            for (index, formulation) in checked.iter_mut().enumerate() {
                if formulation.take().is_some() {
                    rejections[index] = Some(crate::error!(
                        { index, survivor_count },
                        "Cannot allocate OneHot constraint IDs for the compatible promotion batch: {message}"
                    ));
                }
            }
            return OneHotPromotionBatchPlanning {
                plan: OneHotPromotionBatchPlan {
                    entries: Vec::new(),
                },
                rejections,
            };
        }

        let first_id = (survivor_count > 0)
            .then(|| self.one_hot_constraint_collection.unused_id().into_inner());
        let mut offset = 0_u64;
        let mut entries = Vec::with_capacity(survivor_count);
        for (index, formulation) in checked.into_iter().enumerate() {
            let Some(formulation) = formulation else {
                continue;
            };
            let one_hot_constraint_id = OneHotConstraintID::from(
                first_id
                    .expect("a non-empty compatible batch has a first OneHot ID")
                    .checked_add(offset)
                    .expect("batch ID capacity was validated before allocation"),
            );
            offset += 1;
            entries.push((
                index,
                OneHotPromotionPlan {
                    result: OneHotPromotion {
                        one_hot_constraint_id,
                        variables: formulation.variables,
                        relaxed_constraint_id: formulation.source_constraint_id,
                    },
                    one_hot_constraint: formulation.one_hot_constraint,
                    context: formulation.context,
                },
            ));
        }

        OneHotPromotionBatchPlanning {
            plan: OneHotPromotionBatchPlan { entries },
            rejections,
        }
    }

    fn check_one_hot_promotion(
        &self,
        request: &OneHotPromotionRequest,
    ) -> crate::Result<CheckedOneHotFormulation> {
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

        let one_hot_constraint = OneHotConstraint::new(request.variables.clone())?;
        let context = self
            .constraint_collection
            .context()
            .collect_for(request.source_constraint_id);

        Ok(CheckedOneHotFormulation {
            variables: request.variables.clone(),
            source_constraint_id: request.source_constraint_id,
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

    fn batch_instance() -> (Instance, Vec<OneHotPromotionRequest>) {
        let requests = vec![
            OneHotPromotionRequest {
                source_constraint_id: ConstraintID::from(10),
                variables: variables([1, 2]),
            },
            OneHotPromotionRequest {
                source_constraint_id: ConstraintID::from(20),
                variables: variables([2, 3]),
            },
        ];
        let instance = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::Zero)
            .decision_variables(BTreeMap::from([
                (VariableID::from(1), DecisionVariable::binary()),
                (VariableID::from(2), DecisionVariable::binary()),
                (VariableID::from(3), DecisionVariable::binary()),
            ]))
            .constraints(BTreeMap::from([
                (
                    ConstraintID::from(10),
                    Constraint::equal_to_zero(exact_scaled_one_hot(&[1, 2], 1.0)),
                ),
                (
                    ConstraintID::from(20),
                    Constraint::equal_to_zero(exact_scaled_one_hot(&[2, 3], -2.0)),
                ),
            ]))
            .build()
            .unwrap();
        (instance, requests)
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
        let regular_before = instance.constraint_collection.clone();
        let one_hot_before = instance.one_hot_constraint_collection.clone();
        let error = instance.promote_one_hot(request).unwrap_err();
        assert!(
            error.to_string().contains(message),
            "unexpected error: {error:#}"
        );
        assert_eq!(instance.constraint_collection, regular_before);
        assert_eq!(instance.one_hot_constraint_collection, one_hot_before);
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
    fn batch_promotes_disjoint_rows_with_a_shared_member() {
        let (mut instance, requests) = batch_instance();

        let outcomes = instance.promote_compatible_one_hot(&requests);

        let promotions = outcomes
            .iter()
            .map(|outcome| outcome.as_ref().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(
            promotions
                .iter()
                .map(|promotion| promotion.one_hot_constraint_id())
                .collect::<Vec<_>>(),
            vec![OneHotConstraintID::from(0), OneHotConstraintID::from(1)]
        );
        assert!(instance.constraints().is_empty());
        assert_eq!(instance.removed_constraints().len(), 2);
        assert_eq!(instance.one_hot_constraints().len(), 2);
        assert_eq!(
            instance.one_hot_constraints()[&OneHotConstraintID::from(0)].variables,
            variables([1, 2])
        );
        assert_eq!(
            instance.one_hot_constraints()[&OneHotConstraintID::from(1)].variables,
            variables([2, 3])
        );
    }

    #[test]
    fn batch_rejects_all_duplicate_source_claims_and_promotes_unrelated_request() {
        let (mut instance, requests) = batch_instance();
        let requests = vec![
            requests[0].clone(),
            requests[0].clone(),
            requests[1].clone(),
        ];

        let outcomes = instance.promote_compatible_one_hot(&requests);

        assert!(outcomes[0]
            .as_ref()
            .unwrap_err()
            .to_string()
            .contains("conflicts with another individually valid request"));
        assert!(outcomes[1].is_err());
        assert_eq!(
            outcomes[2].as_ref().unwrap().one_hot_constraint_id(),
            OneHotConstraintID::from(0)
        );
        assert!(instance.constraints().contains_key(&ConstraintID::from(10)));
        assert!(instance
            .removed_constraints()
            .contains_key(&ConstraintID::from(20)));
        assert_eq!(instance.one_hot_constraints().len(), 1);
    }

    #[test]
    fn batch_rejects_id_exhaustion_before_any_mutation() {
        let (mut instance, requests) = batch_instance();
        instance
            .one_hot_constraint_collection
            .insert_active_with_context(
                OneHotConstraintID::from(u64::MAX),
                OneHotConstraint::new(variables([1])).unwrap(),
                ConstraintContext::default(),
            )
            .unwrap();
        let regular_before = instance.constraint_collection.clone();
        let one_hot_before = instance.one_hot_constraint_collection.clone();

        let outcomes = instance.promote_compatible_one_hot(&requests);

        assert!(outcomes.iter().all(Result::is_err));
        assert!(outcomes.iter().all(|outcome| outcome
            .as_ref()
            .unwrap_err()
            .to_string()
            .contains("Cannot allocate OneHot constraint IDs")));
        assert_eq!(instance.constraint_collection, regular_before);
        assert_eq!(instance.one_hot_constraint_collection, one_hot_before);
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
