use super::Instance;
use crate::{
    ATol, Bound, Bounds, ConstraintID, Equality, Evaluate, Kind, LinearMonomial, VariableIDSet,
};
use std::collections::BTreeSet;

impl Instance {
    /// Apply one simultaneous bound-tightening pass using all active regular constraints.
    ///
    /// This is the convenience form of
    /// [`Self::tighten_bounds_simultaneously_once_using_constraints`] with every
    /// active regular constraint ID. Non-affine rows are skipped. Every row
    /// reads the bounds at entry, and all updates are applied together once.
    /// Returns the new bounds of the variables actually changed.
    ///
    /// The same tolerance, supported-domain and atomicity rules apply as for
    /// the explicitly selected form.
    pub fn tighten_bounds_simultaneously_once(&mut self, atol: ATol) -> crate::Result<Bounds> {
        let rows = self.constraints().keys().copied().collect();
        self.tighten_bounds_simultaneously_once_using_constraints(&rows, atol)
    }

    /// Apply one simultaneous bound-tightening pass using selected regular constraints.
    ///
    /// `constraint_ids` contains active regular constraint IDs. Unknown and
    /// removed IDs are errors; an empty set applies no updates. Non-affine
    /// selected rows are skipped, as in [`Self::tighten_bounds_simultaneously_once`].
    /// All eligible variables in the selected rows may have their bounds tightened.
    ///
    /// For each variable in `a*x + r <= 0`, minimize `r` over the current
    /// variable domains and derive an upper or lower bound on `x`. Equalities
    /// are processed in both directions. Every row reads the bounds at entry;
    /// all updates are collected and applied together. Newly tightened bounds
    /// are not reused during this call. Call again to propagate the resulting
    /// bounds through further rows.
    /// Returns the new bounds of the variables actually changed.
    ///
    /// This preserves feasibility under the supplied `atol`, including the
    /// tolerance on continuous variable bounds and on constraint residuals.
    /// Integer and binary domains use canonical discrete values. Use the same
    /// tolerance for subsequent evaluation. Like [`Self::clip_bounds`], changes
    /// within `atol` are ignored.
    ///
    /// Only the selected active regular constraints are used.
    /// Each upper/lower candidate is skipped if its required residual or
    /// boundary calculation is non-finite. Other candidates from that row are
    /// still processed. Unbounded variable domains remain infinite. Semi-variable
    /// domains include zero when deriving bounds on other variables, but their
    /// own bounds are not changed. Fixed and dependent variables are not changed.
    /// This is a conservative propagation pass, not a complete infeasibility
    /// detector or a fixed-point presolver.
    ///
    /// Requires finite `atol < 1`. If a contradiction or an invalid update is
    /// detected, no changes are applied.
    pub fn tighten_bounds_simultaneously_once_using_constraints(
        &mut self,
        constraint_ids: &BTreeSet<ConstraintID>,
        atol: ATol,
    ) -> crate::Result<Bounds> {
        let targets = self.decision_variables().keys().copied().collect();
        let bounds = infer_bounds_simultaneously_once(self, constraint_ids, &targets, atol)?;
        self.clip_bounds(&bounds, atol)?;
        Ok(bounds)
    }
}

