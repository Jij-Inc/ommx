use super::{CertifiedLinearRelaxation, ExactAffine, ExactRational, LinearProofError};
use num::{Signed, Zero};

/// Exact interval activity of one affine expression over the proof-domain
/// facts in a [`CertifiedLinearRelaxation`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivityBounds {
    lower: Option<ExactRational>,
    upper: Option<ExactRational>,
}

impl ActivityBounds {
    pub fn lower(&self) -> Option<&ExactRational> {
        self.lower.as_ref()
    }

    pub fn upper(&self) -> Option<&ExactRational> {
        self.upper.as_ref()
    }
}

impl CertifiedLinearRelaxation {
    /// Evaluate exact lower and upper activity using only stored finite bound
    /// sides and fixed-value equations.
    ///
    /// A missing required side makes only that result side unbounded. No
    /// round-to-nearest `f64` operation or tolerance is used.
    pub fn activity_bounds(
        &self,
        affine: &ExactAffine,
    ) -> Result<ActivityBounds, LinearProofError> {
        let mut lower = Some(affine.constant().clone());
        let mut upper = Some(affine.constant().clone());

        for (id, coefficient) in affine.coefficients() {
            if coefficient.is_zero() {
                continue;
            }
            let domain = self
                .domains()
                .get(id)
                .ok_or(LinearProofError::UnknownDomain { id: *id })?;
            let (lower_value, upper_value) = if let Some(fixed) = domain.fixed() {
                (Some(fixed), Some(fixed))
            } else if coefficient.is_positive() {
                (domain.lower(), domain.upper())
            } else {
                debug_assert!(coefficient.is_negative());
                (domain.upper(), domain.lower())
            };

            lower = add_term(lower, coefficient, lower_value);
            upper = add_term(upper, coefficient, upper_value);
        }

        Ok(ActivityBounds { lower, upper })
    }

    /// Check whether variable-domain proof atoms imply `affine <= 0`.
    pub fn variable_facts_imply_nonpositive(
        &self,
        affine: &ExactAffine,
    ) -> Result<bool, LinearProofError> {
        Ok(self
            .activity_bounds(affine)?
            .upper()
            .is_some_and(|upper| upper <= &ExactRational::zero()))
    }
}

