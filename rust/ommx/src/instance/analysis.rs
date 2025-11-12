use super::*;
use crate::{v1::State, ATol, Bound, Bounds, Evaluate, Kind, VariableIDSet};
use std::collections::BTreeMap;

/// The result of analyzing the decision variables in an instance.
///
/// Responsibility
/// ---------------
/// This struct is responsible for
///
/// - Serving kind-based and usage-based partitioning of decision variables to solvers.
///   Solvers only want to know the mathematical properties of the optimization problem.
///   They do not need to know the details of the instance, such as the names of the decision
///   variables and constraints, removed constraints or fixed variables which does not affect the
///   optimization problem itself.
///
/// - Validating the state returned by solvers, and populating the variables which does not passed to the solvers.
///   - The state by solvers is **valid** if:
///     - It contains every [`Self::used`] decision variables. Other IDs are allowed as long as consistent with the population result.
///     - The values for each decision variable are within the bounds.
///   - [`Self::populate`] checks the state is valid, and populates the state as follows:
///     - For [`Self::fixed`] ID, the fixed value is used.
///     - For [`Self::irrelevant`] ID, [`Bound::nearest_to_zero`] is used as the value.
///     - For [`Self::dependent`] ID, the value is evaluated from other IDs.
///
/// Invariants
/// -----------
/// - Every IDs are subset of [`Self::all`].
/// - (kind-based partitioning) [`Self::binary`], [`Self::integer`], [`Self::continuous`], [`Self::semi_integer`], and [`Self::semi_continuous`]
///   are disjoint, and their union is equal to [`Self::all`].
/// - (usage-based partitioning) The union of [`Self::used_in_objective`] and [`Self::used_in_constraints`] (= [`Self::used`]), [`Self::fixed`],
///   and [`Self::dependent`] are disjoint each other. Remaining decision variables are [`Self::irrelevant`].
#[derive(Debug, Clone, PartialEq, getset::Getters, serde::Serialize, serde::Deserialize)]
pub struct DecisionVariableAnalysis {
    /// The IDs of all decision variables
    #[getset(get = "pub")]
    all: VariableIDSet,

    /*
     * Kind-based partition
     */
    #[getset(get = "pub")]
    binary: Bounds,
    #[getset(get = "pub")]
    integer: Bounds,
    #[getset(get = "pub")]
    continuous: Bounds,
    #[getset(get = "pub")]
    semi_integer: Bounds,
    #[getset(get = "pub")]
    semi_continuous: Bounds,

    /*
     * Usage-based partition
     */
    /// The set of decision variables that are used in the objective function.
    #[getset(get = "pub")]
    used_in_objective: VariableIDSet,
    /// The set of decision variables that are used in the constraints.
    #[getset(get = "pub")]
    used_in_constraints: BTreeMap<ConstraintID, VariableIDSet>,
    /// The set of decision variables that are used in the objective function or constraints.
    #[getset(get = "pub")]
    used: VariableIDSet,
    /// Fixed decision variables
    #[getset(get = "pub")]
    fixed: BTreeMap<VariableID, f64>,
    /// Dependent variables
    #[getset(get = "pub")]
    dependent: BTreeMap<VariableID, (Kind, Bound, Function)>,
    /// The set of decision variables that are not used in the objective or constraints and are not fixed or dependent.
    #[getset(get = "pub")]
    irrelevant: BTreeMap<VariableID, (Kind, Bound)>,
}

impl DecisionVariableAnalysis {
    pub fn used_binary(&self) -> Bounds {
        let used_ids = self.used();
        self.binary()
            .iter()
            .filter(|(id, _)| used_ids.contains(id))
            .map(|(id, bound)| (*id, *bound))
            .collect()
    }

    pub fn used_integer(&self) -> Bounds {
        let used_ids = self.used();
        self.integer()
            .iter()
            .filter(|(id, _)| used_ids.contains(id))
            .map(|(id, bound)| (*id, *bound))
            .collect()
    }

    pub fn used_continuous(&self) -> Bounds {
        let used_ids = self.used();
        self.continuous()
            .iter()
            .filter(|(id, _)| used_ids.contains(id))
            .map(|(id, bound)| (*id, *bound))
            .collect()
    }

    pub fn used_semi_integer(&self) -> Bounds {
        let used_ids = self.used();
        self.semi_integer()
            .iter()
            .filter(|(id, _)| used_ids.contains(id))
            .map(|(id, bound)| (*id, *bound))
            .collect()
    }

