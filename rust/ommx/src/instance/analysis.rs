use super::*;
use crate::{Evaluate, Kind};
use fnv::FnvHashSet;

#[derive(Debug, Clone, PartialEq, Eq, getset::Getters)]
pub struct DecisionVariableAnalysis {
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

    /// Dependent variables
    #[getset(get = "pub")]
    dependent: FnvHashSet<VariableID>,
    /// The decision variables that used for evaluating the dependent variables.
    #[getset(get = "pub")]
    dependency_source: FnvHashSet<VariableID>,
}

impl Instance {
    pub fn analyze_decision_variables(&self) -> DecisionVariableAnalysis {
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
        let dependent = FnvHashSet::default();
        let dependency_source = FnvHashSet::default();

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
