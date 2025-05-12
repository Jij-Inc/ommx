use super::*;
use crate::{v1::State, Bound, Evaluate, Kind};
use ::approx::AbsDiffEq;
use fnv::FnvHashSet;

/// The result of analyzing the decision variables in an instance.
///
/// Invariants
/// -----------
/// - Every field are subset of `all`.
/// - `binary`, `integer`, `continuous`, `semi_integer`, and `semi_continuous`
///   are disjoint, and their union is equal to `all`.
/// - The union of `used_in_objective` and `used_in_constraints` (= `used`), `fixed`,
///   and `dependent` are disjoint each other.
#[derive(Debug, Clone, PartialEq, getset::Getters)]
pub struct DecisionVariableAnalysis {
    /// The IDs of all decision variables
    #[getset(get = "pub")]
    all: FnvHashSet<VariableID>,

    #[getset(get = "pub")]
    binary: FnvHashSet<VariableID>,
    #[getset(get = "pub")]
    integer: FnvHashMap<VariableID, Bound>,
    #[getset(get = "pub")]
    continuous: FnvHashMap<VariableID, Bound>,
    #[getset(get = "pub")]
    semi_integer: FnvHashMap<VariableID, Bound>,
    #[getset(get = "pub")]
    semi_continuous: FnvHashMap<VariableID, Bound>,

    /// The set of decision variables that are used in the objective function.
    #[getset(get = "pub")]
    used_in_objective: FnvHashSet<VariableID>,
    /// The set of decision variables that are used in the constraints.
    #[getset(get = "pub")]
    used_in_constraints: FnvHashMap<ConstraintID, FnvHashSet<VariableID>>,

    /// Fixed decision variables
    #[getset(get = "pub")]
    fixed: FnvHashMap<VariableID, f64>,
    /// Dependent variables
    #[getset(get = "pub")]
    dependent: FnvHashSet<VariableID>,
}

impl DecisionVariableAnalysis {
    /// Union of `used_in_objective` and `used_in_constraints`
    pub fn used(&self) -> FnvHashSet<VariableID> {
        let mut used = self.used_in_objective.clone();
        for ids in self.used_in_constraints.values() {
            used.extend(ids);
        }
        used
    }

    /// The set of decision variables that are not used in the objective or constraints and are not fixed or dependent.
    pub fn irrelevant(&self) -> FnvHashSet<VariableID> {
        let relevant = self
            .used()
            .iter()
            .chain(self.dependent.iter())
            .chain(self.fixed.keys())
            .cloned()
            .collect();
        self.all.difference(&relevant).cloned().collect()
    }

    pub fn used_binary(&self) -> FnvHashSet<VariableID> {
        let used_ids = self.used();
        self.binary()
            .intersection(&used_ids)
            .cloned()
            .collect::<FnvHashSet<VariableID>>()
    }

    pub fn used_integer(&self) -> FnvHashMap<VariableID, Bound> {
        let used_ids = self.used();
        self.integer()
            .iter()
            .filter(|(id, _)| used_ids.contains(id))
            .map(|(id, bound)| (*id, *bound))
            .collect::<FnvHashMap<VariableID, Bound>>()
    }

    pub fn used_continuous(&self) -> FnvHashMap<VariableID, Bound> {
        let used_ids = self.used();
        self.continuous()
            .iter()
            .filter(|(id, _)| used_ids.contains(id))
            .map(|(id, bound)| (*id, *bound))
            .collect::<FnvHashMap<VariableID, Bound>>()
    }

    pub fn used_semi_integer(&self) -> FnvHashMap<VariableID, Bound> {
        let used_ids = self.used();
        self.semi_integer()
            .iter()
            .filter(|(id, _)| used_ids.contains(id))
            .map(|(id, bound)| (*id, *bound))
            .collect::<FnvHashMap<VariableID, Bound>>()
    }

    pub fn used_semi_continuous(&self) -> FnvHashMap<VariableID, Bound> {
        let used_ids = self.used();
        self.semi_continuous()
            .iter()
            .filter(|(id, _)| used_ids.contains(id))
            .map(|(id, bound)| (*id, *bound))
            .collect::<FnvHashMap<VariableID, Bound>>()
    }

