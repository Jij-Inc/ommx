use super::*;
use crate::{Evaluate, Kind};
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, Eq, getset::Getters)]
pub struct DecisionVariableAnalysis {
    #[getset(get = "pub")]
    binary: HashSet<VariableID>,
    #[getset(get = "pub")]
    integer: HashSet<VariableID>,
    #[getset(get = "pub")]
    continuous: HashSet<VariableID>,
    #[getset(get = "pub")]
    semi_integer: HashSet<VariableID>,
    #[getset(get = "pub")]
    semi_continuous: HashSet<VariableID>,

    /// The set of decision variables that are used in the objective function.
    #[getset(get = "pub")]
    used_in_objective: HashSet<VariableID>,
    /// The set of decision variables that are used in the constraints.
    #[getset(get = "pub")]
    used_in_constraints: HashSet<VariableID>,

    /// Dependent variables
    #[getset(get = "pub")]
    dependent: HashSet<VariableID>,
    /// The decision variables that used for evaluating the dependent variables.
    #[getset(get = "pub")]
    dependency_source: HashSet<VariableID>,
}

impl Instance {
    pub fn analyze_decision_variables(&self) -> DecisionVariableAnalysis {
        let mut binary = HashSet::new();
        let mut integer = HashSet::new();
        let mut continuous = HashSet::new();
        let mut semi_integer = HashSet::new();
        let mut semi_continuous = HashSet::new();
        for (id, dv) in &self.decision_variables {
            match dv.kind {
                Kind::Binary => binary.insert(*id),
                Kind::Integer => integer.insert(*id),
                Kind::Continuous => continuous.insert(*id),
                Kind::SemiInteger => semi_integer.insert(*id),
                Kind::SemiContinuous => semi_continuous.insert(*id),
            };
        }
        let used_in_objective = self.objective.required_ids();
        let mut used_in_constraints = HashSet::new();
        for constraint in self.constraints.values() {
            used_in_constraints.extend(constraint.function.required_ids());
        }

        DecisionVariableAnalysis {
            binary,
            integer,
            continuous,
            semi_integer,
            semi_continuous,
            used_in_objective,
            used_in_constraints,
            dependent,
            dependency_source,
        }
    }
}
