use super::*;
use crate::{
    constraint::Equality, decision_variable::Kind, Coefficient, LinearMonomial, OneHotConstraintID,
};
use std::collections::{BTreeMap, BTreeSet};

/// A detector-supplied witness for promoting a regular constraint.
///
/// Certificates are never trusted as mutation plans. [`Instance`] recomputes
/// the represented mathematical condition against its current state whenever
/// a certificate is checked or applied.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PromotionCertificate {
    /// Promote an exact linear equality over binary variables to one-hot form.
    OneHot(OneHotPromotionCertificate),
}

// Data-carrying enums are not supported by the derive. Delegate to the
// certificate payload so its variable-set allocation remains visible.
impl crate::logical_memory::LogicalMemoryProfile for PromotionCertificate {
    fn visit_logical_memory<V: crate::logical_memory::LogicalMemoryVisitor>(
        &self,
        path: &mut crate::logical_memory::Path,
        visitor: &mut V,
    ) {
        match self {
            Self::OneHot(certificate) => {
                crate::logical_memory::LogicalMemoryProfile::visit_logical_memory(
                    certificate,
                    path.with("PromotionCertificate.OneHot").as_mut(),
                    visitor,
                );
            }
        }
    }
}

impl From<OneHotPromotionCertificate> for PromotionCertificate {
    fn from(certificate: OneHotPromotionCertificate) -> Self {
        Self::OneHot(certificate)
    }
}

/// A witness that an active regular constraint is exactly a one-hot constraint.
///
/// The fields are intentionally only claims. In particular, callers may
/// construct an empty variable set, refer to non-binary variables, or choose a
/// used target ID. [`Instance::check_promotion_certificate`] and the mutation
/// methods validate every invariant against the current instance.
#[derive(Debug, Clone, PartialEq, Eq, crate::logical_memory::LogicalMemoryProfile)]
pub struct OneHotPromotionCertificate {
    /// Active regular constraint to replace with a one-hot constraint.
    pub source_constraint_id: ConstraintID,
    /// Claimed one-hot members. The set representation makes membership unique.
    pub variables: BTreeSet<VariableID>,
    /// Requested one-hot ID, or [`None`] to allocate one during verification.
    pub target_one_hot_constraint_id: Option<OneHotConstraintID>,
}

/// Informational result of checking one promotion certificate.
///
/// A preview cannot be applied. The instance may change after it is returned,
/// so all mutation entry points verify the original certificate again.
#[derive(Debug, Clone, PartialEq, Eq, crate::logical_memory::LogicalMemoryProfile)]
pub struct PromotionPreview {
    source_constraint_id: ConstraintID,
    variables: BTreeSet<VariableID>,
    target_one_hot_constraint_id: OneHotConstraintID,
}

impl PromotionPreview {
    /// The active regular constraint that would be promoted.
    pub fn source_constraint_id(&self) -> ConstraintID {
        self.source_constraint_id
    }

    /// The exactly verified one-hot members.
    pub fn variables(&self) -> &BTreeSet<VariableID> {
        &self.variables
    }

    /// The requested or provisionally allocated target one-hot ID.
    pub fn target_one_hot_constraint_id(&self) -> OneHotConstraintID {
        self.target_one_hot_constraint_id
    }
}

/// Result of one successfully applied promotion.
#[derive(Debug, Clone, PartialEq, Eq, crate::logical_memory::LogicalMemoryProfile)]
pub struct PromotionResult {
    source_constraint_id: ConstraintID,
    target_one_hot_constraint_id: OneHotConstraintID,
}

impl PromotionResult {
    /// The regular constraint moved into `removed_constraints`.
    pub fn source_constraint_id(&self) -> ConstraintID {
        self.source_constraint_id
    }

    /// The newly active one-hot constraint.
    pub fn target_one_hot_constraint_id(&self) -> OneHotConstraintID {
        self.target_one_hot_constraint_id
    }
}

/// Result of an all-or-nothing bulk promotion.
#[derive(Debug, Clone, PartialEq, Eq, Default, crate::logical_memory::LogicalMemoryProfile)]
pub struct PromotionReport {
    source_to_target: BTreeMap<ConstraintID, OneHotConstraintID>,
}

impl PromotionReport {
    /// Mapping from every promoted regular constraint to its one-hot target.
    pub fn source_to_target(&self) -> &BTreeMap<ConstraintID, OneHotConstraintID> {
        &self.source_to_target
    }
}

