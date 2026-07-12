use super::exact::{from_f64, ExactArithmeticError, ExactRational};
use crate::{constraint::Equality, ConstraintID, Function, Instance, Kind, Linear, VariableID};
use num::{BigInt, Signed};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

mod activity;
mod candidate;

pub(crate) use candidate::verify_indicator_big_m_v1;

const FINGERPRINT_DOMAIN: &[u8] = b"org.ommx.proof.linear-relaxation\0";
const FINGERPRINT_VERSION: u32 = 1;

/// Errors while constructing or reading the exact continuous linear
/// relaxation of the current [`Instance`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
enum LinearProofError {
    #[error("active regular constraint {id:?} is not linear")]
    NonlinearConstraint { id: ConstraintID },
    #[error(
        "active regular constraint {constraint_id:?} references unknown variable {variable_id:?}"
    )]
    UnknownVariable {
        constraint_id: ConstraintID,
        variable_id: VariableID,
    },
    #[error("{fact} for variable {id:?} is not finite")]
    NonFiniteVariableFact { id: VariableID, fact: &'static str },
    #[error("regular constraint {id:?} is not an inequality proof atom")]
    EqualityUsedAsInequality { id: ConstraintID },
    #[error("regular constraint {id:?} is not an equality proof atom")]
    InequalityUsedAsEquality { id: ConstraintID },
    #[error("regular constraint {id:?} is not active in the certified relaxation")]
    UnknownConstraint { id: ConstraintID },
    #[error("variable {id:?} has no finite {side} bound proof atom")]
    MissingBound { id: VariableID, side: &'static str },
    #[error("variable {id:?} has no fixed-value equality proof atom")]
    MissingFixedValue { id: VariableID },
    #[error("variable {id:?} is not present in the certified relaxation")]
    UnknownDomain { id: VariableID },
    #[error(transparent)]
    ExactArithmetic(#[from] ExactArithmeticError),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum ProofVariableKind {
    Continuous,
    Integer,
    Binary,
    SemiContinuous,
    SemiInteger,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ExactVariableDomain {
    kind: ProofVariableKind,
    lower: Option<ExactRational>,
    upper: Option<ExactRational>,
    fixed: Option<ExactRational>,
}

impl ExactVariableDomain {
    fn kind(&self) -> ProofVariableKind {
        self.kind
    }

    fn lower(&self) -> Option<&ExactRational> {
        self.lower.as_ref()
    }

    fn upper(&self) -> Option<&ExactRational> {
        self.upper.as_ref()
    }

    fn fixed(&self) -> Option<&ExactRational> {
        self.fixed.as_ref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct ExactAffine {
    coefficients: BTreeMap<VariableID, ExactRational>,
    constant: ExactRational,
}

impl ExactAffine {
    fn from_linear(linear: &Linear) -> Result<Self, ExactArithmeticError> {
        let coefficients = linear
            .linear_terms()
            .map(|(id, coefficient)| Ok((id, from_f64(coefficient.into_inner())?)))
            .collect::<Result<_, ExactArithmeticError>>()?;
        Ok(Self {
            coefficients,
            constant: from_f64(linear.constant_term())?,
        })
    }

    fn from_function(function: &Function) -> Result<Option<Self>, ExactArithmeticError> {
        function
            .as_linear()
            .map(|linear| Self::from_linear(&linear))
            .transpose()
    }

    fn coefficients(&self) -> &BTreeMap<VariableID, ExactRational> {
        &self.coefficients
    }

    fn constant(&self) -> &ExactRational {
        &self.constant
    }

    fn negated(&self) -> Self {
        Self {
            coefficients: self
                .coefficients
                .iter()
                .map(|(id, coefficient)| (*id, -coefficient))
                .collect(),
            constant: -&self.constant,
        }
    }

    fn substitute(&self, id: VariableID, value: &ExactRational) -> Self {
        let mut out = self.clone();
        if let Some(coefficient) = out.coefficients.remove(&id) {
            out.constant += coefficient * value;
        }
        out
    }

    fn coordinate(id: VariableID, coefficient: i32, constant: ExactRational) -> Self {
        Self {
            coefficients: BTreeMap::from([(id, ExactRational::from_integer(coefficient.into()))]),
            constant,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExactConstraintSense {
    LessEqual,
    Equal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ExactLinearConstraint {
    affine: ExactAffine,
    sense: ExactConstraintSense,
}

impl ExactLinearConstraint {
    fn affine(&self) -> &ExactAffine {
        &self.affine
    }

    fn sense(&self) -> ExactConstraintSense {
        self.sense
    }
}

/// Typed reference to an inequality in the certified relaxation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum InequalityRef {
    RegularConstraint(ConstraintID),
    LowerBound(VariableID),
    UpperBound(VariableID),
}

/// Typed reference to an equality in the certified relaxation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum EqualityRef {
    RegularConstraint(ConstraintID),
    FixedValue(VariableID),
}

/// Content identity of one canonical exact linear relaxation.
///
/// This binds evidence to a representation. It does not prove the mathematical
/// relation between an arbitrary source model and this representation.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct LinearRelaxationFingerprint([u8; 32]);

impl std::fmt::Debug for LinearRelaxationFingerprint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("LinearRelaxationFingerprint(")?;
        for byte in self.0 {
            write!(f, "{byte:02x}")?;
        }
        f.write_str(")")
    }
}

impl LinearRelaxationFingerprint {
    #[cfg(test)]
    fn hex(self) -> String {
        self.0.iter().map(|byte| format!("{byte:02x}")).collect()
    }
}

/// Immutable exact continuous linear relaxation of the current [`Instance`].
///
/// It contains all active regular linear rows, sound finite variable bound
/// sides, and fixed-value equations. Integrality and active special constraints
/// are deliberately omitted, which makes this an outer relaxation. Split
/// semi-variable intervals are also omitted because they do not describe the
/// complete `{0} ∪ [l, u]` domain. Removed rows, objectives, labels,
/// annotations, and named functions are not proof atoms.
#[derive(Debug, Clone, PartialEq, Eq)]
struct CertifiedLinearRelaxation {
    domains: BTreeMap<VariableID, ExactVariableDomain>,
    regular_rows: BTreeMap<ConstraintID, ExactLinearConstraint>,
    fingerprint: LinearRelaxationFingerprint,
}

impl CertifiedLinearRelaxation {
    fn domains(&self) -> &BTreeMap<VariableID, ExactVariableDomain> {
        &self.domains
    }

    fn regular_rows(&self) -> &BTreeMap<ConstraintID, ExactLinearConstraint> {
        &self.regular_rows
    }

    fn fingerprint(&self) -> LinearRelaxationFingerprint {
        self.fingerprint
    }

    fn inequality(&self, reference: InequalityRef) -> Result<ExactAffine, LinearProofError> {
        match reference {
            InequalityRef::RegularConstraint(id) => {
                let row = self
                    .regular_rows
                    .get(&id)
                    .ok_or(LinearProofError::UnknownConstraint { id })?;
                if row.sense != ExactConstraintSense::LessEqual {
                    return Err(LinearProofError::EqualityUsedAsInequality { id });
                }
                Ok(row.affine.clone())
            }
            InequalityRef::LowerBound(id) => {
                let domain = self
                    .domains
                    .get(&id)
                    .ok_or(LinearProofError::UnknownDomain { id })?;
                let value = domain
                    .lower
                    .clone()
                    .ok_or(LinearProofError::MissingBound { id, side: "lower" })?;
                Ok(ExactAffine::coordinate(id, -1, value))
            }
            InequalityRef::UpperBound(id) => {
                let domain = self
                    .domains
                    .get(&id)
                    .ok_or(LinearProofError::UnknownDomain { id })?;
                let value = domain
                    .upper
                    .clone()
                    .ok_or(LinearProofError::MissingBound { id, side: "upper" })?;
                Ok(ExactAffine::coordinate(id, 1, -value))
            }
        }
    }

    fn equality(&self, reference: EqualityRef) -> Result<ExactAffine, LinearProofError> {
        match reference {
            EqualityRef::RegularConstraint(id) => {
                let row = self
                    .regular_rows
                    .get(&id)
                    .ok_or(LinearProofError::UnknownConstraint { id })?;
                if row.sense != ExactConstraintSense::Equal {
                    return Err(LinearProofError::InequalityUsedAsEquality { id });
                }
                Ok(row.affine.clone())
            }
            EqualityRef::FixedValue(id) => {
                let domain = self
                    .domains
                    .get(&id)
                    .ok_or(LinearProofError::UnknownDomain { id })?;
                let value = domain
                    .fixed
                    .clone()
                    .ok_or(LinearProofError::MissingFixedValue { id })?;
                Ok(ExactAffine::coordinate(id, 1, -value))
            }
        }
    }
}

impl Instance {
    /// Build the private exact proof target for this current representation.
    ///
    /// This is crate-internal because a snapshot is not an SDK certificate and
    /// cannot authorize mutation after the `Instance` changes.
    fn certified_linear_relaxation(&self) -> Result<CertifiedLinearRelaxation, LinearProofError> {
        let mut domains = BTreeMap::new();
        for (id, variable) in self.decision_variables() {
            let (kind, use_interval_bounds) = match variable.kind() {
                Kind::Continuous => (ProofVariableKind::Continuous, true),
                Kind::Integer => (ProofVariableKind::Integer, true),
                Kind::Binary => (ProofVariableKind::Binary, true),
                // The stored interval does not describe the complete split
                // domain `{0} ∪ [l, u]`. Omitting both sides is a sound outer
                // relaxation and lets unrelated semi variables coexist with
                // a proof; later proof rules must not infer either side.
                Kind::SemiContinuous => (ProofVariableKind::SemiContinuous, false),
                Kind::SemiInteger => (ProofVariableKind::SemiInteger, false),
            };
            let bound = variable.bound();
            let lower = (use_interval_bounds && bound.lower().is_finite())
                .then(|| from_f64(bound.lower()))
                .transpose()
                .map_err(|_| LinearProofError::NonFiniteVariableFact {
                    id: *id,
                    fact: "lower bound",
                })?;
            let upper = (use_interval_bounds && bound.upper().is_finite())
                .then(|| from_f64(bound.upper()))
                .transpose()
                .map_err(|_| LinearProofError::NonFiniteVariableFact {
                    id: *id,
                    fact: "upper bound",
                })?;
            let fixed = self
                .fixed_decision_variable_value(*id)
                .map(from_f64)
                .transpose()
                .map_err(|_| LinearProofError::NonFiniteVariableFact {
                    id: *id,
                    fact: "fixed value",
                })?;
            domains.insert(
                *id,
                ExactVariableDomain {
                    kind,
                    lower,
                    upper,
                    fixed,
                },
            );
        }

        let mut regular_rows = BTreeMap::new();
        for (id, constraint) in self.constraints() {
            let affine = ExactAffine::from_function(constraint.function())?
                .ok_or(LinearProofError::NonlinearConstraint { id: *id })?;
            if let Some(variable_id) = affine
                .coefficients
                .keys()
                .find(|variable_id| !domains.contains_key(variable_id))
            {
                return Err(LinearProofError::UnknownVariable {
                    constraint_id: *id,
                    variable_id: *variable_id,
                });
            }
            let sense = match constraint.equality {
                Equality::LessThanOrEqualToZero => ExactConstraintSense::LessEqual,
                Equality::EqualToZero => ExactConstraintSense::Equal,
            };
            regular_rows.insert(*id, ExactLinearConstraint { affine, sense });
        }

        let fingerprint = fingerprint(&domains, &regular_rows);
        Ok(CertifiedLinearRelaxation {
            domains,
            regular_rows,
            fingerprint,
        })
    }
}

fn fingerprint(
    domains: &BTreeMap<VariableID, ExactVariableDomain>,
    rows: &BTreeMap<ConstraintID, ExactLinearConstraint>,
) -> LinearRelaxationFingerprint {
    let mut hasher = Sha256::new();
    hasher.update(FINGERPRINT_DOMAIN);
    hasher.update(FINGERPRINT_VERSION.to_be_bytes());
    hash_len(&mut hasher, domains.len());
    for (id, domain) in domains {
        hasher.update(id.into_inner().to_be_bytes());
        hasher.update([match domain.kind {
            ProofVariableKind::Continuous => 0,
            ProofVariableKind::Integer => 1,
            ProofVariableKind::Binary => 2,
            ProofVariableKind::SemiContinuous => 3,
            ProofVariableKind::SemiInteger => 4,
        }]);
        hash_optional_exact(&mut hasher, domain.lower.as_ref());
        hash_optional_exact(&mut hasher, domain.upper.as_ref());
        hash_optional_exact(&mut hasher, domain.fixed.as_ref());
    }
    hash_len(&mut hasher, rows.len());
    for (id, row) in rows {
        hasher.update(id.into_inner().to_be_bytes());
        hasher.update([match row.sense {
            ExactConstraintSense::LessEqual => 0,
            ExactConstraintSense::Equal => 1,
        }]);
        hash_exact(&mut hasher, &row.affine.constant);
        hash_len(&mut hasher, row.affine.coefficients.len());
        for (id, coefficient) in &row.affine.coefficients {
            hasher.update(id.into_inner().to_be_bytes());
            hash_exact(&mut hasher, coefficient);
        }
    }
    LinearRelaxationFingerprint(hasher.finalize().into())
}

fn hash_len(hasher: &mut Sha256, len: usize) {
    hasher.update(
        u64::try_from(len)
            .expect("proof representation length exceeds u64")
            .to_be_bytes(),
    );
}

fn hash_optional_exact(hasher: &mut Sha256, value: Option<&ExactRational>) {
    match value {
        Some(value) => {
            hasher.update([1]);
            hash_exact(hasher, value);
        }
        None => hasher.update([0]),
    }
}

fn hash_exact(hasher: &mut Sha256, value: &ExactRational) {
    let numerator = value.numer();
    hasher.update([if numerator.is_negative() { 1 } else { 0 }]);
    hash_bigint_magnitude(hasher, numerator);
    hash_bigint_magnitude(hasher, value.denom());
}

fn hash_bigint_magnitude(hasher: &mut Sha256, value: &BigInt) {
    let (_, bytes) = value.to_bytes_be();
    hash_len(hasher, bytes.len());
    hasher.update(bytes);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        coeff, linear, Bound, Constraint, DecisionVariable, IndicatorConstraint,
        IndicatorConstraintID, ModelingLabel, RemovedReason, Sense,
    };
    use fnv::FnvHashMap;
    use num::{One, Zero};

    fn exact(value: i64) -> ExactRational {
        ExactRational::from_integer(value.into())
    }

    fn base_instance() -> Instance {
        let x = VariableID::from(1);
        let fixed = VariableID::from(2);
        Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::zero())
            .decision_variables(BTreeMap::from([
                (
                    x,
                    DecisionVariable::new(
                        Kind::Continuous,
                        Bound::new(0.0, 2.0).unwrap(),
                        crate::ATol::default(),
                    )
                    .unwrap(),
                ),
                (fixed, DecisionVariable::continuous()),
            ]))
            .fixed_decision_variable_values(BTreeMap::from([(fixed, 0.5)]))
            .constraints(BTreeMap::from([
                (
                    ConstraintID::from(10),
                    Constraint::less_than_or_equal_to_zero(Function::from(
                        (linear!(1) + coeff!(-1.0)).unwrap(),
                    )),
                ),
                (
                    ConstraintID::from(11),
                    Constraint::equal_to_zero(Function::from(linear!(1))),
                ),
            ]))
            .build()
            .unwrap()
    }

    fn fixture_fingerprint(
        variables: BTreeMap<VariableID, DecisionVariable>,
        fixed: BTreeMap<VariableID, f64>,
        constraints: BTreeMap<ConstraintID, Constraint>,
    ) -> LinearRelaxationFingerprint {
        Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::zero())
            .decision_variables(variables)
            .fixed_decision_variable_values(fixed)
            .constraints(constraints)
            .build()
            .unwrap()
            .certified_linear_relaxation()
            .unwrap()
            .fingerprint()
    }

    #[test]
    fn snapshot_has_exact_rows_domains_and_typed_atoms() {
        let snapshot = base_instance().certified_linear_relaxation().unwrap();
        let x = VariableID::from(1);
        let fixed = VariableID::from(2);

        assert_eq!(snapshot.domains.len(), 2);
        assert_eq!(snapshot.regular_rows.len(), 2);
        assert_eq!(snapshot.domains[&x].kind(), ProofVariableKind::Continuous);
        assert_eq!(snapshot.domains[&x].lower(), Some(&exact(0)));
        assert_eq!(snapshot.domains[&x].upper(), Some(&exact(2)));
        assert_eq!(
            snapshot.domains[&fixed].fixed(),
            Some(&from_f64(0.5).unwrap())
        );

        let lower = snapshot.inequality(InequalityRef::LowerBound(x)).unwrap();
        assert_eq!(lower.coefficients[&x], -ExactRational::one());
        assert_eq!(lower.constant, ExactRational::zero());
        let upper = snapshot.inequality(InequalityRef::UpperBound(x)).unwrap();
        assert_eq!(upper.coefficients[&x], ExactRational::one());
        assert_eq!(upper.constant, ExactRational::from_integer((-2).into()));
        let fixed_row = snapshot.equality(EqualityRef::FixedValue(fixed)).unwrap();
        assert_eq!(fixed_row.coefficients[&fixed], ExactRational::one());
        assert_eq!(fixed_row.constant, from_f64(-0.5).unwrap());

        assert!(matches!(
            snapshot.inequality(InequalityRef::RegularConstraint(11.into())),
            Err(LinearProofError::EqualityUsedAsInequality { .. })
        ));
        assert!(matches!(
            snapshot.equality(EqualityRef::RegularConstraint(10.into())),
            Err(LinearProofError::InequalityUsedAsEquality { .. })
        ));
    }

    #[test]
    fn snapshot_rejects_nonlinear_rows_without_partial_output() {
        let x = VariableID::from(1);
        let nonlinear = Function::from(crate::quadratic!(1, 1));
        let instance = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::zero())
            .decision_variables(BTreeMap::from([(x, DecisionVariable::continuous())]))
            .constraints(BTreeMap::from([(
                ConstraintID::from(1),
                Constraint::less_than_or_equal_to_zero(nonlinear),
            )]))
            .build()
            .unwrap();
        assert!(matches!(
            instance.certified_linear_relaxation(),
            Err(LinearProofError::NonlinearConstraint { id }) if id == ConstraintID::from(1)
        ));
    }

    #[test]
    fn snapshot_keeps_semi_coordinates_without_unsound_interval_atoms() {
        for (kind, proof_kind) in [
            (Kind::SemiContinuous, ProofVariableKind::SemiContinuous),
            (Kind::SemiInteger, ProofVariableKind::SemiInteger),
        ] {
            let variable =
                DecisionVariable::new(kind, Bound::new(2.0, 5.0).unwrap(), crate::ATol::default())
                    .unwrap();
            let instance = Instance::builder()
                .sense(Sense::Minimize)
                .objective(Function::zero())
                .decision_variables(BTreeMap::from([(VariableID::from(1), variable)]))
                .constraints(BTreeMap::new())
                .build()
                .unwrap();
            let snapshot = instance.certified_linear_relaxation().unwrap();
            let domain = &snapshot.domains()[&VariableID::from(1)];
            assert_eq!(domain.kind(), proof_kind);
            assert_eq!(domain.lower(), None);
            assert_eq!(domain.upper(), None);
            assert!(matches!(
                snapshot.inequality(InequalityRef::LowerBound(VariableID::from(1))),
                Err(LinearProofError::MissingBound { .. })
            ));
            assert!(matches!(
                snapshot.inequality(InequalityRef::UpperBound(VariableID::from(1))),
                Err(LinearProofError::MissingBound { .. })
            ));
        }
    }

    #[test]
    fn active_special_and_removed_regular_rows_are_not_atoms() {
        let instance = base_instance();
        let indicator_variable = VariableID::from(3);
        let mut variables = instance.decision_variables().clone();
        variables.insert(indicator_variable, DecisionVariable::binary());
        let mut removed_reason = FnvHashMap::default();
        removed_reason.insert("why".to_string(), "test".to_string());
        let instance = Instance::builder()
            .sense(instance.sense())
            .objective(instance.objective().clone())
            .decision_variables(variables)
            .fixed_decision_variable_values(instance.fixed_decision_variable_values().clone())
            .constraints(instance.constraints().clone())
            .removed_constraints(BTreeMap::from([(
                ConstraintID::from(20),
                (
                    Constraint::equal_to_zero(Function::from(linear!(1))),
                    RemovedReason {
                        reason: "test".to_string(),
                        parameters: removed_reason,
                    },
                ),
            )]))
            .indicator_constraints(BTreeMap::from([(
                IndicatorConstraintID::from(7),
                IndicatorConstraint::new(
                    indicator_variable,
                    Equality::LessThanOrEqualToZero,
                    Function::from(linear!(1)),
                ),
            )]))
            .build()
            .unwrap();

        let snapshot = instance.certified_linear_relaxation().unwrap();
        assert_eq!(snapshot.regular_rows.len(), 2);
        assert!(!snapshot.regular_rows.contains_key(&ConstraintID::from(20)));
    }

    #[test]
    fn fingerprint_ignores_metadata_and_non_atoms() {
        let mut first = base_instance();
        let before = first.certified_linear_relaxation().unwrap().fingerprint();
        first
            .set_variable_label(
                VariableID::from(1),
                ModelingLabel {
                    name: Some("renamed".to_string()),
                    ..Default::default()
                },
            )
            .unwrap();
        first.description = Some(crate::v1::instance::Description {
            name: Some("metadata".to_string()),
            ..Default::default()
        });
        assert_eq!(
            first.certified_linear_relaxation().unwrap().fingerprint(),
            before
        );

        let changed = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::zero())
            .decision_variables(BTreeMap::from([
                (
                    VariableID::from(1),
                    DecisionVariable::new(
                        Kind::Continuous,
                        Bound::new(0.0, 3.0).unwrap(),
                        crate::ATol::default(),
                    )
                    .unwrap(),
                ),
                (VariableID::from(2), DecisionVariable::continuous()),
            ]))
            .fixed_decision_variable_values(BTreeMap::from([(VariableID::from(2), 0.5)]))
            .constraints(base_instance().constraints().clone())
            .build()
            .unwrap();
        assert_ne!(
            changed.certified_linear_relaxation().unwrap().fingerprint(),
            before
        );
    }

    #[test]
    fn fingerprint_changes_for_each_proof_relevant_field() {
        let base = base_instance();
        let base_fingerprint = base.certified_linear_relaxation().unwrap().fingerprint();
        let variables = base.decision_variables().clone();
        let fixed = base.fixed_decision_variable_values().clone();
        let constraints = base.constraints().clone();

        let changed_bound = |kind, lower, upper| {
            let mut changed = variables.clone();
            changed.insert(
                VariableID::from(1),
                DecisionVariable::new(
                    kind,
                    Bound::new(lower, upper).unwrap(),
                    crate::ATol::default(),
                )
                .unwrap(),
            );
            fixture_fingerprint(changed, fixed.clone(), constraints.clone())
        };
        assert_ne!(
            changed_bound(Kind::Continuous, -1.0, 2.0),
            base_fingerprint,
            "lower bound must be fingerprinted"
        );
        assert_ne!(
            changed_bound(Kind::Continuous, 0.0, 3.0),
            base_fingerprint,
            "upper bound must be fingerprinted"
        );
        assert_ne!(
            changed_bound(Kind::Integer, 0.0, 2.0),
            base_fingerprint,
            "variable kind must be fingerprinted"
        );

        let mut changed_fixed = fixed.clone();
        changed_fixed.insert(VariableID::from(2), 0.75);
        assert_ne!(
            fixture_fingerprint(variables.clone(), changed_fixed, constraints.clone()),
            base_fingerprint,
            "fixed value must be fingerprinted"
        );

        let replace_row = |id, row| {
            let mut changed = constraints.clone();
            changed.remove(&ConstraintID::from(10));
            changed.insert(id, row);
            fixture_fingerprint(variables.clone(), fixed.clone(), changed)
        };
        assert_ne!(
            replace_row(
                ConstraintID::from(10),
                Constraint::less_than_or_equal_to_zero(Function::from(
                    ((coeff!(2.0) * linear!(1)).unwrap() + coeff!(-1.0)).unwrap(),
                )),
            ),
            base_fingerprint,
            "coefficient must be fingerprinted"
        );
        assert_ne!(
            replace_row(
                ConstraintID::from(10),
                Constraint::less_than_or_equal_to_zero(Function::from(
                    (linear!(1) + coeff!(-2.0)).unwrap(),
                )),
            ),
            base_fingerprint,
            "constant must be fingerprinted"
        );
        assert_ne!(
            replace_row(
                ConstraintID::from(10),
                Constraint::equal_to_zero(Function::from((linear!(1) + coeff!(-1.0)).unwrap(),)),
            ),
            base_fingerprint,
            "sense must be fingerprinted"
        );
        assert_ne!(
            replace_row(
                ConstraintID::from(12),
                Constraint::less_than_or_equal_to_zero(Function::from(
                    (linear!(1) + coeff!(-1.0)).unwrap(),
                )),
            ),
            base_fingerprint,
            "row ID must be fingerprinted"
        );

        let mut without_row = constraints.clone();
        without_row.remove(&ConstraintID::from(11));
        assert_ne!(
            fixture_fingerprint(variables.clone(), fixed.clone(), without_row),
            base_fingerprint,
            "row presence must be fingerprinted"
        );

        let mut with_domain = variables.clone();
        with_domain.insert(VariableID::from(3), DecisionVariable::continuous());
        assert_ne!(
            fixture_fingerprint(with_domain, fixed.clone(), constraints.clone()),
            base_fingerprint,
            "domain presence and variable ID must be fingerprinted"
        );

        let mut renamed_variables = variables;
        let x = renamed_variables.remove(&VariableID::from(1)).unwrap();
        renamed_variables.insert(VariableID::from(3), x);
        let renamed_constraints = BTreeMap::from([
            (
                ConstraintID::from(10),
                Constraint::less_than_or_equal_to_zero(Function::from(
                    (linear!(3) + coeff!(-1.0)).unwrap(),
                )),
            ),
            (
                ConstraintID::from(11),
                Constraint::equal_to_zero(Function::from(linear!(3))),
            ),
        ]);
        assert_ne!(
            fixture_fingerprint(renamed_variables, fixed, renamed_constraints),
            base_fingerprint,
            "coefficient variable ID must be fingerprinted"
        );
    }

    #[test]
    fn fingerprint_is_independent_of_map_and_function_representation_order() {
        let base = base_instance();
        let base_fingerprint = base.certified_linear_relaxation().unwrap().fingerprint();

        let mut variables = BTreeMap::new();
        variables.insert(VariableID::from(2), DecisionVariable::continuous());
        variables.insert(
            VariableID::from(1),
            DecisionVariable::new(
                Kind::Continuous,
                Bound::new(0.0, 2.0).unwrap(),
                crate::ATol::default(),
            )
            .unwrap(),
        );
        let mut constraints = BTreeMap::new();
        constraints.insert(
            ConstraintID::from(11),
            Constraint::equal_to_zero(Function::from(crate::quadratic!(1))),
        );
        constraints.insert(
            ConstraintID::from(10),
            Constraint::less_than_or_equal_to_zero(Function::from(
                (crate::quadratic!(1) + coeff!(-1.0)).unwrap(),
            )),
        );
        assert_eq!(
            fixture_fingerprint(
                variables,
                BTreeMap::from([(VariableID::from(2), 0.5)]),
                constraints,
            ),
            base_fingerprint
        );
    }

    #[test]
    fn fingerprint_has_a_versioned_golden_encoding() {
        let fingerprint = base_instance()
            .certified_linear_relaxation()
            .unwrap()
            .fingerprint();
        assert_eq!(
            fingerprint.hex(),
            "292a65932984a44f556038deae3942f57464dcbc9e5c5aae0c70c0d409ecc7f4"
        );
    }

    #[test]
    fn exact_affine_substitution_is_algebraic() {
        let affine = ExactAffine::from_function(&Function::from(
            ((linear!(1) + linear!(2)).unwrap() + coeff!(-3.0)).unwrap(),
        ))
        .unwrap()
        .unwrap();
        let substituted = affine.substitute(VariableID::from(2), &exact(2));
        assert_eq!(substituted.coefficients.len(), 1);
        assert_eq!(substituted.coefficients[&VariableID::from(1)], exact(1));
        assert_eq!(substituted.constant, exact(-1));
        assert_eq!(substituted.negated().constant, exact(1));
    }
}
