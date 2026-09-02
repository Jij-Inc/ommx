//! Checked promotion of exact regular equalities to one-hot constraints.
//!
//! A promotion request carries only stable regular-constraint IDs. The current
//! [`Instance`] remains the source of truth for the row, its inferred one-hot
//! membership, and the decision-variable domains. Promotion is allowed only
//! when the active source is exactly a non-zero scalar multiple of
//! `sum(variables) - 1 = 0` over binary variables.
//!
//! The public API is batch-oriented. All candidates are checked against one
//! unchanged source instance before target IDs are allocated. The batch plan
//! proves that every aggregate storage effect must succeed, so applying it
//! never clones the [`Instance`] and cannot return a recoverable error.

use super::Instance;
use crate::{
    Coefficient, ConstraintContext, ConstraintID, Equality, Kind, LinearMonomial, OneHotConstraint,
    OneHotConstraintID, RemovedReason, VariableID, VariableIDSet,
};
use std::collections::{BTreeMap, BTreeSet};

const PROMOTION_REASON: &str = "promoted validated one-hot equality";
const TARGET_ID_PARAMETER: &str = "one_hot_constraint_id";

/// Stable-ID request for a batch of one-hot promotion attempts.
///
/// Each ID selects an active regular constraint to inspect. The current
/// [`Instance`] determines the member set from the row itself and validates its
/// relation, coefficients, and current variable domains before mutation.
///
/// The set representation makes duplicate source claims unrepresentable. No
/// target IDs are supplied; fresh [`OneHotConstraintID`] values are allocated
/// together only after every source has been inspected.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct OneHotPromotionRequest {
    constraint_ids: BTreeSet<ConstraintID>,
}

impl OneHotPromotionRequest {
    /// Construct a request from regular constraint IDs to inspect.
    pub fn new(constraint_ids: impl IntoIterator<Item = ConstraintID>) -> Self {
        Self {
            constraint_ids: constraint_ids.into_iter().collect(),
        }
    }

    /// Regular constraint IDs requested for promotion.
    pub fn constraint_ids(&self) -> &BTreeSet<ConstraintID> {
        &self.constraint_ids
    }

    /// Convert one legacy v1 one-hot hint into an untrusted request.
    ///
    /// This conversion is independent of any [`Instance`] and uses only the
    /// referenced regular constraint ID. Legacy `decision_variables` are
    /// advisory data rather than a certificate; membership is re-derived from
    /// the current source row by [`Instance::promote_one_hot`]. Constraint ID
    /// `0` is a valid stable ID.
    pub fn from_v1_hint(hint: &crate::v1::OneHot) -> Self {
        Self::new([ConstraintID::from(hint.constraint_id)])
    }
}

impl FromIterator<ConstraintID> for OneHotPromotionRequest {
    fn from_iter<T: IntoIterator<Item = ConstraintID>>(iter: T) -> Self {
        Self::new(iter)
    }
}

/// Individually validated OneHot candidate before target-ID allocation.
///
/// This intermediate state separates fallible source-row validation from
/// aggregate ID allocation. Target IDs are deliberately absent; only the batch
/// plan allocates them.
#[derive(Debug)]
struct CheckedOneHotPromotionCandidate {
    one_hot_constraint: OneHotConstraint,
    context: ConstraintContext,
}

/// Fully planned OneHot insertion for one validated source row.
#[derive(Debug)]
struct PlannedOneHot {
    one_hot_constraint_id: OneHotConstraintID,
    one_hot_constraint: OneHotConstraint,
    context: ConstraintContext,
}

/// Aggregate proof object for applying compatible OneHot promotions.
///
/// # Invariants
///
/// - `instance` is the exact [`Instance`] against which every entry was
///   validated, and its exclusive borrow prevents any mutation before Apply;
/// - every source row remains active in that instance;
/// - source row IDs are pairwise distinct;
/// - target OneHot IDs are pairwise distinct and absent from both active and
///   removed OneHot collections;
/// - every structural constraint is non-empty and all of its members are
///   registered Binary variables; and
/// - every source context was captured before mutation.
///
/// The plan is private and Apply consumes it. It cannot be applied to another
/// instance or become stale between checking and mutation, so every aggregate
/// storage effect is infallible under these invariants.
#[derive(Debug)]
struct OneHotPromotionBatchPlan<'a> {
    instance: &'a mut Instance,
    entries: BTreeMap<ConstraintID, PlannedOneHot>,
    rejections: BTreeMap<ConstraintID, crate::Error>,
}