    pub fn used_semi_continuous(&self) -> Bounds {
        let used_ids = self.used();
        self.semi_continuous()
            .iter()
            .filter(|(id, _)| used_ids.contains(id))
            .map(|(id, bound)| (*id, *bound))
            .collect()
    }

    /// Check the state is **valid**, and populate the state with the removed decision variables
    ///
    /// Post-condition
    /// --------------
    /// - The IDs of returned [`State`] are the same as [`Self::all`].
    pub fn populate(&self, mut state: State, atol: ATol) -> Result<State, StateValidationError> {
        let state_ids: VariableIDSet = state.entries.keys().map(|id| (*id).into()).collect();

        // Check the IDs in the state are subset of all IDs
        let unknown_ids: VariableIDSet = state_ids.difference(&self.all).cloned().collect();
        if !unknown_ids.is_empty() {
            return Err(StateValidationError::UnknownIDs { unknown_ids });
        }

        // Check the state contains every used decision variables
        let missing_ids: VariableIDSet = self.used().difference(&state_ids).cloned().collect();
        if !missing_ids.is_empty() {
            return Err(StateValidationError::MissingRequiredIDs { missing_ids });
        }

        // Check bounds and integrality
        for (id, &value) in &state.entries {
            let id = &VariableID::from(*id);
            if let Some(bound) = self.binary.get(id) {
                check_integer(*id, value, atol)?;
                check_bound(*id, value, *bound, Kind::Binary, atol)?;
            } else if let Some(bound) = self.integer.get(id) {
                check_integer(*id, value, atol)?;
                check_bound(*id, value, *bound, Kind::Integer, atol)?;
            } else if let Some(bound) = self.continuous.get(id) {
                check_bound(*id, value, *bound, Kind::Continuous, atol)?;
            } else if let Some(bound) = self.semi_integer.get(id) {
                if value.abs() > atol {
                    check_integer(*id, value, atol)?;
                    check_bound(*id, value, *bound, Kind::SemiInteger, atol)?;
                }
            } else if let Some(bound) = self.semi_continuous.get(id) {
                if value.abs() > atol {
                    check_bound(*id, value, *bound, Kind::SemiContinuous, atol)?;
                }
            }
        }

        // Populate the state with fixed variables
        for (id, value) in self.fixed() {
            use std::collections::hash_map::Entry;
            match state.entries.entry(id.into_inner()) {
                Entry::Occupied(entry) => {
                    if (entry.get() - value).abs() > atol {
                        return Err(StateValidationError::StateValueInconsistent {
                            id: *id,
                            state_value: *entry.get(),
                            instance_value: *value,
                        });
                    }
                }
                Entry::Vacant(entry) => {
                    entry.insert(*value);
                }
            }
        }
        // Populate the state with irrelevant variables
        for (id, (kind, bound)) in self.irrelevant() {
            use std::collections::hash_map::Entry;
            match state.entries.entry(id.into_inner()) {
                Entry::Occupied(entry) => {
                    let value = *entry.get();
                    if matches!(kind, Kind::Binary | Kind::Integer | Kind::SemiInteger) {
                        check_integer(*id, value, atol)?;
                    }
                    check_bound(*id, value, *bound, *kind, atol)?;
                }
                Entry::Vacant(entry) => {
                    let value = match kind {
                        Kind::Binary | Kind::Integer | Kind::Continuous => bound.nearest_to_zero(),
                        Kind::SemiInteger | Kind::SemiContinuous => 0.0,
                    };
                    entry.insert(value);
                }
            }
        }
        // Populate the state with dependent variables
        for (id, (kind, bound, f)) in self.dependent() {
            let value = f.evaluate(&state, atol).map_err(|error| {
                StateValidationError::FailedToEvaluateDependentVariable { id: *id, error }
            })?;
            if matches!(kind, Kind::Binary | Kind::Integer | Kind::SemiInteger) {
                check_integer(*id, value, atol)?;
            }
            check_bound(*id, value, *bound, *kind, atol)?;
            if let Some(v) = state.entries.insert(id.into_inner(), value) {
                if (v - value).abs() > atol {
                    return Err(StateValidationError::StateValueInconsistent {
                        id: *id,
                        state_value: v,
                        instance_value: value,
                    });
                }
            }
        }

        Ok(state)
    }
}

fn check_integer(id: VariableID, value: f64, atol: ATol) -> Result<(), StateValidationError> {
    let rounded = value.round();
    if (rounded - value).abs() > atol {
        return Err(StateValidationError::NotAnInteger { id, value });
    }
    Ok(())
}