// This module is private to Instance. Keep inference read-only so owner
// operations can validate all updates before committing their effects.
pub fn infer_bounds_simultaneously_once(
    instance: &Instance,
    rows: &BTreeSet<ConstraintID>,
    targets: &VariableIDSet,
    atol: ATol,
) -> crate::Result<Bounds> {
    if !atol.into_inner().is_finite() || atol.into_inner() >= 1.0 {
        crate::bail!("Bound tightening requires a finite ATol smaller than one");
    }
    let mut updates = std::collections::BTreeMap::new();
    for &row_id in rows {
        let row = instance.constraints().get(&row_id).ok_or_else(
            || crate::error!({ ?row_id }, "Bound tightening constraint {row_id:?} is not active"),
        )?;
        let Some(linear) = row.function().as_linear() else {
            continue;
        };
        let mut minimum = crate::v1::State::default();
        let mut maximum = crate::v1::State::default();
        for (monomial, coefficient) in linear.iter() {
            let LinearMonomial::Variable(id) = monomial else {
                continue;
            };
            let (lower, upper) = instance
                .decision_variable_domain_bounds(*id, atol)
                .expect("constraint variables are registered in the instance");
            let (min, max) = if coefficient.into_inner() > 0.0 {
                (lower, upper)
            } else {
                (upper, lower)
            };
            minimum.entries.insert(id.into_inner(), min);
            maximum.entries.insert(id.into_inner(), max);
        }
        let directions: &[f64] = match row.equality {
            Equality::LessThanOrEqualToZero => &[1.0],
            Equality::EqualToZero => &[1.0, -1.0],
        };
        for &direction in directions {
            let best = if direction > 0.0 { &minimum } else { &maximum };
            let best_value = direction * linear.evaluate(best, atol)?;
            // A finite row minimum can prove infeasibility. A non-finite
            // minimum says nothing about the individual bound candidates.
            if best_value.is_finite() && best_value > atol.into_inner() {
                crate::bail!({ ?row_id, best_value }, "Bound tightening found constraint {row_id:?} infeasible over the current variable domains");
            }
            for (monomial, coefficient) in linear.iter() {
                let LinearMonomial::Variable(id) = monomial else {
                    continue;
                };
                let original = &instance.decision_variables()[id];
                if !targets.contains(id)
                    || matches!(original.kind(), Kind::SemiContinuous | Kind::SemiInteger)
                    || instance.fixed_decision_variable_values().contains_key(id)
                    || instance.decision_variable_dependency.get(id).is_some()
                {
                    continue;
                }
                let sign = (direction * coefficient.into_inner()).signum();
                let first = sign * best.entries[&id.into_inner()];
                let mut state = best.clone();
                // Exclude the target term before evaluating the residual.
                // Its own domain may be unbounded or its extremal product may
                // overflow, without preventing a finite bound on this side.
                state.entries.insert(id.into_inner(), 0.0);
                let residual = direction * linear.evaluate(&state, atol)?;
                if !residual.is_finite() {
                    continue;
                }
                let numerator = atol.into_inner() - residual;
                if !numerator.is_finite() {
                    continue;
                }
                let estimate = numerator / coefficient.into_inner().abs();
                if !estimate.is_finite() {
                    continue;
                }
                let mut feasible = |value| {
                    state.entries.insert(id.into_inner(), sign * value);
                    let residual = direction
                        * linear
                            .evaluate(&state, atol)
                            .expect("complete affine state");
                    residual
                        .is_finite()
                        .then_some(residual <= atol.into_inner())
                };
                let Some(extremum) = last_satisfying_near(estimate, &mut feasible) else {
                    continue;
                };
                let endpoint = match original.kind() {
                    Kind::Continuous => {
                        let Some(endpoint) = upper_endpoint_containing(extremum, atol) else {
                            continue;
                        };
                        endpoint
                    }
                    Kind::Integer | Kind::Binary => extremum.floor(),
                    _ => unreachable!(),
                };
                // A finite stored endpoint must also have a finite residual
                // at the opposite end of the feasible projection. Otherwise
                // replacing an infinite endpoint could reject valid extreme
                // values merely because Bound::contains overflows.
                // For an unbounded opposite side, test the most extreme
                // representable state only for this overflow guard. This is
                // not an endpoint used in interval inference or searching.
                let residual_is_finite = if first == f64::NEG_INFINITY {
                    (-f64::MAX - endpoint).is_finite()
                } else {
                    (first - endpoint).is_finite()
                };
                if !residual_is_finite {
                    continue;
                }
                let updated = updates.entry(*id).or_insert_with(|| original.clone());
                let current = updated.bound();
                let bound = if sign > 0.0 {
                    let mut upper = endpoint.min(current.upper());
                    // Continuous residual bounds may overlap only within
                    // tolerance. Keep a conservative point interval in that
                    // case rather than falsely diagnosing infeasibility.
                    if original.kind() == Kind::Continuous {
                        upper = upper.max(current.lower());
                    }
                    Bound::new(current.lower(), upper)
                } else {
                    let mut lower = (-endpoint).max(current.lower());
                    if original.kind() == Kind::Continuous {
                        lower = lower.min(current.upper());
                    }
                    Bound::new(lower, current.upper())
                }.map_err(|error| {
                    crate::error!({ ?row_id, ?id, %error }, "Bound tightening found incompatible bounds for variable {id:?} from constraint {row_id:?}: {error}")
                })?;
                updated.clip_bound(*id, bound, atol)?;
            }
        }
    }
    Ok(updates
        .into_iter()
        .filter_map(|(id, variable)| {
            (variable.bound() != instance.decision_variables()[&id].bound())
                .then_some((id, variable.bound()))
        })
        .collect())
}