/// Re-validated audit record for a previous one-hot promotion.
#[derive(Debug, Clone, PartialEq, Eq, crate::logical_memory::LogicalMemoryProfile)]
pub struct PromotionAudit {
    source_constraint_id: ConstraintID,
    variables: BTreeSet<VariableID>,
    target_one_hot_constraint_id: OneHotConstraintID,
    target_is_active: bool,
}

impl PromotionAudit {
    /// The removed regular source constraint.
    pub fn source_constraint_id(&self) -> ConstraintID {
        self.source_constraint_id
    }

    /// The original one-hot members reconstructed from the retained source.
    pub fn variables(&self) -> &BTreeSet<VariableID> {
        &self.variables
    }

    /// The target one-hot ID recorded in the source's removal metadata.
    pub fn target_one_hot_constraint_id(&self) -> OneHotConstraintID {
        self.target_one_hot_constraint_id
    }

    /// Whether the target one-hot constraint is active rather than removed.
    pub fn target_is_active(&self) -> bool {
        self.target_is_active
    }
}

#[derive(Debug, Clone)]
struct PlannedOneHotPromotion {
    source_constraint_id: ConstraintID,
    variables: BTreeSet<VariableID>,
    target_one_hot_constraint_id: OneHotConstraintID,
}

impl PlannedOneHotPromotion {
    fn preview(&self) -> PromotionPreview {
        PromotionPreview {
            source_constraint_id: self.source_constraint_id,
            variables: self.variables.clone(),
            target_one_hot_constraint_id: self.target_one_hot_constraint_id,
        }
    }

    fn result(&self) -> PromotionResult {
        PromotionResult {
            source_constraint_id: self.source_constraint_id,
            target_one_hot_constraint_id: self.target_one_hot_constraint_id,
        }
    }
}

impl Instance {
    /// Check a detector-supplied certificate without mutating this instance.
    ///
    /// `allowed` is the caller's capability boundary. A one-hot certificate is
    /// rejected unless it contains [`AdditionalCapability::OneHot`]. Any target
    /// ID in the returned preview is informational only; applying the
    /// certificate later repeats validation and allocation against the then
    /// current instance.
    pub fn check_promotion_certificate(
        &self,
        certificate: &PromotionCertificate,
        allowed: &Capabilities,
    ) -> crate::Result<PromotionPreview> {
        let plans = self.plan_promotions(std::slice::from_ref(certificate), allowed)?;
        Ok(plans
            .first()
            .expect("a one-element certificate slice produces one plan")
            .preview())
    }

    /// Verify and atomically apply one promotion certificate.
    ///
    /// Verification is repeated against the current instance. On error, the
    /// instance remains unchanged.
    pub fn promote_with_certificate(
        &mut self,
        certificate: PromotionCertificate,
        allowed: &Capabilities,
    ) -> crate::Result<PromotionResult> {
        let plans = self.plan_promotions(std::slice::from_ref(&certificate), allowed)?;
        let result = plans
            .first()
            .expect("a one-element certificate slice produces one plan")
            .result();
        self.apply_promotion_plans(&plans)?;
        Ok(result)
    }

    /// Verify and atomically apply multiple promotion certificates.
    ///
    /// All certificates are checked against one pre-promotion snapshot.
    /// Duplicate sources and targets are rejected. Explicit target IDs are
    /// reserved before omitted IDs are allocated, and any failure leaves this
    /// instance unchanged.
    pub fn promote_with_certificates(
        &mut self,
        certificates: Vec<PromotionCertificate>,
        allowed: &Capabilities,
    ) -> crate::Result<PromotionReport> {
        let plans = self.plan_promotions(&certificates, allowed)?;
        let source_to_target = plans
            .iter()
            .map(|plan| (plan.source_constraint_id, plan.target_one_hot_constraint_id))
            .collect();
        self.apply_promotion_plans(&plans)?;
        Ok(PromotionReport { source_to_target })
    }