fn check_bound(
    id: VariableID,
    value: f64,
    bound: Bound,
    kind: Kind,
    atol: ATol,
) -> Result<(), StateValidationError> {
    if !bound.contains(value, atol) {
        return Err(StateValidationError::ValueOutOfBounds {
            id,
            value,
            bound,
            kind,
        });
    }
    Ok(())
}

#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
pub enum StateValidationError {
    #[error("The state contains some unknown IDs: {unknown_ids:?}")]
    UnknownIDs { unknown_ids: VariableIDSet },
    #[error("The state does not contain some required IDs: {missing_ids:?}")]
    MissingRequiredIDs { missing_ids: VariableIDSet },
    #[error(
        "Value for {kind:?} variable {id:?} is out of bounds. Value: {value}, Bound: {bound:?}"
    )]
    ValueOutOfBounds {
        id: VariableID,
        value: f64,
        bound: Bound,
        kind: Kind,
    },
    #[error("Value for integer variable {id:?} is not an integer. Value: {value}")]
    NotAnInteger { id: VariableID, value: f64 },
    #[error("State's value for variable {id:?} is inconsistent to instance. State value: {state_value}, Instance value: {instance_value}")]
    StateValueInconsistent {
        id: VariableID,
        /// Value in the state
        state_value: f64,
        /// Value determined from instance
        instance_value: f64,
    },
    #[error("Evaluation of dependent variable {id:?} failed. Error: {error:?}")]
    FailedToEvaluateDependentVariable {
        id: VariableID,
        error: anyhow::Error,
    },
}