fn ordered_key(value: f64) -> u64 {
    let bits = value.to_bits();
    if value.is_sign_negative() {
        !bits
    } else {
        bits | (1 << 63)
    }
}

fn from_ordered_key(key: u64) -> f64 {
    f64::from_bits(if key & (1 << 63) == 0 {
        !key
    } else {
        key & !(1 << 63)
    })
}

// The residual interval gives a finite estimate. Refine near that estimate
// using the original evaluator, whose rounding/order can move the boundary.
// Do not synthesize finite endpoints for unbounded domains. A non-finite probe
// abandons only this candidate. Exponential bracketing uses representable-value
// distances so cancellation around zero does not require a walk over every ULP.
fn last_satisfying_near(
    estimate: f64,
    mut predicate: impl FnMut(f64) -> Option<bool>,
) -> Option<f64> {
    if !estimate.is_finite() {
        return None;
    }
    let initial = predicate(estimate)?;
    let key = ordered_key(estimate);
    let mut distance = 1_u64;
    let (mut low, mut high) = loop {
        let probe = if initial {
            key.checked_add(distance)?
        } else {
            key.checked_sub(distance)?
        };
        let value = from_ordered_key(probe);
        if !value.is_finite() {
            return None;
        }
        if predicate(value)? != initial {
            break if initial { (key, probe) } else { (probe, key) };
        }
        distance = distance.checked_mul(2)?;
    };
    while high - low > 1 {
        let middle = low + (high - low) / 2;
        if predicate(from_ordered_key(middle))? {
            low = middle;
        } else {
            high = middle;
        }
    }
    Some(from_ordered_key(low))
}