    /// Re-validate the retained audit trail for a previous one-hot promotion.
    ///
    /// This method trusts neither the original detector nor its certificate. It
    /// reads the reserved `promotion.*` removal metadata, finds the target
    /// one-hot constraint in either lifecycle state, and checks exact
    /// equivalence with the retained regular source row again.
    pub fn verify_promotion_history(
        &self,
        source_constraint_id: ConstraintID,
    ) -> crate::Result<PromotionAudit> {
        let (source, removed_reason) = self
            .constraint_collection
            .removed()
            .get(&source_constraint_id)
            .ok_or_else(|| {
                crate::error!("Removed regular constraint {source_constraint_id:?} was not found")
            })?;
        let target_id = promoted_one_hot_target(removed_reason)?.ok_or_else(|| {
            crate::error!(
                "Removed regular constraint {source_constraint_id:?} is not a one-hot promotion"
            )
        })?;

        let (target, target_is_active) = if let Some(target) =
            self.one_hot_constraint_collection.active().get(&target_id)
        {
            (target, true)
        } else if let Some((target, _reason)) =
            self.one_hot_constraint_collection.removed().get(&target_id)
        {
            (target, false)
        } else {
            crate::bail!(
                "One-hot promotion target {target_id:?} recorded by source {source_constraint_id:?} was not found"
            );
        };

        let original_variables =
            self.verified_one_hot_source_variables(source_constraint_id, source)?;
        self.validate_one_hot_variables(&target.variables)?;
        if !target.variables.is_subset(&original_variables) {
            crate::bail!(
                "One-hot promotion target {target_id:?} contains variables outside the retained source support"
            );
        }

        // Unit propagation may shrink an active target by removing members
        // fixed exactly to zero. Reconstruct the target's lifecycle-normalized
        // source without substituting variables that remain in the target; a
        // consumed target can legitimately retain members now fixed to one or
        // zero in its removed payload.
        let removed_variables: BTreeSet<_> = original_variables
            .difference(&target.variables)
            .copied()
            .collect();
        for variable_id in &removed_variables {
            if self.fixed_decision_variable_value(*variable_id) != Some(0.0) {
                crate::bail!(
                    "One-hot promotion target {target_id:?} omits source variable {variable_id:?} that is not fixed exactly to zero"
                );
            }
        }
        let mut normalized_source = source.clone();
        if !removed_variables.is_empty() {
            let state = crate::v1::State::from(
                removed_variables
                    .iter()
                    .map(|id| (id.into_inner(), 0.0))
                    .collect::<std::collections::HashMap<_, _>>(),
            );
            normalized_source.partial_evaluate(&state, crate::ATol::default())?;
        }
        self.verify_one_hot_equivalence(
            source_constraint_id,
            &normalized_source,
            &target.variables,
        )?;
        Ok(PromotionAudit {
            source_constraint_id,
            variables: original_variables,
            target_one_hot_constraint_id: target_id,
            target_is_active,
        })
    }

    fn plan_promotions(
        &self,
        certificates: &[PromotionCertificate],
        allowed: &Capabilities,
    ) -> crate::Result<Vec<PlannedOneHotPromotion>> {
        let mut source_ids = BTreeSet::new();
        let mut target_ids: BTreeSet<OneHotConstraintID> = self
            .one_hot_constraint_collection
            .active()
            .keys()
            .chain(self.one_hot_constraint_collection.removed().keys())
            .copied()
            .collect();
        let existing_target_ids = target_ids.clone();

        // Reserve every explicit target before allocating any omitted target.
        for certificate in certificates {
            match certificate {
                PromotionCertificate::OneHot(certificate) => {
                    if !allowed.contains(&AdditionalCapability::OneHot) {
                        crate::bail!(
                            "One-hot promotion is outside the caller's allowed capabilities"
                        );
                    }
                    if !source_ids.insert(certificate.source_constraint_id) {
                        crate::bail!(
                            "Duplicate promotion source constraint ID {:?}",
                            certificate.source_constraint_id
                        );
                    }
                    if let Some(target_id) = certificate.target_one_hot_constraint_id {
                        if existing_target_ids.contains(&target_id) {
                            crate::bail!(
                                "One-hot promotion target ID {target_id:?} is already used"
                            );
                        }
                        if !target_ids.insert(target_id) {
                            crate::bail!("Duplicate one-hot promotion target ID {target_id:?}");
                        }
                    }
                }
            }
        }

        let mut plans = Vec::with_capacity(certificates.len());
        for certificate in certificates {
            match certificate {
                PromotionCertificate::OneHot(certificate) => {
                    let target_id = match certificate.target_one_hot_constraint_id {
                        Some(target_id) => target_id,
                        None => allocate_one_hot_target_id(&mut target_ids)?,
                    };
                    let source = self
                        .constraint_collection
                        .active()
                        .get(&certificate.source_constraint_id)
                        .ok_or_else(|| {
                            crate::error!(
                                "Active regular constraint {:?} was not found",
                                certificate.source_constraint_id
                            )
                        })?;
                    self.verify_one_hot_equivalence(
                        certificate.source_constraint_id,
                        source,
                        &certificate.variables,
                    )?;
                    plans.push(PlannedOneHotPromotion {
                        source_constraint_id: certificate.source_constraint_id,
                        variables: certificate.variables.clone(),
                        target_one_hot_constraint_id: target_id,
                    });
                }
            }
        }
        Ok(plans)
    }