impl<'a> OneHotPromotionBatchPlan<'a> {
    fn new(instance: &'a mut Instance, request: &OneHotPromotionRequest) -> Self {
        let mut checked = BTreeMap::new();
        let mut rejections = BTreeMap::new();
        for &source_constraint_id in request.constraint_ids() {
            match instance.check_one_hot_promotion(source_constraint_id) {
                Ok(candidate) => {
                    checked.insert(source_constraint_id, candidate);
                }
                Err(error) => {
                    rejections.insert(source_constraint_id, error);
                }
            }
        }

        let survivor_count = checked.len();
        if let Err(error) = instance
            .one_hot_constraint_collection
            .ensure_unused_id_capacity(survivor_count)
        {
            let message = error.to_string();
            for source_constraint_id in checked.keys().copied() {
                rejections.insert(source_constraint_id, crate::error!(
                        { ?source_constraint_id, survivor_count },
                        "Cannot allocate OneHot constraint IDs for the compatible promotion batch: {message}"
                    ));
            }
            return Self {
                instance,
                entries: BTreeMap::new(),
                rejections,
            };
        }

        let first_id = (survivor_count > 0).then(|| {
            instance
                .one_hot_constraint_collection
                .unused_id()
                .into_inner()
        });
        let mut entries = BTreeMap::new();
        for (offset, (source_constraint_id, candidate)) in checked.into_iter().enumerate() {
            let offset = u64::try_from(offset)
                .expect("batch size was converted to u64 during ID capacity validation");
            let one_hot_constraint_id = OneHotConstraintID::from(
                first_id
                    .expect("a non-empty compatible batch has a first OneHot ID")
                    .checked_add(offset)
                    .expect("batch ID capacity was validated before allocation"),
            );
            entries.insert(
                source_constraint_id,
                PlannedOneHot {
                    one_hot_constraint_id,
                    one_hot_constraint: candidate.one_hot_constraint,
                    context: candidate.context,
                },
            );
        }

        Self {
            instance,
            entries,
            rejections,
        }
    }

    fn apply(self) -> BTreeMap<ConstraintID, crate::Result<OneHotConstraintID>> {
        let Self {
            instance,
            entries,
            rejections,
        } = self;

        let mut results: BTreeMap<_, _> = rejections
            .into_iter()
            .map(|(source_constraint_id, error)| (source_constraint_id, Err(error)))
            .collect();

        if entries.is_empty() {
            return results;
        }

        let removal_reasons = entries
            .iter()
            .map(|(&source_constraint_id, planned)| {
                (
                    source_constraint_id,
                    RemovedReason {
                        reason: PROMOTION_REASON.to_string(),
                        parameters: [(
                            TARGET_ID_PARAMETER.to_string(),
                            planned.one_hot_constraint_id.into_inner().to_string(),
                        )]
                        .into_iter()
                        .collect(),
                    },
                )
            })
            .collect();
        instance
            .constraint_collection
            .move_active_rows_to_removed_with_reasons(removal_reasons)
            .expect("source rows and bound Instance were validated by OneHotPromotionBatchPlan");

        for (source_constraint_id, planned) in entries {
            instance
                .one_hot_constraint_collection
                .insert_active_with_context(
                    planned.one_hot_constraint_id,
                    planned.one_hot_constraint,
                    planned.context,
                )
                .expect(
                    "target IDs, member IDs, and bound Instance were validated by OneHotPromotionBatchPlan",
                );
            let previous = results.insert(source_constraint_id, Ok(planned.one_hot_constraint_id));
            debug_assert!(previous.is_none());
        }

        results
    }
}

impl Instance {
    /// Promote compatible exact regular equalities to one-hot constraints as a
    /// single batch.
    ///
    /// The request IDs are untrusted. This method verifies each source against
    /// the current instance, requiring that:
    ///
    /// - the source is an active regular equality;
    /// - its function is exactly linear;
    /// - its support is non-empty and every member is a binary decision
    ///   variable; and
    /// - for one common non-zero coefficient `c`, the row is exactly
    ///   `c * (sum(variables) - 1) = 0`.
    ///
    /// The returned map has exactly one entry per requested source ID. Each
    /// value is either the allocated OneHot ID or that source's rejection
    /// reason. An invalid source does not prevent independent valid sources in
    /// the same request from being promoted.
    ///
    /// On each successful entry the source moves to `removed_constraints`, its
    /// context is copied to the new active one-hot constraint, and the removal
    /// reason records the allocated target ID. Planning is atomic: rejected
    /// requests leave their rows unchanged. The private plan exclusively
    /// borrows this instance until its infallible Apply consumes the plan.
    #[must_use = "each requested source has a success or rejection result"]
    pub fn promote_one_hot(
        &mut self,
        request: &OneHotPromotionRequest,
    ) -> BTreeMap<ConstraintID, crate::Result<OneHotConstraintID>> {
        OneHotPromotionBatchPlan::new(self, request).apply()
    }