fn upper_endpoint_containing(value: f64, atol: ATol) -> Option<f64> {
    // A stored upper bound contributes its own residual tolerance. Invert
    // that comparison so tightening does not add a second ATol to the row.
    let estimate = value - atol.into_inner();
    let least_endpoint = -last_satisfying_near(-estimate, |negative_endpoint| {
        let residual = value + negative_endpoint;
        residual
            .is_finite()
            .then_some(residual <= atol.into_inner())
    })?;
    // Prefer the ordinary subtraction when it is conservative. In particular,
    // an exact zero endpoint stays zero rather than a tiny negative value
    // whose difference is hidden by rounding the residual.
    Some(estimate.max(least_endpoint))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{coeff, linear, Constraint, DecisionVariable, Function, Sense};
    use std::collections::BTreeMap;

    fn continuous(lower: f64, upper: f64) -> DecisionVariable {
        DecisionVariable::continuous()
            .with_bound(Bound::new(lower, upper).unwrap(), ATol::default())
            .unwrap()
    }

    fn instance(variables: Vec<DecisionVariable>, rows: Vec<Constraint>) -> Instance {
        Instance::new(
            Sense::Minimize,
            Function::Zero,
            variables
                .into_iter()
                .enumerate()
                .map(|(i, v)| ((i as u64).into(), v))
                .collect(),
            rows.into_iter()
                .enumerate()
                .map(|(i, row)| ((i as u64).into(), row))
                .collect(),
        )
        .unwrap()
    }

    #[test]
    fn selected_constraints_control_which_bounds_are_tightened() {
        let mut selected = instance(
            vec![continuous(-10.0, 10.0)],
            vec![
                Constraint::less_than_or_equal_to_zero((linear!(0) - coeff!(2.0)).unwrap().into()),
                Constraint::less_than_or_equal_to_zero((-linear!(0) - coeff!(3.0)).unwrap().into()),
            ],
        );
        let mut all = selected.clone();
        let mut explicit_all = selected.clone();
        let atol = ATol::new(0.125).unwrap();
        let bounds = selected
            .tighten_bounds_simultaneously_once_using_constraints(&BTreeSet::from([0.into()]), atol)
            .unwrap();
        assert_eq!(bounds[&0.into()], Bound::new(-10.0, 2.0).unwrap());
        let all_bounds = all.tighten_bounds_simultaneously_once(atol).unwrap();
        assert_eq!(all_bounds[&0.into()], Bound::new(-3.0, 2.0).unwrap());
        assert_eq!(
            explicit_all
                .tighten_bounds_simultaneously_once_using_constraints(
                    &BTreeSet::from([0.into(), 1.into()]),
                    atol
                )
                .unwrap(),
            all_bounds
        );
        assert_eq!(explicit_all, all);
        let before_empty = selected.clone();
        assert!(selected
            .tighten_bounds_simultaneously_once_using_constraints(&BTreeSet::new(), atol)
            .unwrap()
            .is_empty());
        assert_eq!(selected, before_empty);
    }

    #[test]
    fn unknown_and_removed_selected_constraints_are_atomic_errors() {
        let mut problem = instance(
            vec![continuous(-10.0, 10.0)],
            vec![
                Constraint::less_than_or_equal_to_zero((linear!(0) - coeff!(2.0)).unwrap().into()),
                Constraint::less_than_or_equal_to_zero(linear!(0).into()),
            ],
        );
        problem
            .relax_constraint(1.into(), "test".to_string(), [])
            .unwrap();
        let original = problem.clone();
        for invalid in [1_u64, 999] {
            let error = problem
                .tighten_bounds_simultaneously_once_using_constraints(
                    &BTreeSet::from([0.into(), invalid.into()]),
                    ATol::default(),
                )
                .unwrap_err();
            assert!(error.to_string().contains("is not active"));
            assert_eq!(problem, original);
        }
    }

    #[test]
    fn tightens_both_big_m_links_and_preserves_boundary_feasibility() {
        let atol = ATol::new(0.125).unwrap();
        let mut problem = instance(
            vec![continuous(-100.0, 100.0), DecisionVariable::binary()],
            vec![
                Constraint::less_than_or_equal_to_zero(
                    (linear!(0) - (coeff!(3.0) * linear!(1)).unwrap())
                        .unwrap()
                        .into(),
                ),
                Constraint::less_than_or_equal_to_zero(
                    (-linear!(0) - (coeff!(2.0) * linear!(1)).unwrap())
                        .unwrap()
                        .into(),
                ),
            ],
        );
        let original = problem.clone();
        let bounds = problem.tighten_bounds_simultaneously_once(atol).unwrap();
        assert_eq!(
            bounds,
            BTreeMap::from([(0.into(), Bound::new(-2.0, 3.0).unwrap())])
        );
        assert!(problem
            .tighten_bounds_simultaneously_once(atol)
            .unwrap()
            .is_empty());
        for value in [
            -2.125_f64,
            -2.125_f64.next_down(),
            3.125,
            3.125_f64.next_up(),
            0.125,
            0.125_f64.next_up(),
        ] {
            for z in [0.0, 1.0] {
                let state = crate::v1::State::from_iter([(0, value), (1, z)]);
                assert_eq!(
                    original.evaluate(&state, atol).unwrap().feasible(),
                    problem.evaluate(&state, atol).unwrap().feasible()
                );
            }
        }
    }

    #[test]
    fn equality_uses_both_sides_and_other_variables_bound_tolerance() {
        let mut problem = instance(
            vec![continuous(0.0, 10.0), continuous(1.0, 2.0)],
            vec![Constraint::equal_to_zero(
                ((linear!(0) + (coeff!(2.0) * linear!(1)).unwrap()).unwrap() - coeff!(5.0))
                    .unwrap()
                    .into(),
            )],
        );
        let bounds = problem
            .tighten_bounds_simultaneously_once(ATol::new(0.125).unwrap())
            .unwrap();
        let bound = bounds[&0.into()];
        // The affine evaluator adds the constant first. Its cancellation can
        // admit points beyond the real-arithmetic [.625, 3.375] projection.
        assert!(bound.lower() <= 0.75 && bound.upper() >= 3.25);
        assert!((bound.lower() - 0.75).abs() < 1e-14);
        assert!((bound.upper() - 3.25).abs() < 1e-14);
        let original_row = problem.constraints()[&0.into()].function();
        for (x, y) in [(0.625_f64.next_down(), 2.125), (3.375_f64.next_up(), 0.875)] {
            let residual = original_row
                .evaluate(
                    &crate::v1::State::from_iter([(0, x), (1, y)]),
                    ATol::new(0.125).unwrap(),
                )
                .unwrap();
            assert!(residual.abs() <= 0.125);
            assert!(bound.contains(x, ATol::new(0.125).unwrap()));
        }
    }

    #[test]
    fn integer_rounding_and_chained_passes() {
        let mut problem = instance(
            vec![
                DecisionVariable::new(
                    Kind::Integer,
                    Bound::new(-10.0, 10.0).unwrap(),
                    ATol::default(),
                )
                .unwrap(),
                continuous(-10.0, 10.0),
            ],
            vec![
                Constraint::less_than_or_equal_to_zero(
                    ((coeff!(2.0) * linear!(0)).unwrap() - coeff!(5.0))
                        .unwrap()
                        .into(),
                ),
                Constraint::less_than_or_equal_to_zero((linear!(1) - linear!(0)).unwrap().into()),
            ],
        );
        let mut selected = problem.clone();
        let first = problem
            .tighten_bounds_simultaneously_once(ATol::new(0.125).unwrap())
            .unwrap();
        assert_eq!(first[&0.into()].upper(), 2.0);
        assert!(!first.contains_key(&1.into()));
        let selected_first = selected
            .tighten_bounds_simultaneously_once_using_constraints(
                &BTreeSet::from([0.into(), 1.into()]),
                ATol::new(0.125).unwrap(),
            )
            .unwrap();
        assert_eq!(selected_first, first);
        assert_eq!(selected, problem);
        let second = problem
            .tighten_bounds_simultaneously_once(ATol::new(0.125).unwrap())
            .unwrap();
        assert_eq!(second[&1.into()].upper(), 2.0);
    }

    #[test]
    fn contradiction_is_atomic_and_invalid_tolerance_is_rejected() {
        let mut problem = instance(
            vec![continuous(0.0, 10.0)],
            vec![
                Constraint::less_than_or_equal_to_zero((linear!(0) - coeff!(2.0)).unwrap().into()),
                Constraint::less_than_or_equal_to_zero(Function::from(coeff!(1.0))),
            ],
        );
        let original = problem.clone();
        assert!(problem
            .tighten_bounds_simultaneously_once(ATol::default())
            .is_err());
        assert_eq!(problem, original);
        for tolerance in [1.0, f64::INFINITY] {
            assert!(problem
                .tighten_bounds_simultaneously_once(ATol::new(tolerance).unwrap())
                .is_err());
            assert_eq!(problem, original);
        }
    }

    #[test]
    fn unbounded_member_can_be_tightened_and_semi_domain_includes_zero() {
        let semi = DecisionVariable::new(
            Kind::SemiContinuous,
            Bound::new(2.0, 3.0).unwrap(),
            ATol::default(),
        )
        .unwrap();
        let mut problem = instance(
            vec![DecisionVariable::continuous(), semi],
            vec![Constraint::less_than_or_equal_to_zero(
                (linear!(0) + linear!(1)).unwrap().into(),
            )],
        );
        let bounds = problem
            .tighten_bounds_simultaneously_once(ATol::new(0.125).unwrap())
            .unwrap();
        assert_eq!(bounds[&0.into()].upper(), 0.0);
        assert!(!bounds.contains_key(&1.into()));
    }

    #[test]
    fn ignores_nonlinear_and_removed_rows_but_uses_a_finite_candidate() {
        let mut problem = instance(
            vec![continuous(-10.0, 10.0)],
            vec![
                Constraint::less_than_or_equal_to_zero((linear!(0) * linear!(0)).into()),
                Constraint::less_than_or_equal_to_zero(linear!(0).into()),
                Constraint::less_than_or_equal_to_zero(
                    (coeff!(f64::MAX) * linear!(0)).unwrap().into(),
                ),
            ],
        );
        problem
            .relax_constraint(1.into(), "test".to_string(), [])
            .unwrap();
        let original = problem.clone();
        let atol = ATol::default();
        let bounds = problem.tighten_bounds_simultaneously_once(atol).unwrap();
        // MAX*x overflows at the old endpoints, but excluding x leaves zero.
        assert_eq!(
            bounds[&0.into()],
            Bound::new(-10.0, -atol.into_inner()).unwrap()
        );
        for value in [-1e-308, 0.0, 1e-315, 1e-307] {
            let state = crate::v1::State::from_iter([(0, value)]);
            assert_eq!(
                original.evaluate(&state, atol).unwrap().feasible(),
                problem.evaluate(&state, atol).unwrap().feasible()
            );
        }
    }

    #[test]
    fn unbounded_target_uses_only_the_other_terms_for_each_side() {
        let atol = ATol::new(0.125).unwrap();
        for sign in [-1.0, 1.0] {
            for kind in [Kind::Continuous, Kind::Integer] {
                let coefficient = crate::Coefficient::try_from(2.0 * sign).unwrap();
                let mut problem = instance(
                    vec![DecisionVariable::new(kind, Bound::unbounded(), atol).unwrap()],
                    vec![Constraint::less_than_or_equal_to_zero(
                        ((coefficient * linear!(0)).unwrap() - coeff!(6.0))
                            .unwrap()
                            .into(),
                    )],
                );
                let original = problem.clone();
                let bounds = problem.tighten_bounds_simultaneously_once(atol).unwrap();
                let endpoint = if kind == Kind::Integer { 3.0 } else { 2.9375 };
                let expected = if sign > 0.0 {
                    Bound::new(f64::NEG_INFINITY, endpoint).unwrap()
                } else {
                    Bound::new(-endpoint, f64::INFINITY).unwrap()
                };
                assert_eq!(bounds[&0.into()], expected);
                for value in [-4.0, -3.0625, -3.0, 0.0, 3.0, 3.0625, 4.0] {
                    let state = crate::v1::State::from_iter([(0, value)]);
                    assert_eq!(
                        original.evaluate(&state, atol).unwrap().feasible(),
                        problem.evaluate(&state, atol).unwrap().feasible()
                    );
                }
            }
        }
    }

    #[test]
    fn equality_retains_a_finite_upper_candidate_when_other_candidates_are_infinite() {
        let mut problem = instance(
            vec![
                DecisionVariable::continuous(),
                continuous(1.0, f64::INFINITY),
            ],
            vec![Constraint::equal_to_zero(
                ((linear!(0) + linear!(1)).unwrap() - coeff!(6.0))
                    .unwrap()
                    .into(),
            )],
        );
        let bounds = problem
            .tighten_bounds_simultaneously_once(ATol::new(0.125).unwrap())
            .unwrap();
        assert_eq!(
            bounds,
            BTreeMap::from([(0.into(), Bound::new(f64::NEG_INFINITY, 5.125).unwrap(),)])
        );
        assert_eq!(
            problem.decision_variables()[&1.into()].bound(),
            Bound::new(1.0, f64::INFINITY).unwrap()
        );
    }

    #[test]
    fn nonfinite_residual_or_division_discards_only_its_candidate() {
        let mut overflowing_residuals = instance(
            vec![continuous(2.0, 3.0), continuous(2.0, 3.0)],
            vec![Constraint::less_than_or_equal_to_zero(
                ((coeff!(f64::MAX) * linear!(0)).unwrap()
                    + (coeff!(f64::MAX) * linear!(1)).unwrap())
                .unwrap()
                .into(),
            )],
        );
        let original = overflowing_residuals.clone();
        assert!(overflowing_residuals
            .tighten_bounds_simultaneously_once(ATol::default())
            .unwrap()
            .is_empty());
        assert_eq!(overflowing_residuals, original);

        let mut overflowing_division = instance(
            vec![
                DecisionVariable::continuous(),
                DecisionVariable::continuous(),
            ],
            vec![
                Constraint::less_than_or_equal_to_zero(
                    ((coeff!(1e-308) * linear!(0)).unwrap() - coeff!(f64::MAX))
                        .unwrap()
                        .into(),
                ),
                Constraint::less_than_or_equal_to_zero((linear!(1) - coeff!(3.0)).unwrap().into()),
            ],
        );
        let bounds = overflowing_division
            .tighten_bounds_simultaneously_once(ATol::new(0.125).unwrap())
            .unwrap();
        assert_eq!(
            bounds,
            BTreeMap::from([(1.into(), Bound::new(f64::NEG_INFINITY, 3.0).unwrap(),)])
        );
        assert_eq!(
            overflowing_division.decision_variables()[&0.into()].bound(),
            Bound::unbounded()
        );
    }

    #[test]
    fn unbounded_target_preserves_cancellation_in_the_original_evaluation_order() {
        let atol = ATol::new(0.125).unwrap();
        let mut problem = instance(
            vec![DecisionVariable::continuous(), continuous(1e16, 1e16)],
            vec![Constraint::less_than_or_equal_to_zero(
                ((linear!(0) - linear!(1)).unwrap() + coeff!(1e16))
                    .unwrap()
                    .into(),
            )],
        );
        let original = problem.clone();
        let bounds = problem.tighten_bounds_simultaneously_once(atol).unwrap();
        assert_eq!(
            bounds[&0.into()],
            Bound::new(f64::NEG_INFINITY, 0.875).unwrap()
        );
        for value in [0.125, 1.0, 1.0_f64.next_up()] {
            let state = crate::v1::State::from_iter([(0, value), (1, 1e16)]);
            assert_eq!(
                original.evaluate(&state, atol).unwrap().feasible(),
                problem.evaluate(&state, atol).unwrap().feasible()
            );
        }
    }

    #[test]
    fn a_new_finite_endpoint_must_not_overflow_at_the_opposite_extreme() {
        for sign in [-1.0, 1.0] {
            let row = ((crate::Coefficient::try_from(sign * 1e-308).unwrap() * linear!(0))
                .unwrap()
                - coeff!(1.0))
            .unwrap();
            let mut problem = instance(
                vec![DecisionVariable::continuous()],
                vec![Constraint::less_than_or_equal_to_zero(row.into())],
            );
            let state = crate::v1::State::from_iter([(0, -sign * f64::MAX)]);
            assert!(problem
                .evaluate(&state, ATol::default())
                .unwrap()
                .feasible());
            assert!(problem
                .tighten_bounds_simultaneously_once(ATol::default())
                .unwrap()
                .is_empty());
            assert!(problem
                .evaluate(&state, ATol::default())
                .unwrap()
                .feasible());
        }
    }

    #[test]
    fn fixed_and_dependent_domains_are_preserved() {
        let bound = Bound::new(-10.0, 10.0).unwrap();
        let mut problem = Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::Zero)
            .decision_variables(
                (0_u64..3)
                    .map(|id| (id.into(), continuous(-10.0, 10.0)))
                    .collect(),
            )
            .fixed_decision_variable_values(BTreeMap::from([(0.into(), 3.0)]))
            .decision_variable_dependency(
                crate::AcyclicAssignments::new([(1.into(), Function::from(linear!(2)))]).unwrap(),
            )
            .constraints(BTreeMap::from([(
                0.into(),
                Constraint::less_than_or_equal_to_zero(linear!(2).into()),
            )]))
            .build()
            .unwrap();
        let bounds = problem
            .tighten_bounds_simultaneously_once(ATol::default())
            .unwrap();
        assert_eq!(bounds.keys().copied().collect::<Vec<_>>(), vec![2.into()]);
        assert_eq!(problem.decision_variables()[&0.into()].bound(), bound);
        assert_eq!(problem.decision_variables()[&1.into()].bound(), bound);
        assert_eq!(problem.fixed_decision_variable_value(0.into()), Some(3.0));
    }

    proptest::proptest! {
        #[test]
        fn affine_tightening_preserves_feasible_states(
            a in -10.0_f64..10.0, b in -10.0_f64..10.0, c in -10.0_f64..10.0,
            x in -1.125_f64..1.125, y in -1.125_f64..1.125,
        ) {
            let mut row = crate::Linear::default();
            for (monomial, value) in [(LinearMonomial::Variable(0.into()), a), (LinearMonomial::Variable(1.into()), b), (LinearMonomial::Constant, c)] {
                if let Ok(coefficient) = crate::Coefficient::try_from(value) {
                    row.add_term(monomial, coefficient).unwrap();
                }
            }
            let mut problem = instance(vec![continuous(-1.0, 1.0), continuous(-1.0, 1.0)], vec![Constraint::less_than_or_equal_to_zero(row.into())]);
            let original = problem.clone();
            let atol = ATol::new(0.125).unwrap();
            let state = crate::v1::State::from_iter([(0, x), (1, y)]);
            if original.evaluate(&state, atol).unwrap().feasible() {
                problem.tighten_bounds_simultaneously_once(atol).unwrap();
                proptest::prop_assert!(problem.evaluate(&state, atol).unwrap().feasible());
            }
        }

        #[test]
        fn partially_unbounded_domains_preserve_feasible_states(
            a in -10.0_f64..10.0, b in -10.0_f64..10.0, c in -10.0_f64..10.0,
            x in -10.0_f64..10.0, y in -10.0_f64..10.0,
            infinite_sides in proptest::array::uniform4(proptest::bool::ANY),
        ) {
            let mut row = crate::Linear::default();
            for (monomial, value) in [(LinearMonomial::Variable(0.into()), a), (LinearMonomial::Variable(1.into()), b), (LinearMonomial::Constant, c)] {
                if let Ok(coefficient) = crate::Coefficient::try_from(value) {
                    row.add_term(monomial, coefficient).unwrap();
                }
            }
            let mut problem = instance(
                infinite_sides.chunks_exact(2).map(|sides| continuous(
                    if sides[0] { f64::NEG_INFINITY } else { -1.0 },
                    if sides[1] { f64::INFINITY } else { 1.0 },
                )).collect(),
                vec![Constraint::less_than_or_equal_to_zero(row.into())],
            );
            let original = problem.clone();
            let atol = ATol::new(0.125).unwrap();
            let state = crate::v1::State::from_iter([(0, x), (1, y)]);
            if original.evaluate(&state, atol).unwrap().feasible() {
                problem.tighten_bounds_simultaneously_once(atol).unwrap();
                proptest::prop_assert!(problem.evaluate(&state, atol).unwrap().feasible());
            }
        }
    }
}
