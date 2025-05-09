use super::*;
use crate::{Bound, Evaluate, Kind};
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
        let used_in_objective = self
            .objective
            .required_ids()
            .into_iter()
            .map(VariableID::from)
            .collect();
        let mut used_in_constraints = FnvHashMap::default();
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
        let dependent = self.decision_variable_dependency.keys().cloned().collect();
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