    fn verify_one_hot_equivalence(
        &self,
        source_constraint_id: ConstraintID,
        source: &Constraint,
        variables: &BTreeSet<VariableID>,
    ) -> crate::Result<()> {
        self.validate_one_hot_variables(variables)?;
        let support = self.verified_one_hot_source_variables(source_constraint_id, source)?;
        if support != *variables {
            crate::bail!(
                "Regular constraint {source_constraint_id:?} has linear support {support:?}, expected {variables:?}"
            );
        }
        Ok(())
    }

    fn validate_one_hot_variables(&self, variables: &BTreeSet<VariableID>) -> crate::Result<()> {
        if variables.is_empty() {
            crate::bail!("One-hot promotion variables must not be empty");
        }
        for variable_id in variables {
            let variable = self.decision_variables.get(variable_id).ok_or_else(|| {
                crate::error!(
                    "One-hot promotion variable {variable_id:?} is not a decision variable"
                )
            })?;
            if variable.kind() != Kind::Binary {
                crate::bail!("One-hot promotion variable {variable_id:?} must be binary");
            }
        }
        Ok(())
    }

    fn verified_one_hot_source_variables(
        &self,
        source_constraint_id: ConstraintID,
        source: &Constraint,
    ) -> crate::Result<BTreeSet<VariableID>> {
        if source.equality != Equality::EqualToZero {
            crate::bail!(
                "Regular constraint {source_constraint_id:?} is not an equality-to-zero constraint"
            );
        }

        let linear = source.function().as_linear().ok_or_else(|| {
            crate::error!("Regular constraint {source_constraint_id:?} is not exactly linear")
        })?;
        let coefficients: BTreeMap<VariableID, Coefficient> = linear.linear_terms().collect();
        let support: BTreeSet<VariableID> = coefficients.keys().copied().collect();
        self.validate_one_hot_variables(&support)?;

        let common_coefficient = *coefficients
            .values()
            .next()
            .expect("non-empty support has at least one coefficient");
        if coefficients
            .values()
            .any(|coefficient| *coefficient != common_coefficient)
        {
            crate::bail!(
                "Regular constraint {source_constraint_id:?} does not use one common coefficient for all one-hot variables"
            );
        }

        let constant = linear.get(&LinearMonomial::Constant);
        if constant != Some(-common_coefficient) {
            crate::bail!(
                "Regular constraint {source_constraint_id:?} has constant coefficient {constant:?}, expected {:?}",
                -common_coefficient
            );
        }
        Ok(support)
    }

    fn apply_promotion_plans(&mut self, plans: &[PlannedOneHotPromotion]) -> crate::Result<()> {
        let mut staged = self.clone();
        for plan in plans {
            let context = staged
                .constraint_collection
                .context()
                .collect_for(plan.source_constraint_id);
            let target = OneHotConstraint::new(plan.variables.clone())?;
            staged
                .one_hot_constraint_collection
                .insert_active_with_context(plan.target_one_hot_constraint_id, target, context)?;

            let mut parameters = fnv::FnvHashMap::default();
            parameters.insert(
                PROMOTION_KIND_PARAMETER.to_string(),
                ONE_HOT_PROMOTION_KIND.to_string(),
            );
            parameters.insert(
                PROMOTION_TARGET_ID_PARAMETER.to_string(),
                plan.target_one_hot_constraint_id.into_inner().to_string(),
            );
            parameters.insert(
                PROMOTION_CERTIFICATE_VERSION_PARAMETER.to_string(),
                PROMOTION_CERTIFICATE_VERSION.to_string(),
            );
            staged.constraint_collection.relax(
                plan.source_constraint_id,
                RemovedReason {
                    reason: ONE_HOT_PROMOTION_REASON.to_string(),
                    parameters,
                },
            )?;
        }
        *self = staged;
        Ok(())
    }
}

/// Reject restoring a promoted regular source while its one-hot target is
/// retained. This is public only inside the private `instance` module so the
/// sibling restore implementation can enforce the cross-family invariant.
pub fn ensure_promoted_constraint_can_be_restored(
    instance: &Instance,
    source_constraint_id: ConstraintID,
) -> crate::Result<()> {
    let Some((_source, removed_reason)) = instance
        .constraint_collection
        .removed()
        .get(&source_constraint_id)
    else {
        return Ok(());
    };
    let Some(target_id) = promoted_one_hot_target(removed_reason)? else {
        return Ok(());
    };

    if instance
        .one_hot_constraint_collection
        .active()
        .contains_key(&target_id)
        || instance
            .one_hot_constraint_collection
            .removed()
            .contains_key(&target_id)
    {
        crate::bail!(
            "Cannot restore promoted regular constraint {source_constraint_id:?} while one-hot target {target_id:?} exists"
        );
    }
    Ok(())
}