    /// Check the state is valid for this analysis.
    ///
    /// The state is **valid** if:
    /// - The IDs which the state contains equals to `used` exactly.
    /// - The values of the state satisfy the bounds of the decision variables.
    pub fn validate_state(&self, state: &State, atol: f64) -> Result<(), StateValidationError> {
        let state_ids: FnvHashSet<VariableID> =
            state.entries.keys().map(|id| (*id).into()).collect();
        let used_ids = self.used();

        if state_ids != used_ids {
            let extra_in_state: FnvHashSet<VariableID> =
                state_ids.difference(&used_ids).cloned().collect();
            let missing_from_state: FnvHashSet<VariableID> =
                used_ids.difference(&state_ids).cloned().collect();
            return Err(StateValidationError::MismatchedIDs {
                extra: extra_in_state,
                missing: missing_from_state,
            });
        }

        for (id, &value) in &state.entries {
            let id_ref = &VariableID::from(*id);
            if self.binary.contains(id_ref) {
                if (value - 0.0).abs() > atol && (value - 1.0).abs() > atol {
                    return Err(StateValidationError::BinaryValueNotBool { id: *id_ref, value });
                }
            } else if let Some(bound) = self.integer.get(id_ref) {
                if (value.fract()).abs() > atol {
                    return Err(StateValidationError::NotAnInteger { id: *id_ref, value });
                }
                if !bound.contains(value, atol) {
                    return Err(StateValidationError::ValueOutOfBounds {
                        id: *id_ref,
                        value,
                        bound: *bound,
                        kind: Kind::Integer,
                    });
                }
            } else if let Some(bound) = self.continuous.get(id_ref) {
                if !bound.contains(value, atol) {
                    return Err(StateValidationError::ValueOutOfBounds {
                        id: *id_ref,
                        value,
                        bound: *bound,
                        kind: Kind::Continuous,
                    });
                }
            } else if let Some(bound) = self.semi_integer.get(id_ref) {
                if value.abs() > atol {
                    // If not zero
                    if (value.fract()).abs() > atol {
                        return Err(StateValidationError::SemiIntegerNonZeroNotInteger {
                            id: *id_ref,
                            value,
                        });
                    }
                    if !bound.contains(value, atol) {
                        return Err(StateValidationError::ValueOutOfBounds {
                            id: *id_ref,
                            value,
                            bound: *bound,
                            kind: Kind::SemiInteger,
                        });
                    }
                }
            } else if let Some(bound) = self.semi_continuous.get(id_ref) {
                if value.abs() > atol {
                    // If not zero
                    if !bound.contains(value, atol) {
                        return Err(StateValidationError::ValueOutOfBounds {
                            id: *id_ref,
                            value,
                            bound: *bound,
                            kind: Kind::SemiContinuous,
                        });
                    }
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum StateValidationError {
    #[error("State IDs do not match used variable IDs. Extra in state: {extra:?}, Missing from state: {missing:?}")]
    MismatchedIDs {
        extra: FnvHashSet<VariableID>,
        missing: FnvHashSet<VariableID>,
    },
    #[error(
        "Value for {kind:?} variable {id:?} is out of bounds. Value: {value}, Bound: {bound:?}"
    )]
    ValueOutOfBounds {
        id: VariableID,
        value: f64,
        bound: Bound,
        kind: Kind,
    },
    #[error("Value for binary variable {id:?} is not 0.0 or 1.0. Value: {value}")]
    BinaryValueNotBool { id: VariableID, value: f64 },
    #[error("Value for integer variable {id:?} is not an integer. Value: {value}")]
    NotAnInteger { id: VariableID, value: f64 },
    #[error("Non-zero value for semi-integer variable {id:?} is not an integer. Value: {value}")]
    SemiIntegerNonZeroNotInteger { id: VariableID, value: f64 },
}

/// Check if **used** decision variables has the same bounds
///
/// Other decision variables e.g. `fixed` are ignored.
impl AbsDiffEq for DecisionVariableAnalysis {
    type Epsilon = f64;

    fn default_epsilon() -> Self::Epsilon {
        Bound::default_epsilon()
    }

    fn abs_diff_eq(&self, other: &Self, epsilon: Self::Epsilon) -> bool {
        if self.used_binary() != other.used_binary() {
            return false;
        }

        let self_integers = self.used_integer();
        let other_integers = other.used_integer();
        if self_integers.len() != other_integers.len() {
            return false;
        }
        for (id, bound) in &self_integers {
            if let Some(other_bound) = other_integers.get(id) {
                if !bound.abs_diff_eq(other_bound, epsilon) {
                    return false;
                }
            } else {
                return false; // Not found in other instance
            }
        }

        let self_continuous = self.used_continuous();
        let other_continuous = other.used_continuous();
        if self_continuous.len() != other_continuous.len() {
            return false;
        }
        for (id, bound) in &self_continuous {
            if let Some(other_bound) = other_continuous.get(id) {
                if !bound.abs_diff_eq(other_bound, epsilon) {
                    return false;
                }
            } else {
                return false; // Not found in other instance
            }
        }

        let self_semi_integer = self.used_semi_integer();
        let other_semi_integer = other.used_semi_integer();
        if self_semi_integer.len() != other_semi_integer.len() {
            return false;
        }
        for (id, bound) in &self_semi_integer {
            if let Some(other_bound) = other_semi_integer.get(id) {
                if !bound.abs_diff_eq(other_bound, epsilon) {
                    return false;
                }
            } else {
                return false; // Not found in other instance
            }
        }

        let self_semi_continuous = self.used_semi_continuous();
        let other_semi_continuous = other.used_semi_continuous();
        if self_semi_continuous.len() != other_semi_continuous.len() {
            return false;
        }
        for (id, bound) in &self_semi_continuous {
            if let Some(other_bound) = other_semi_continuous.get(id) {
                if !bound.abs_diff_eq(other_bound, epsilon) {
                    return false;
                }
            } else {
                return false; // Not found in other instance
            }
        }
        true
    }
}

impl Instance {
    pub fn analyze_decision_variables(&self) -> DecisionVariableAnalysis {
        let mut all = FnvHashSet::default();
        let mut fixed = FnvHashMap::default();
        let mut binary = FnvHashSet::default();
        let mut integer = FnvHashMap::default();
        let mut continuous = FnvHashMap::default();
        let mut semi_integer = FnvHashMap::default();
        let mut semi_continuous = FnvHashMap::default();
        for (id, dv) in &self.decision_variables {
            match dv.kind {
                Kind::Binary => binary.insert(*id),
                Kind::Integer => integer.insert(*id, dv.bound).is_some(),
                Kind::Continuous => continuous.insert(*id, dv.bound).is_some(),
                Kind::SemiInteger => semi_integer.insert(*id, dv.bound).is_some(),
                Kind::SemiContinuous => semi_continuous.insert(*id, dv.bound).is_some(),
            };
            all.insert(*id);
            if let Some(value) = dv.substituted_value {
                fixed.insert(*id, value);
            }
        }

        let used_in_objective: FnvHashSet<VariableID> = self
            .objective
            .required_ids()
            .into_iter()
            .map(VariableID::from)
            .collect();
        debug_assert!(
            used_in_objective.is_subset(&all),
            "Objective function uses variables not in the instance"
        );

        let mut used_in_constraints: FnvHashMap<ConstraintID, FnvHashSet<VariableID>> =
            FnvHashMap::default();
        for constraint in self.constraints.values() {
            used_in_constraints.insert(
                constraint.id,
                constraint
                    .function
                    .required_ids()
                    .into_iter()
                    .map(VariableID::from)
                    .collect(),
            );
        }
        debug_assert!(
            used_in_constraints.values().all(|ids| ids.is_subset(&all)),
            "Constraints use variables not in the instance"
        );

        let dependent: FnvHashSet<VariableID> =
            self.decision_variable_dependency.keys().cloned().collect();
        debug_assert!(
            dependent.is_subset(&all),
            "Dependent variables not in the instance"
        );

        DecisionVariableAnalysis {
            all,
            fixed,
            binary,
            integer,
            continuous,
            semi_integer,
            semi_continuous,
            used_in_objective,
            used_in_constraints,
            dependent,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        // Binary, integer, continuous, semi_integer, and semi_continuous are disjoint
        // and their union is equal to all.
        #[test]
        fn test_kind_partition(instance in Instance::arbitrary()) {
            let analysis = instance.analyze_decision_variables();
            prop_assert_eq!(
                analysis.all.len(),
                analysis.binary.len() + analysis.integer.len() + analysis.continuous.len()
                + analysis.semi_integer.len() + analysis.semi_continuous.len()
            );
            let mut all = analysis.binary().clone();
            all.extend(analysis.integer.keys());
            all.extend(analysis.continuous.keys());
            all.extend(analysis.semi_integer.keys());
            all.extend(analysis.semi_continuous.keys());
            prop_assert_eq!(&all, &analysis.all);
        }
    }
}