impl Instance {
    pub fn binary_ids(&self) -> VariableIDSet {
        self.decision_variables
            .iter()
            .filter_map(|(id, dv)| {
                if dv.kind() == Kind::Binary {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect()
    }

    pub fn used_decision_variable_ids(&self) -> VariableIDSet {
        let mut used = self.objective.required_ids();
        for constraint in self.constraints.values() {
            used.extend(constraint.function.required_ids());
        }
        used
    }

    pub fn used_decision_variables(&self) -> BTreeMap<VariableID, &DecisionVariable> {
        let used_ids = self.used_decision_variable_ids();
        self.decision_variables
            .iter()
            .filter(|(id, _)| used_ids.contains(id))
            .map(|(id, dv)| (*id, dv))
            .collect()
    }

    pub fn analyze_decision_variables(&self) -> DecisionVariableAnalysis {
        let mut all = VariableIDSet::default();
        let mut fixed = BTreeMap::default();
        let mut binary = Bounds::default();
        let mut integer = Bounds::default();
        let mut continuous = Bounds::default();
        let mut semi_integer = Bounds::default();
        let mut semi_continuous = Bounds::default();
        for (id, dv) in &self.decision_variables {
            match dv.kind() {
                Kind::Binary => binary.insert(*id, dv.bound()),
                Kind::Integer => integer.insert(*id, dv.bound()),
                Kind::Continuous => continuous.insert(*id, dv.bound()),
                Kind::SemiInteger => semi_integer.insert(*id, dv.bound()),
                Kind::SemiContinuous => semi_continuous.insert(*id, dv.bound()),
            };
            all.insert(*id);
            if let Some(value) = dv.substituted_value() {
                fixed.insert(*id, value);
            }
        }

        let used_in_objective: VariableIDSet = self.objective.required_ids().into_iter().collect();
        debug_assert!(
            used_in_objective.is_subset(&all),
            "Objective function uses variables not in the instance"
        );

        let mut used_in_constraints: BTreeMap<ConstraintID, VariableIDSet> = BTreeMap::default();
        for constraint in self.constraints.values() {
            let required_ids: VariableIDSet =
                constraint.function.required_ids().into_iter().collect();
            debug_assert!(
                required_ids.is_subset(&all),
                "Constraints use variables not in the instance"
            );
            used_in_constraints.insert(constraint.id, required_ids);
        }
        let mut used = used_in_objective.clone();
        for ids in used_in_constraints.values() {
            used.extend(ids);
        }

        let dependent: BTreeMap<VariableID, _> = self
            .decision_variable_dependency
            .iter()
            .map(|(id, f)| {
                let dv = self
                    .decision_variables
                    .get(id)
                    .expect("Invariant of Instance.decision_variable_dependency is violated");
                (*id, (dv.kind(), dv.bound(), f.clone()))
            })
            .collect();

        let relevant: VariableIDSet = used
            .iter()
            .chain(dependent.keys())
            .chain(fixed.keys())
            .cloned()
            .collect();
        let irrelevant = all
            .difference(&relevant)
            .map(|id| {
                let dv = self.decision_variables.get(id).unwrap(); // subset of all
                debug_assert!(dv.substituted_value().is_none()); // fixed is subtracted
                (*id, (dv.kind(), dv.bound()))
            })
            .collect();

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
            used,
            dependent,
            irrelevant,
        }
    }
}

impl std::fmt::Display for DecisionVariableAnalysis {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "DecisionVariableAnalysis {{")?;
        writeln!(f, "  Total Variables: {}", self.all.len())?;
        writeln!(f)?;

        // Kind-based partitioning summary
        writeln!(f, "  Kind-based Partitioning:")?;
        writeln!(
            f,
            "    Binary: {}, Integer: {}, Continuous: {}, Semi-Integer: {}, Semi-Continuous: {}",
            self.binary.len(),
            self.integer.len(),
            self.continuous.len(),
            self.semi_integer.len(),
            self.semi_continuous.len()
        )?;
        writeln!(f)?;

        // Usage-based partitioning summary
        writeln!(f, "  Usage-based Partitioning:")?;
        writeln!(
            f,
            "    Used: {} (in objective: {}, in constraints: {} constraints), Fixed: {}, Dependent: {}, Irrelevant: {}",
            self.used.len(),
            self.used_in_objective.len(),
            self.used_in_constraints.len(),
            self.fixed.len(),
            self.dependent.len(),
            self.irrelevant.len()
        )?;

        // Kind-based details
        if !self.binary.is_empty() {
            writeln!(f, "\n  Binary Variables ({}):", self.binary.len())?;
            for (id, bound) in &self.binary {
                writeln!(f, "    x{}: {}", id.into_inner(), bound)?;
            }
        }

        if !self.integer.is_empty() {
            writeln!(f, "\n  Integer Variables ({}):", self.integer.len())?;
            for (id, bound) in &self.integer {
                writeln!(f, "    x{}: {}", id.into_inner(), bound)?;
            }
        }

        if !self.continuous.is_empty() {
            writeln!(f, "\n  Continuous Variables ({}):", self.continuous.len())?;
            for (id, bound) in &self.continuous {
                writeln!(f, "    x{}: {}", id.into_inner(), bound)?;
            }
        }

        if !self.semi_integer.is_empty() {
            writeln!(f, "\n  Semi-Integer Variables ({}):", self.semi_integer.len())?;
            for (id, bound) in &self.semi_integer {
                writeln!(f, "    x{}: {}", id.into_inner(), bound)?;
            }
        }

        if !self.semi_continuous.is_empty() {
            writeln!(
                f,
                "\n  Semi-Continuous Variables ({}):",
                self.semi_continuous.len()
            )?;
            for (id, bound) in &self.semi_continuous {
                writeln!(f, "    x{}: {}", id.into_inner(), bound)?;
            }
        }

        // Usage-based details
        if !self.used_in_objective.is_empty() {
            writeln!(
                f,
                "\n  Used in Objective ({}):",
                self.used_in_objective.len()
            )?;
            let vars: Vec<String> = self.used_in_objective.iter().map(|id| format!("x{}", id.into_inner())).collect();
            writeln!(f, "    {}", vars.join(", "))?;
        }

        if !self.used_in_constraints.is_empty() {
            writeln!(
                f,
                "\n  Used in Constraints ({} constraints):",
                self.used_in_constraints.len()
            )?;
            for (constraint_id, var_ids) in &self.used_in_constraints {
                write!(f, "    {}: ", constraint_id)?;
                let vars: Vec<String> = var_ids.iter().map(|id| format!("x{}", id.into_inner())).collect();
                writeln!(f, "{}", vars.join(", "))?;
            }
        }

        if !self.fixed.is_empty() {
            writeln!(f, "\n  Fixed Variables ({}):", self.fixed.len())?;
            for (id, value) in &self.fixed {
                writeln!(f, "    x{} = {}", id.into_inner(), value)?;
            }
        }

        if !self.dependent.is_empty() {
            writeln!(f, "\n  Dependent Variables ({}):", self.dependent.len())?;
            for (id, (kind, bound, function)) in &self.dependent {
                writeln!(
                    f,
                    "    x{} ({:?}, {}): {}",
                    id.into_inner(),
                    kind,
                    bound,
                    function
                )?;
            }
        }

        if !self.irrelevant.is_empty() {
            writeln!(f, "\n  Irrelevant Variables ({}):", self.irrelevant.len())?;
            for (id, (kind, bound)) in &self.irrelevant {
                let default_value = bound.nearest_to_zero();
                writeln!(
                    f,
                    "    x{} ({:?}, {}): will be set to {}",
                    id.into_inner(),
                    kind,
                    bound,
                    default_value
                )?;
            }
        }

        write!(f, "}}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        assign, coeff, linear, v1::State, Constraint, ConstraintID, Evaluate, Sense, Substitute,
        VariableID,
    };
    use maplit::hashmap;
    use proptest::prelude::*;
    use std::collections::BTreeMap;

    #[test]
    fn test_decision_variable_analysis_display() {
        // Create instance with 5 binary variables
        let mut decision_variables = BTreeMap::new();
        for i in 0..5 {
            decision_variables.insert(
                VariableID::from(i),
                crate::DecisionVariable::binary(VariableID::from(i)),
            );
        }

        // Objective: x0 + x1 + x2
        let objective = crate::Function::from(linear!(0) + linear!(1) + linear!(2));

        // Constraints:
        // 0: x1 + x2 == 1
        // 1: x3 == x0 + x1  (this will make x3 dependent after substitution)
        let mut constraints = BTreeMap::new();
        constraints.insert(
            ConstraintID::from(0),
            Constraint::equal_to_zero(
                ConstraintID::from(0),
                (linear!(1) + linear!(2) + coeff!(-1.0)).into(),
            ),
        );
        constraints.insert(
            ConstraintID::from(1),
            Constraint::equal_to_zero(
                ConstraintID::from(1),
                (linear!(3) + coeff!(-1.0) * linear!(0) + coeff!(-1.0) * linear!(1)).into(),
            ),
        );

        let mut instance =
            Instance::new(Sense::Maximize, objective, decision_variables, constraints).unwrap();

        // Apply partial_evaluate to fix x0 = 1
        let state = State {
            entries: hashmap! { 0 => 1.0 },
        };
        instance
            .partial_evaluate(&state, crate::ATol::default())
            .unwrap();

        // Apply substitute_acyclic to create dependent variables
        // x3 <- x0 + x1, but x0 is already fixed to 1, so x3 <- 1 + x1
        let substitutions = assign! {
            3 <- linear!(0) + linear!(1)
        };
        let instance = instance.substitute_acyclic(&substitutions).unwrap();

        let analysis = instance.analyze_decision_variables();
        insta::assert_snapshot!(analysis);
    }

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
            let mut all: VariableIDSet = analysis.binary.keys().cloned().collect();
            prop_assert_eq!(&all, &instance.binary_ids());

            all.extend(analysis.integer.keys());
            all.extend(analysis.continuous.keys());
            all.extend(analysis.semi_integer.keys());
            all.extend(analysis.semi_continuous.keys());
            prop_assert_eq!(&all, &analysis.all);
        }