    fn check_one_hot_promotion(
        &self,
        source_constraint_id: ConstraintID,
    ) -> crate::Result<CheckedOneHotPromotionCandidate> {
        let source = self
            .constraint_collection
            .active()
            .get(&source_constraint_id)
            .ok_or_else(|| {
                crate::error!(
                    { ?source_constraint_id },
                    "Active regular constraint {:?} was not found",
                    source_constraint_id
                )
            })?;
        if source.equality != Equality::EqualToZero {
            crate::bail!(
                { ?source_constraint_id },
                "Regular constraint {:?} is not an equality-to-zero constraint",
                source_constraint_id
            );
        }

        let linear = source.function().as_linear().ok_or_else(|| {
            crate::error!(
                { ?source_constraint_id },
                "Regular constraint {:?} is not exactly linear",
                source_constraint_id
            )
        })?;
        let coefficients: BTreeMap<VariableID, Coefficient> = linear.linear_terms().collect();
        let support: VariableIDSet = coefficients.keys().copied().collect();
        if support.is_empty() {
            crate::bail!(
                { ?source_constraint_id },
                "Regular constraint {source_constraint_id:?} has no one-hot members"
            );
        }

        for &variable_id in &support {
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
                { ?source_constraint_id },
                "Regular constraint {:?} does not use one common coefficient for all one-hot members",
                source_constraint_id
            );
        }

        let constant = linear.get(&LinearMonomial::Constant);
        if constant != Some(-common_coefficient) {
            crate::bail!(
                {
                    ?source_constraint_id,
                    ?constant,
                    expected = ?(-common_coefficient)
                },
                "Regular constraint {:?} has constant coefficient {constant:?}, expected {:?}",
                source_constraint_id,
                -common_coefficient
            );
        }

        let one_hot_constraint = OneHotConstraint::new(support)?;
        let context = self
            .constraint_collection
            .context()
            .collect_for(source_constraint_id);

