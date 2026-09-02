//! Checked promotion of exact regular equalities to one-hot constraints.
//!
//! A promotion request is deliberately untrusted. It carries only stable IDs
//! and claimed one-hot membership; the current [`Instance`] remains the source
//! of truth for the regular row and decision-variable domains. Promotion is
//! allowed only when the active source is exactly a non-zero scalar multiple of
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

/// Stable-ID mapping produced by one checked one-hot promotion.
///
/// All promoted constraint data remains queryable from the mutated
/// [`Instance`]; this value records only which regular source became which
/// first-class OneHot constraint.
#[must_use = "the result identifies the source-to-OneHot promotion mapping"]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OneHotPromotion {
    source_constraint_id: ConstraintID,
    one_hot_constraint_id: OneHotConstraintID,
}

impl OneHotPromotion {
    /// Regular source constraint moved from active to removed.
    pub fn source_constraint_id(&self) -> ConstraintID {
        self.source_constraint_id
    }

    /// ID allocated to the promoted one-hot constraint.
    pub fn one_hot_constraint_id(&self) -> OneHotConstraintID {
        self.one_hot_constraint_id
    }
}

/// Individually validated OneHot candidate before batch compatibility and
/// target-ID allocation.
///
/// This intermediate state separates fallible request validation from
/// cross-request conflict resolution. Target IDs are deliberately absent;
/// only the batch plan allocates them after rejecting duplicate source claims.
#[derive(Debug)]
struct CheckedOneHotPromotionCandidate {
    source_constraint_id: ConstraintID,
    one_hot_constraint: OneHotConstraint,
    context: ConstraintContext,
}

/// Fully validated entry in one OneHot promotion batch.
#[derive(Debug)]
struct OneHotPromotionPlanEntry {
    result: OneHotPromotion,
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
    entries: Vec<(usize, OneHotPromotionPlanEntry)>,
    rejections: Vec<Option<crate::Error>>,
}