        // Used, fixed, dependent, and irrelevant are disjoint each other, and their union is equal to all.
        #[test]
        fn test_used_partition(instance in Instance::arbitrary()) {
            let analysis = instance.analyze_decision_variables();
            let used = analysis.used();
            let all_len = analysis.all.len();
            let used_len = used.len();
            let fixed_len = analysis.fixed.len();
            let dependent_len = analysis.dependent.len();
            let irrelevant_len = analysis.irrelevant().len();
            prop_assert_eq!(used, &instance.used_decision_variable_ids());
            prop_assert_eq!(
                all_len,
                used_len + fixed_len + dependent_len + irrelevant_len,
                "all: {}, used: {}, fixed: {}, dependent: {}, irrelevant: {}",
                all_len, used_len, fixed_len, dependent_len, irrelevant_len
            );
            let mut all = used.clone();
            all.extend(analysis.fixed.keys());
            all.extend(analysis.dependent.keys());
            all.extend(analysis.irrelevant.keys());
            prop_assert_eq!(&all, &analysis.all);
        }

        /// Test post-condition
        #[test]
        fn test_populate(
            (instance, state) in Instance::arbitrary()
                .prop_flat_map(move |instance| instance.arbitrary_state().prop_map(move |state| (instance.clone(), state)))
        ) {
            let analysis = instance.analyze_decision_variables();
            let populated = analysis.populate(state.clone(), ATol::default()).unwrap();
            let populated_ids: VariableIDSet = populated.entries.keys().map(|id| (*id).into()).collect();
            prop_assert_eq!(populated_ids, analysis.all);
        }
    }
}