fn add_term(
    total: Option<ExactRational>,
    coefficient: &ExactRational,
    value: Option<&ExactRational>,
) -> Option<ExactRational> {
    Some(total? + coefficient * value?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        coeff, linear, Bound, Constraint, DecisionVariable, Function, Instance, Kind, Sense,
        VariableID,
    };
    use num::BigRational;
    use std::collections::BTreeMap;

    fn exact(value: i64) -> ExactRational {
        BigRational::from_integer(value.into())
    }

    fn bounded_instance(fixed: Option<f64>) -> Instance {
        let mut builder = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::zero())
            .decision_variables(BTreeMap::from([
                (
                    VariableID::from(1),
                    DecisionVariable::new(
                        Kind::Continuous,
                        Bound::new(-2.0, 3.0).unwrap(),
                        crate::ATol::default(),
                    )
                    .unwrap(),
                ),
                (VariableID::from(2), DecisionVariable::continuous()),
            ]))
            .constraints(BTreeMap::from([(
                crate::ConstraintID::from(1),
                Constraint::less_than_or_equal_to_zero(Function::from(linear!(1))),
            )]));
        if let Some(value) = fixed {
            builder = builder
                .objective(Function::zero())
                .fixed_decision_variable_values(BTreeMap::from([(VariableID::from(2), value)]));
        }
        builder.build().unwrap()
    }

    #[test]
    fn activity_uses_coefficient_signs_exactly() {
        let snapshot = bounded_instance(None)
            .certified_linear_relaxation()
            .unwrap();
        let affine = ExactAffine::from_function(&Function::from(
            ((coeff!(2.0) * linear!(1)).unwrap() + coeff!(-1.0)).unwrap(),
        ))
        .unwrap()
        .unwrap();
        let activity = snapshot.activity_bounds(&affine).unwrap();
        assert_eq!(activity.lower(), Some(&exact(-5)));
        assert_eq!(activity.upper(), Some(&exact(5)));

        let negated = snapshot.activity_bounds(&affine.negated()).unwrap();
        assert_eq!(negated.lower(), Some(&exact(-5)));
        assert_eq!(negated.upper(), Some(&exact(5)));
    }

    #[test]
    fn unbounded_coordinate_only_removes_the_required_side() {
        let snapshot = bounded_instance(None)
            .certified_linear_relaxation()
            .unwrap();
        let affine = ExactAffine::from_function(&Function::from(linear!(2)))
            .unwrap()
            .unwrap();
        let activity = snapshot.activity_bounds(&affine).unwrap();
        assert_eq!(activity.lower(), None);
        assert_eq!(activity.upper(), None);
        assert!(!snapshot.variable_facts_imply_nonpositive(&affine).unwrap());
    }

    #[test]
    fn fixed_equation_supplies_both_activity_sides() {
        let snapshot = bounded_instance(Some(0.25))
            .certified_linear_relaxation()
            .unwrap();
        let affine = ExactAffine::from_function(&Function::from(linear!(2)))
            .unwrap()
            .unwrap();
        let activity = snapshot.activity_bounds(&affine).unwrap();
        let expected = crate::proof::exact::from_f64(0.25).unwrap();
        assert_eq!(activity.lower(), Some(&expected));
        assert_eq!(activity.upper(), Some(&expected));
    }

    #[test]
    fn fixed_equation_takes_precedence_over_finite_bounds() {
        let instance = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::zero())
            .decision_variables(BTreeMap::from([(
                VariableID::from(1),
                DecisionVariable::new(
                    Kind::Continuous,
                    Bound::new(-4.0, 6.0).unwrap(),
                    crate::ATol::default(),
                )
                .unwrap(),
            )]))
            .fixed_decision_variable_values(BTreeMap::from([(VariableID::from(1), 0.25)]))
            .constraints(BTreeMap::new())
            .build()
            .unwrap();
        let snapshot = instance.certified_linear_relaxation().unwrap();
        let affine = ExactAffine::coordinate(VariableID::from(1), 1, ExactRational::zero());
        let activity = snapshot.activity_bounds(&affine).unwrap();
        let expected = crate::proof::exact::from_f64(0.25).unwrap();
        assert_eq!(activity.lower(), Some(&expected));
        assert_eq!(activity.upper(), Some(&expected));
    }

    #[test]
    fn one_sided_domains_propagate_by_coefficient_sign() {
        let instance = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::zero())
            .decision_variables(BTreeMap::from([
                (
                    VariableID::from(1),
                    DecisionVariable::new(
                        Kind::Continuous,
                        Bound::new(2.0, f64::INFINITY).unwrap(),
                        crate::ATol::default(),
                    )
                    .unwrap(),
                ),
                (
                    VariableID::from(2),
                    DecisionVariable::new(
                        Kind::Continuous,
                        Bound::new(f64::NEG_INFINITY, 5.0).unwrap(),
                        crate::ATol::default(),
                    )
                    .unwrap(),
                ),
            ]))
            .constraints(BTreeMap::new())
            .build()
            .unwrap();
        let snapshot = instance.certified_linear_relaxation().unwrap();

        let cases = [
            (VariableID::from(1), 3, Some(exact(6)), None),
            (VariableID::from(1), -3, None, Some(exact(-6))),
            (VariableID::from(2), 4, None, Some(exact(20))),
            (VariableID::from(2), -4, Some(exact(-20)), None),
        ];
        for (id, coefficient, expected_lower, expected_upper) in cases {
            let affine = ExactAffine::coordinate(id, coefficient, ExactRational::zero());
            let activity = snapshot.activity_bounds(&affine).unwrap();
            assert_eq!(activity.lower(), expected_lower.as_ref());
            assert_eq!(activity.upper(), expected_upper.as_ref());
        }
    }

    #[test]
    fn implication_has_no_tolerance() {
        let snapshot = bounded_instance(None)
            .certified_linear_relaxation()
            .unwrap();
        let implied =
            ExactAffine::from_function(&Function::from((linear!(1) + coeff!(-3.0)).unwrap()))
                .unwrap()
                .unwrap();
        assert!(snapshot.variable_facts_imply_nonpositive(&implied).unwrap());

        let not_implied = ExactAffine::from_function(&Function::from(
            (linear!(1) + coeff!(-2.9999999999999996)).unwrap(),
        ))
        .unwrap()
        .unwrap();
        assert!(!snapshot
            .variable_facts_imply_nonpositive(&not_implied)
            .unwrap());
    }

    #[test]
    fn zero_coefficient_does_not_require_a_domain_or_bound() {
        let snapshot = bounded_instance(None)
            .certified_linear_relaxation()
            .unwrap();
        let affine = ExactAffine {
            coefficients: BTreeMap::from([(VariableID::from(999), ExactRational::zero())]),
            constant: exact(4),
        };
        let activity = snapshot.activity_bounds(&affine).unwrap();
        assert_eq!(activity.lower(), Some(&exact(4)));
        assert_eq!(activity.upper(), Some(&exact(4)));
    }

    #[test]
    fn nonzero_unknown_coordinate_fails_closed() {
        let snapshot = bounded_instance(None)
            .certified_linear_relaxation()
            .unwrap();
        let affine = ExactAffine::coordinate(VariableID::from(999), 1, ExactRational::zero());
        assert!(matches!(
            snapshot.activity_bounds(&affine),
            Err(LinearProofError::UnknownDomain { id }) if id == VariableID::from(999)
        ));
    }
}
