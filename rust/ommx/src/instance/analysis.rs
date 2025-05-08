use super::*;
use crate::{Evaluate, Kind};
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
#[derive(Debug, Clone, PartialEq, Eq, getset::Getters)]
pub struct DecisionVariableAnalysis {
    /// The IDs of all decision variables
    #[getset(get = "pub")]
    all: FnvHashSet<VariableID>,

    #[getset(get = "pub")]
    binary: FnvHashSet<VariableID>,
    #[getset(get = "pub")]
    integer: FnvHashSet<VariableID>,
    #[getset(get = "pub")]
    continuous: FnvHashSet<VariableID>,
    #[getset(get = "pub")]
    semi_integer: FnvHashSet<VariableID>,
    #[getset(get = "pub")]
    semi_continuous: FnvHashSet<VariableID>,

    /// The set of decision variables that are used in the objective function.
    #[getset(get = "pub")]
    used_in_objective: FnvHashSet<VariableID>,
    /// The set of decision variables that are used in the constraints.
    #[getset(get = "pub")]
    used_in_constraints: FnvHashSet<VariableID>,

    /// Fixed decision variables
    #[getset(get = "pub")]
    fixed: FnvHashSet<VariableID>,
    /// Dependent variables
    #[getset(get = "pub")]
    dependent: FnvHashSet<VariableID>,
}

impl DecisionVariableAnalysis {
    /// Union of `used_in_objective` and `used_in_constraints`
    pub fn used(&self) -> FnvHashSet<VariableID> {
        self.used_in_constraints
            .union(&self.used_in_objective)
            .cloned()
            .collect()
    }

    /// The set of decision variables that are not used in the objective or constraints and are not fixed or dependent.
    pub fn irrelevant(&self) -> FnvHashSet<VariableID> {
        let relevant = self
            .used_in_objective
            .iter()
            .chain(self.used_in_constraints.iter())
            .chain(self.dependent.iter())
            .chain(self.fixed.iter())
            .cloned()
            .collect();
        self.all.difference(&relevant).cloned().collect()
    }
}

impl Instance {
    pub fn analyze_decision_variables(&self) -> DecisionVariableAnalysis {
        let mut all = FnvHashSet::default();
        let mut fixed = FnvHashSet::default();
        let mut binary = FnvHashSet::default();
        let mut integer = FnvHashSet::default();
        let mut continuous = FnvHashSet::default();
        let mut semi_integer = FnvHashSet::default();
        let mut semi_continuous = FnvHashSet::default();
        for (id, dv) in &self.decision_variables {
            match dv.kind {
                Kind::Binary => binary.insert(*id),
                Kind::Integer => integer.insert(*id),
                Kind::Continuous => continuous.insert(*id),
                Kind::SemiInteger => semi_integer.insert(*id),
                Kind::SemiContinuous => semi_continuous.insert(*id),
            };
            all.insert(*id);
            if dv.substituted_value.is_some() {
                fixed.insert(*id);
            }
        }
        let used_in_objective = self
            .objective
            .required_ids()
            .into_iter()
            .map(VariableID::from)
            .collect();
        let mut used_in_constraints = FnvHashSet::default();
        for constraint in self.constraints.values() {
            used_in_constraints.extend(
                constraint
                    .function
                    .required_ids()
                    .into_iter()
                    .map(VariableID::from),
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