fn allocate_one_hot_target_id(
    reserved: &mut BTreeSet<OneHotConstraintID>,
) -> crate::Result<OneHotConstraintID> {
    let next = match reserved.last().copied() {
        Some(max_id) => match max_id.into_inner().checked_add(1) {
            Some(next) => next,
            None => first_unused_one_hot_target_id(reserved)?,
        },
        None => 0,
    };
    let id = OneHotConstraintID::from(next);
    let inserted = reserved.insert(id);
    debug_assert!(inserted, "max + 1 must be an unused target ID");
    Ok(id)
}

fn first_unused_one_hot_target_id(reserved: &BTreeSet<OneHotConstraintID>) -> crate::Result<u64> {
    let mut candidate = 0u64;
    for id in reserved {
        let value = id.into_inner();
        if value < candidate {
            continue;
        }
        if value > candidate {
            return Ok(candidate);
        }
        candidate = candidate.checked_add(1).ok_or_else(|| {
            crate::error!("One-hot constraint ID space is exhausted during promotion")
        })?;
    }
    Ok(candidate)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        coeff, quadratic, DecisionVariable, Evaluate, Linear, ModelingLabel, Provenance, Sense,
    };

    fn variables(ids: impl IntoIterator<Item = u64>) -> BTreeSet<VariableID> {
        ids.into_iter().map(VariableID::from).collect()
    }

    fn exact_one_hot_function(ids: &[u64], coefficient: f64) -> Function {
        let coefficient = Coefficient::try_from(coefficient).unwrap();
        let terms = ids
            .iter()
            .copied()
            .map(|id| (LinearMonomial::from(id), coefficient))
            .chain(std::iter::once((LinearMonomial::Constant, -coefficient)));
        Function::Linear(Linear::try_from_terms(terms).unwrap())
    }

    fn linear_function(terms: &[(u64, f64)], constant: f64) -> Function {
        let terms = terms
            .iter()
            .map(|(id, coefficient)| {
                (
                    LinearMonomial::from(*id),
                    Coefficient::try_from(*coefficient).unwrap(),
                )
            })
            .chain(std::iter::once((
                LinearMonomial::Constant,
                Coefficient::try_from(constant).unwrap(),
            )));
        Function::Linear(Linear::try_from_terms(terms).unwrap())
    }

    fn certificate(
        source_constraint_id: u64,
        variable_ids: impl IntoIterator<Item = u64>,
        target_one_hot_constraint_id: Option<u64>,
    ) -> PromotionCertificate {
        OneHotPromotionCertificate {
            source_constraint_id: ConstraintID::from(source_constraint_id),
            variables: variables(variable_ids),
            target_one_hot_constraint_id: target_one_hot_constraint_id
                .map(OneHotConstraintID::from),
        }
        .into()
    }

    fn instance_with_constraints(constraints: BTreeMap<ConstraintID, Constraint>) -> Instance {
        let mut decision_variables = BTreeMap::new();
        for id in 1..=4 {
            decision_variables.insert(VariableID::from(id), DecisionVariable::binary());
        }
        decision_variables.insert(VariableID::from(5), DecisionVariable::integer());
        Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::Zero)
            .decision_variables(decision_variables)
            .constraints(constraints)
            .build()
            .unwrap()
    }

    fn one_source_instance(coefficient: f64) -> Instance {
        instance_with_constraints(BTreeMap::from([(
            ConstraintID::from(10),
            Constraint::equal_to_zero(exact_one_hot_function(&[1, 2, 3], coefficient)),
        )]))
    }

    fn allowed_one_hot() -> Capabilities {
        Capabilities::from([AdditionalCapability::OneHot])
    }

    #[test]
    fn checks_and_applies_exact_one_hot_with_audit_context_and_capability() {
        let mut instance = one_source_instance(2.0);
        instance
            .set_constraint_context(
                ConstraintID::from(10),
                ConstraintContext {
                    label: ModelingLabel {
                        name: Some("choose".to_string()),
                        ..Default::default()
                    },
                    provenance: vec![Provenance::Sos1Constraint(crate::Sos1ConstraintID::from(
                        22,
                    ))],
                },
            )
            .unwrap();
        let certificate = certificate(10, [1, 2, 3], None);

        let before = instance.clone();
        let preview = instance
            .check_promotion_certificate(&certificate, &allowed_one_hot())
            .unwrap();
        assert_eq!(instance, before, "dry-run verification must not mutate");
        assert_eq!(preview.source_constraint_id(), ConstraintID::from(10));
        assert_eq!(preview.variables(), &variables([1, 2, 3]));
        assert_eq!(
            preview.target_one_hot_constraint_id(),
            OneHotConstraintID::from(0)
        );

        let result = instance
            .promote_with_certificate(certificate, &allowed_one_hot())
            .unwrap();
        assert_eq!(result.source_constraint_id(), ConstraintID::from(10));
        assert_eq!(
            result.target_one_hot_constraint_id(),
            OneHotConstraintID::from(0)
        );
        assert!(instance.constraints().is_empty());
        assert_eq!(
            instance
                .one_hot_constraints()
                .get(&OneHotConstraintID::from(0))
                .unwrap()
                .variables,
            variables([1, 2, 3])
        );
        assert_eq!(
            instance
                .one_hot_constraint_context()
                .name(OneHotConstraintID::from(0)),
            Some("choose")
        );
        assert_eq!(
            instance
                .one_hot_constraint_context()
                .provenance(OneHotConstraintID::from(0)),
            &[Provenance::Sos1Constraint(crate::Sos1ConstraintID::from(
                22
            ))]
        );
        assert_eq!(instance.required_capabilities(), allowed_one_hot());

        let (_source, removed_reason) = instance
            .removed_constraints()
            .get(&ConstraintID::from(10))
            .unwrap();
        assert_eq!(removed_reason.reason, ONE_HOT_PROMOTION_REASON);
        assert_eq!(
            removed_reason.parameters.get(PROMOTION_KIND_PARAMETER),
            Some(&ONE_HOT_PROMOTION_KIND.to_string())
        );
        assert_eq!(
            removed_reason.parameters.get(PROMOTION_TARGET_ID_PARAMETER),
            Some(&"0".to_string())
        );
        assert_eq!(
            removed_reason
                .parameters
                .get(PROMOTION_CERTIFICATE_VERSION_PARAMETER),
            Some(&PROMOTION_CERTIFICATE_VERSION.to_string())
        );

        let audit = instance
            .verify_promotion_history(ConstraintID::from(10))
            .unwrap();
        assert_eq!(audit.source_constraint_id(), ConstraintID::from(10));
        assert_eq!(audit.variables(), &variables([1, 2, 3]));
        assert_eq!(
            audit.target_one_hot_constraint_id(),
            OneHotConstraintID::from(0)
        );
        assert!(audit.target_is_active());
    }

    #[test]
    fn accepts_positive_and_negative_exact_scalar_multiples() {
        for coefficient in [1.0, 2.0, -1.0, -3.5] {
            one_source_instance(coefficient)
                .check_promotion_certificate(&certificate(10, [1, 2, 3], None), &allowed_one_hot())
                .unwrap();
        }
    }

    #[test]
    fn rejects_disallowed_nonbinary_and_non_exact_certificates() {
        let instance = one_source_instance(1.0);
        let err = instance
            .check_promotion_certificate(&certificate(10, [1, 2, 3], None), &Capabilities::new())
            .unwrap_err();
        assert!(err.to_string().contains("allowed capabilities"));

        for invalid in [
            certificate(10, [], None),
            certificate(10, [1, 2, 5], None),
            certificate(10, [1, 2, 99], None),
            certificate(10, [1, 2], None),
            certificate(999, [1, 2, 3], None),
        ] {
            assert!(instance
                .check_promotion_certificate(&invalid, &allowed_one_hot())
                .is_err());
        }

        let inequality = instance_with_constraints(BTreeMap::from([(
            ConstraintID::from(10),
            Constraint::less_than_or_equal_to_zero(exact_one_hot_function(&[1, 2, 3], 1.0)),
        )]));
        assert!(inequality
            .check_promotion_certificate(&certificate(10, [1, 2, 3], None), &allowed_one_hot(),)
            .is_err());

        let unequal = instance_with_constraints(BTreeMap::from([(
            ConstraintID::from(10),
            Constraint::equal_to_zero(linear_function(&[(1, 1.0), (2, 2.0)], -1.0)),
        )]));
        assert!(unequal
            .check_promotion_certificate(&certificate(10, [1, 2], None), &allowed_one_hot(),)
            .is_err());

        let wrong_constant = instance_with_constraints(BTreeMap::from([(
            ConstraintID::from(10),
            Constraint::equal_to_zero(linear_function(&[(1, 1.0), (2, 1.0)], -2.0)),
        )]));
        assert!(wrong_constant
            .check_promotion_certificate(&certificate(10, [1, 2], None), &allowed_one_hot(),)
            .is_err());

        let approximate = instance_with_constraints(BTreeMap::from([(
            ConstraintID::from(10),
            Constraint::equal_to_zero(linear_function(&[(1, 1.0), (2, 1.0 + f64::EPSILON)], -1.0)),
        )]));
        assert!(approximate
            .check_promotion_certificate(&certificate(10, [1, 2], None), &allowed_one_hot(),)
            .is_err());
    }

    #[test]
    fn rejects_nonlinear_payload_with_valid_looking_linear_terms() {
        let linear_part = ((quadratic!(1) + quadratic!(2)).unwrap() + coeff!(-1.0)).unwrap();
        let nonlinear = Function::Quadratic((linear_part + quadratic!(1, 2)).unwrap());
        let instance = instance_with_constraints(BTreeMap::from([(
            ConstraintID::from(10),
            Constraint::equal_to_zero(nonlinear),
        )]));

        let err = instance
            .check_promotion_certificate(&certificate(10, [1, 2], None), &allowed_one_hot())
            .unwrap_err();
        assert!(err.to_string().contains("not exactly linear"));
    }

    #[test]
    fn rejects_target_ids_used_in_active_or_removed_one_hot_collections() {
        let existing = OneHotConstraint::new(variables([3, 4])).unwrap();
        let mut active_collision = one_source_instance(1.0);
        active_collision
            .one_hot_constraint_collection
            .insert_active_with_context(
                OneHotConstraintID::from(7),
                existing.clone(),
                ConstraintContext::default(),
            )
            .unwrap();
        assert!(active_collision
            .check_promotion_certificate(&certificate(10, [1, 2, 3], Some(7)), &allowed_one_hot(),)
            .is_err());

        active_collision
            .convert_one_hot_to_constraint(OneHotConstraintID::from(7))
            .unwrap();
        assert!(active_collision
            .check_promotion_certificate(&certificate(10, [1, 2, 3], Some(7)), &allowed_one_hot(),)
            .is_err());
    }

    #[test]
    fn bulk_reserves_explicit_targets_and_is_atomic() {
        let mut instance = instance_with_constraints(BTreeMap::from([
            (
                ConstraintID::from(10),
                Constraint::equal_to_zero(exact_one_hot_function(&[1, 2], 1.0)),
            ),
            (
                ConstraintID::from(11),
                Constraint::equal_to_zero(exact_one_hot_function(&[3, 4], -2.0)),
            ),
        ]));
        instance
            .one_hot_constraint_collection
            .insert_active_with_context(
                OneHotConstraintID::from(5),
                OneHotConstraint::new(variables([1])).unwrap(),
                ConstraintContext::default(),
            )
            .unwrap();

        let report = instance
            .promote_with_certificates(
                vec![
                    certificate(10, [1, 2], None),
                    certificate(11, [3, 4], Some(10)),
                ],
                &allowed_one_hot(),
            )
            .unwrap();
        assert_eq!(
            report.source_to_target(),
            &BTreeMap::from([
                (ConstraintID::from(10), OneHotConstraintID::from(11)),
                (ConstraintID::from(11), OneHotConstraintID::from(10)),
            ])
        );

        let duplicate_source_before = instance.clone();
        assert!(instance
            .promote_with_certificates(
                vec![
                    certificate(99, [1], Some(20)),
                    certificate(99, [1], Some(21)),
                ],
                &allowed_one_hot(),
            )
            .is_err());
        assert_eq!(instance, duplicate_source_before);

        let mut atomic = instance_with_constraints(BTreeMap::from([
            (
                ConstraintID::from(20),
                Constraint::equal_to_zero(exact_one_hot_function(&[1, 2], 1.0)),
            ),
            (
                ConstraintID::from(21),
                Constraint::less_than_or_equal_to_zero(exact_one_hot_function(&[3, 4], 1.0)),
            ),
        ]));
        let before = atomic.clone();
        assert!(atomic
            .promote_with_certificates(
                vec![certificate(20, [1, 2], None), certificate(21, [3, 4], None),],
                &allowed_one_hot(),
            )
            .is_err());
        assert_eq!(atomic, before);

        let before = atomic.clone();
        assert!(atomic
            .promote_with_certificates(
                vec![
                    certificate(20, [1, 2], Some(30)),
                    certificate(21, [3, 4], Some(30)),
                ],
                &allowed_one_hot(),
            )
            .is_err());
        assert_eq!(atomic, before);
    }

    #[test]
    fn bulk_allocates_a_gap_when_an_explicit_target_is_u64_max() {
        let mut instance = instance_with_constraints(BTreeMap::from([
            (
                ConstraintID::from(10),
                Constraint::equal_to_zero(exact_one_hot_function(&[1, 2], 1.0)),
            ),
            (
                ConstraintID::from(11),
                Constraint::equal_to_zero(exact_one_hot_function(&[3, 4], 1.0)),
            ),
        ]));

        let report = instance
            .promote_with_certificates(
                vec![
                    certificate(10, [1, 2], None),
                    certificate(11, [3, 4], Some(u64::MAX)),
                ],
                &allowed_one_hot(),
            )
            .unwrap();
        assert_eq!(
            report.source_to_target(),
            &BTreeMap::from([
                (ConstraintID::from(10), OneHotConstraintID::from(0)),
                (ConstraintID::from(11), OneHotConstraintID::from(u64::MAX),),
            ])
        );
    }

    #[test]
    fn preview_is_not_a_reusable_plan() {
        let mut instance = one_source_instance(1.0);
        let certificate = certificate(10, [1, 2, 3], None);
        let preview = instance
            .check_promotion_certificate(&certificate, &allowed_one_hot())
            .unwrap();
        assert_eq!(
            preview.target_one_hot_constraint_id(),
            OneHotConstraintID::from(0)
        );

        instance
            .one_hot_constraint_collection
            .insert_active_with_context(
                OneHotConstraintID::from(0),
                OneHotConstraint::new(variables([4])).unwrap(),
                ConstraintContext::default(),
            )
            .unwrap();
        let result = instance
            .promote_with_certificate(certificate, &allowed_one_hot())
            .unwrap();
        assert_eq!(
            result.target_one_hot_constraint_id(),
            OneHotConstraintID::from(1)
        );
    }

    #[test]
    fn lowering_retains_auditable_history_and_blocks_source_restore() {
        let mut instance = one_source_instance(-1.0);
        instance
            .promote_with_certificate(certificate(10, [1, 2, 3], None), &allowed_one_hot())
            .unwrap();
        instance.reduce_capabilities(&Capabilities::new()).unwrap();
        assert!(instance.one_hot_constraints().is_empty());
        assert!(instance
            .removed_one_hot_constraints()
            .contains_key(&OneHotConstraintID::from(0)));

        let audit = instance
            .verify_promotion_history(ConstraintID::from(10))
            .unwrap();
        assert!(!audit.target_is_active());

        let before_restore = instance.clone();
        let error = instance
            .restore_constraint(ConstraintID::from(10))
            .unwrap_err();
        assert!(error.to_string().contains("Cannot restore promoted"));
        assert_eq!(instance, before_restore);

        let bytes = instance.to_v2_bytes();
        let round_trip = Instance::from_v2_bytes(&bytes).unwrap();
        let audit = round_trip
            .verify_promotion_history(ConstraintID::from(10))
            .unwrap();
        assert_eq!(audit.variables(), &variables([1, 2, 3]));
        assert!(!audit.target_is_active());
    }

    #[test]
    fn audit_normalizes_members_removed_by_exact_zero_propagation() {
        let mut instance = one_source_instance(1.0);
        instance
            .promote_with_certificate(certificate(10, [1, 2, 3], None), &allowed_one_hot())
            .unwrap();
        instance
            .partial_evaluate(
                &crate::v1::State::from(std::collections::HashMap::from([(1, 0.0)])),
                crate::ATol::default(),
            )
            .unwrap();
        assert_eq!(
            instance
                .one_hot_constraints()
                .get(&OneHotConstraintID::from(0))
                .unwrap()
                .variables,
            variables([2, 3])
        );

        let audit = instance
            .verify_promotion_history(ConstraintID::from(10))
            .unwrap();
        assert_eq!(audit.variables(), &variables([1, 2, 3]));
        assert!(audit.target_is_active());

        let round_trip = Instance::from_v2_bytes(&instance.to_v2_bytes()).unwrap();
        let audit = round_trip
            .verify_promotion_history(ConstraintID::from(10))
            .unwrap();
        assert_eq!(audit.variables(), &variables([1, 2, 3]));
        assert!(audit.target_is_active());
    }

    #[test]
    fn audit_rejects_unjustified_target_shrinkage() {
        let mut instance = one_source_instance(1.0);
        instance
            .promote_with_certificate(certificate(10, [1, 2, 3], None), &allowed_one_hot())
            .unwrap();
        instance
            .one_hot_constraint_collection
            .replace_active_row(
                OneHotConstraintID::from(0),
                OneHotConstraint::new(variables([2, 3])).unwrap(),
            )
            .unwrap();

        let error = instance
            .verify_promotion_history(ConstraintID::from(10))
            .unwrap_err();
        assert!(error.to_string().contains("not fixed exactly to zero"));
    }
}