impl<'a> OneHotPromotionBatchPlan<'a> {
    fn new(instance: &'a mut Instance, requests: &[OneHotPromotionRequest]) -> Self {
        let mut checked = Vec::with_capacity(requests.len());
        let mut rejections = Vec::with_capacity(requests.len());
        for request in requests {
            match instance.check_one_hot_promotion(request) {
                Ok(candidate) => {
                    checked.push(Some(candidate));
                    rejections.push(None);
                }
                Err(error) => {
                    checked.push(None);
                    rejections.push(Some(error));
                }
            }
        }

        let mut source_claimants = BTreeMap::<ConstraintID, Vec<usize>>::new();
        for (index, candidate) in checked.iter().enumerate() {
            if let Some(candidate) = candidate {
                source_claimants
                    .entry(candidate.source_constraint_id)
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
        if let Err(error) = instance
            .one_hot_constraint_collection
            .ensure_unused_id_capacity(survivor_count)
        {
            let message = error.to_string();
            for (index, candidate) in checked.iter_mut().enumerate() {
                if candidate.take().is_some() {
                    rejections[index] = Some(crate::error!(
                        { index, survivor_count },
                        "Cannot allocate OneHot constraint IDs for the compatible promotion batch: {message}"
                    ));
                }
            }
            return Self {
                instance,
                entries: Vec::new(),
                rejections,
            };
        }

        let first_id = (survivor_count > 0).then(|| {
            instance
                .one_hot_constraint_collection
                .unused_id()
                .into_inner()
        });
        let mut offset = 0_u64;
        let mut entries = Vec::with_capacity(survivor_count);
        for (index, candidate) in checked.into_iter().enumerate() {
            let Some(candidate) = candidate else {
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
                OneHotPromotionPlanEntry {
                    result: OneHotPromotion {
                        source_constraint_id: candidate.source_constraint_id,
                        one_hot_constraint_id,
                    },
                    one_hot_constraint: candidate.one_hot_constraint,
                    context: candidate.context,
                },
            ));
        }

        Self {
            instance,
            entries,
            rejections,
        }
    }

    fn apply(self) -> Vec<crate::Result<OneHotPromotion>> {
        let Self {
            instance,
            entries,
            rejections,
        } = self;
        let mut outcomes = rejections
            .into_iter()
            .map(|error| error.map(Err))
            .collect::<Vec<_>>();

        if entries.is_empty() {
            return outcomes
                .into_iter()
                .map(|outcome| outcome.expect("every OneHot request must receive one outcome"))
                .collect();
        }

        let removal_reasons = entries
            .iter()
            .map(|(_, entry)| {
                (
                    entry.result.source_constraint_id,
                    RemovedReason {
                        reason: PROMOTION_REASON.to_string(),
                        parameters: [(
                            TARGET_ID_PARAMETER.to_string(),
                            entry.result.one_hot_constraint_id.into_inner().to_string(),
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

        for (index, entry) in entries {
            instance
                .one_hot_constraint_collection
                .insert_active_with_context(
                    entry.result.one_hot_constraint_id,
                    entry.one_hot_constraint,
                    entry.context,
                )
                .expect(
                    "target IDs, member IDs, and bound Instance were validated by OneHotPromotionBatchPlan",
                );
            debug_assert!(outcomes[index].is_none());
            outcomes[index] = Some(Ok(entry.result));
        }

        outcomes
            .into_iter()
            .map(|outcome| outcome.expect("every OneHot request must receive one outcome"))
            .collect()
    }
}

impl Instance {
    /// Promote compatible exact regular equalities to one-hot constraints as a
    /// single batch.
    ///
    /// The requests are untrusted. This method verifies each candidate against
    /// the current instance, requiring that:
    ///
    /// - the claimed member set is non-empty;
    /// - the source is an active regular equality;
    /// - its function is exactly linear;
    /// - its support is exactly the claimed member set;
    /// - every member is a binary decision variable; and
    /// - for one common non-zero coefficient `c`, the row is exactly
    ///   `c * (sum(variables) - 1) = 0`.
    ///
    /// Results remain aligned with `requests`. Individually invalid requests
    /// and every request sharing a source row with another individually valid
    /// request are rejected, while compatible requests are all applied.
    ///
    /// On each successful entry the source moves to `removed_constraints`, its
    /// context is copied to the new active one-hot constraint, and the removal
    /// reason records the allocated target ID. Planning is atomic: rejected
    /// requests leave their rows unchanged. The private plan exclusively
    /// borrows this instance until its infallible Apply consumes the plan.
    pub fn promote_one_hot(
        &mut self,
        requests: &[OneHotPromotionRequest],
    ) -> Vec<crate::Result<OneHotPromotion>> {
        OneHotPromotionBatchPlan::new(self, requests).apply()
    }

    fn check_one_hot_promotion(
        &self,
        request: &OneHotPromotionRequest,
    ) -> crate::Result<CheckedOneHotPromotionCandidate> {
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

        Ok(CheckedOneHotPromotionCandidate {
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
        let error = instance
            .promote_one_hot(std::slice::from_ref(request))
            .pop()
            .unwrap()
            .unwrap_err();
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

            let promotion = instance
                .promote_one_hot(&[request([1, 2, 3])])
                .pop()
                .unwrap()
                .unwrap();

            assert_eq!(
                promotion.one_hot_constraint_id(),
                OneHotConstraintID::from(0)
            );
            assert_eq!(promotion.source_constraint_id(), ConstraintID::from(10));
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

        let outcomes = instance.promote_one_hot(&requests);

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

        let outcomes = instance.promote_one_hot(&requests);

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
    fn batch_promotes_valid_requests_while_rejecting_individually_invalid_requests() {
        let (mut instance, requests) = batch_instance();
        let requests = vec![
            OneHotPromotionRequest {
                source_constraint_id: ConstraintID::from(99),
                variables: variables([1, 2]),
            },
            requests[1].clone(),
        ];

        let outcomes = instance.promote_one_hot(&requests);

        assert_eq!(outcomes.len(), 2);
        assert!(outcomes[0]
            .as_ref()
            .unwrap_err()
            .to_string()
            .contains("was not found"));
        assert_eq!(
            outcomes[1].as_ref().unwrap().one_hot_constraint_id(),
            OneHotConstraintID::from(0)
        );
        assert!(instance.constraints().contains_key(&ConstraintID::from(10)));
        assert!(!instance.constraints().contains_key(&ConstraintID::from(20)));
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

        let outcomes = instance.promote_one_hot(&requests);

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