        Ok(CheckedOneHotPromotionCandidate {
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

    fn batch_instance() -> (Instance, OneHotPromotionRequest) {
        let request = OneHotPromotionRequest::new([ConstraintID::from(10), ConstraintID::from(20)]);
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
        (instance, request)
    }

    fn request(ids: impl IntoIterator<Item = u64>) -> OneHotPromotionRequest {
        OneHotPromotionRequest::new(ids.into_iter().map(ConstraintID::from))
    }

    fn assert_atomic_rejection(
        mut instance: Instance,
        source_constraint_id: ConstraintID,
        message: &str,
    ) {
        let regular_before = instance.constraint_collection.clone();
        let one_hot_before = instance.one_hot_constraint_collection.clone();
        let results =
            instance.promote_one_hot(&OneHotPromotionRequest::new([source_constraint_id]));
        assert_eq!(results.len(), 1);
        let error = results[&source_constraint_id].as_ref().unwrap_err();
        assert!(
            error.to_string().contains(message),
            "unexpected error: {error:#}"
        );
        assert_eq!(instance.constraint_collection, regular_before);
        assert_eq!(instance.one_hot_constraint_collection, one_hot_before);
    }

    #[test]
    fn converts_v1_hint_to_a_singleton_request_without_using_claimed_members() {
        for members in [vec![], vec![2, 1], vec![1, 1]] {
            let request = OneHotPromotionRequest::from_v1_hint(&crate::v1::OneHot {
                constraint_id: 7,
                decision_variables: members,
            });
            assert_eq!(
                request.constraint_ids(),
                &BTreeSet::from([ConstraintID::from(7)])
            );
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

            let results = instance.promote_one_hot(&request([10]));

            assert_eq!(
                results[&ConstraintID::from(10)].as_ref().unwrap(),
                &OneHotConstraintID::from(0)
            );
            assert_eq!(results.len(), 1);
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
        let (mut instance, request) = batch_instance();

        let results = instance.promote_one_hot(&request);

        assert_eq!(
            results[&ConstraintID::from(10)].as_ref().unwrap(),
            &OneHotConstraintID::from(0)
        );
        assert_eq!(
            results[&ConstraintID::from(20)].as_ref().unwrap(),
            &OneHotConstraintID::from(1)
        );
        assert_eq!(results.len(), 2);
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
    fn request_deduplicates_source_ids_by_construction() {
        let request = OneHotPromotionRequest::new([
            ConstraintID::from(10),
            ConstraintID::from(10),
            ConstraintID::from(20),
        ]);

        assert_eq!(
            request.constraint_ids(),
            &BTreeSet::from([ConstraintID::from(10), ConstraintID::from(20)])
        )
    }

    #[test]
    fn batch_promotes_valid_requests_while_rejecting_individually_invalid_requests() {
        let (mut instance, _) = batch_instance();
        let request = OneHotPromotionRequest::new([ConstraintID::from(99), ConstraintID::from(20)]);

        let results = instance.promote_one_hot(&request);

        assert_eq!(
            results.keys().copied().collect::<BTreeSet<_>>(),
            request.constraint_ids().clone()
        );
        assert_eq!(
            results[&ConstraintID::from(20)].as_ref().unwrap(),
            &OneHotConstraintID::from(0)
        );
        assert!(results[&ConstraintID::from(99)]
            .as_ref()
            .unwrap_err()
            .to_string()
            .contains("was not found"));
        assert_eq!(results.len(), 2);
        assert!(instance.constraints().contains_key(&ConstraintID::from(10)));
        assert!(!instance.constraints().contains_key(&ConstraintID::from(20)));
        assert!(instance
            .removed_constraints()
            .contains_key(&ConstraintID::from(20)));
        assert_eq!(instance.one_hot_constraints().len(), 1);
    }

    #[test]
    fn batch_rejects_id_exhaustion_before_any_mutation() {
        let (mut instance, request) = batch_instance();
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

        let results = instance.promote_one_hot(&request);

        assert_eq!(
            results.keys().copied().collect::<BTreeSet<_>>(),
            request.constraint_ids().clone()
        );
        assert!(results.values().all(|result| result
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
            ConstraintID::from(99),
            "was not found",
        );
        assert_atomic_rejection(
            instance_with_source(Constraint::less_than_or_equal_to_zero(
                exact_scaled_one_hot(&[1, 2, 3], 1.0),
            )),
            ConstraintID::from(10),
            "not an equality-to-zero",
        );
        assert_atomic_rejection(
            instance_with_source(Constraint::equal_to_zero(exact_scaled_one_hot(
                &[1, 2, 4],
                1.0,
            ))),
            ConstraintID::from(10),
            "must be binary",
        );

        let constant_only = Function::from(Linear::single_term(
            LinearMonomial::Constant,
            Coefficient::try_from(-1.0).unwrap(),
        ));
        assert_atomic_rejection(
            instance_with_source(Constraint::equal_to_zero(constant_only)),
            ConstraintID::from(10),
            "has no one-hot members",
        );
    }

    #[test]
    fn rejects_non_exact_coefficients_constant_and_nonlinear_function_atomically() {
        let unequal = Function::from(
            ((linear!(1) + (coeff!(2.0) * linear!(2)).unwrap()).unwrap() + coeff!(-1.0)).unwrap(),
        );
        assert_atomic_rejection(
            instance_with_source(Constraint::equal_to_zero(unequal)),
            ConstraintID::from(10),
            "common coefficient",
        );

        let wrong_constant =
            Function::from(((linear!(1) + linear!(2)).unwrap() + coeff!(-2.0)).unwrap());
        assert_atomic_rejection(
            instance_with_source(Constraint::equal_to_zero(wrong_constant)),
            ConstraintID::from(10),
            "constant coefficient",
        );

        let nonlinear = Function::from(
            (((quadratic!(1) + quadratic!(2)).unwrap() + quadratic!(1, 2)).unwrap() + coeff!(-1.0))
                .unwrap(),
        );
        assert_atomic_rejection(
            instance_with_source(Constraint::equal_to_zero(nonlinear)),
            ConstraintID::from(10),
            "not exactly linear",
        );
    }
}
