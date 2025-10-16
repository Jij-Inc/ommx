use serde::{Deserialize, Serialize};

/// Statistics about decision variables categorized by kind.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VariableStatsByKind {
    /// Number of binary variables
    pub binary: usize,
    /// Number of integer variables
    pub integer: usize,
    /// Number of continuous variables
    pub continuous: usize,
    /// Number of semi-integer variables
    pub semi_integer: usize,
    /// Number of semi-continuous variables
    pub semi_continuous: usize,
}

/// Statistics about decision variables categorized by usage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VariableStatsByUsage {
    /// Number of variables used in the objective function
    pub used_in_objective: usize,
    /// Number of variables used in constraints
    pub used_in_constraints: usize,
    /// Number of variables used in either objective or constraints
    pub used: usize,
    /// Number of fixed variables
    pub fixed: usize,
    /// Number of dependent variables
    pub dependent: usize,
    /// Number of irrelevant variables (not used, fixed, or dependent)
    pub irrelevant: usize,
}

/// Statistics about decision variables in an instance.
///
/// This struct provides counts of decision variables categorized by:
/// - Kind: binary, integer, continuous, semi-integer, semi-continuous
/// - Usage: used (in objective or constraints), fixed, dependent, irrelevant
///
/// Note on usage categories:
/// The usage-based categories (used, fixed, dependent, irrelevant) are mutually exclusive.
/// A variable belongs to exactly one category, determined by this priority:
/// 1. `fixed`: Variables with substituted values
/// 2. `dependent`: Variables defined by assignments in decision_variable_dependency
/// 3. `used`: Variables appearing in objective or active constraints (not in categories 1-2)
/// 4. `irrelevant`: All other variables (not in categories 1-3)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionVariableStats {
    /// Total number of decision variables
    pub total: usize,
    /// Statistics categorized by variable kind
    pub by_kind: VariableStatsByKind,
    /// Statistics categorized by variable usage
    pub by_usage: VariableStatsByUsage,
}

/// Statistics about constraints in an instance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConstraintStats {
    /// Total number of constraints (active + removed)
    pub total: usize,
    /// Number of active constraints
    pub active: usize,
    /// Number of removed constraints
    pub removed: usize,
}

/// Statistics about an optimization problem instance.
///
/// This struct provides a summary of the instance structure,
/// including counts of variables and constraints by category.
/// It is designed to be serializable for snapshot testing and reporting.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstanceStats {
    /// Statistics about decision variables
    pub decision_variables: DecisionVariableStats,
    /// Statistics about constraints
    pub constraints: ConstraintStats,
}

impl super::Instance {
    /// Compute statistics about this instance.
    ///
    /// # Returns
    ///
    /// An `InstanceStats` struct containing counts of variables and constraints
    /// categorized by kind, usage, and status.
    ///
    /// # Example
    ///
    /// ```
    /// use ommx::Instance;
    ///
    /// let instance = Instance::default();
    /// let stats = instance.stats();
    /// println!("Total variables: {}", stats.decision_variables.total);
    /// println!("Active constraints: {}", stats.constraints.active);
    /// ```
    pub fn stats(&self) -> InstanceStats {
        let analysis = self.analyze_decision_variables();

        let by_kind = VariableStatsByKind {
            binary: analysis.binary().len(),
            integer: analysis.integer().len(),
            continuous: analysis.continuous().len(),
            semi_integer: analysis.semi_integer().len(),
            semi_continuous: analysis.semi_continuous().len(),
        };

        let by_usage = VariableStatsByUsage {
            used_in_objective: analysis.used_in_objective().len(),
            used_in_constraints: analysis
                .used_in_constraints()
                .values()
                .flat_map(|vars| vars.iter())
                .collect::<std::collections::HashSet<_>>()
                .len(),
            used: analysis.used().len(),
            fixed: analysis.fixed().len(),
            dependent: analysis.dependent().len(),
            irrelevant: analysis.irrelevant().len(),
        };

        let decision_variables = DecisionVariableStats {
            total: self.decision_variables.len(),
            by_kind,
            by_usage,
        };

        let constraints = ConstraintStats {
            total: self.constraints.len() + self.removed_constraints.len(),
            active: self.constraints.len(),
            removed: self.removed_constraints.len(),
        };

        InstanceStats {
            decision_variables,
            constraints,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        coeff, linear, Constraint, ConstraintID, DecisionVariable, Instance, Sense, VariableID,
    };
    use maplit::btreemap;
    use std::collections::BTreeMap;

    #[test]
    fn test_empty_instance_stats() {
        let instance = Instance::default();
        let stats = instance.stats();

        assert_eq!(stats.decision_variables.total, 0);
        assert_eq!(stats.decision_variables.by_kind.binary, 0);
        assert_eq!(stats.decision_variables.by_kind.integer, 0);
        assert_eq!(stats.decision_variables.by_kind.continuous, 0);
        assert_eq!(stats.decision_variables.by_usage.used, 0);
        assert_eq!(stats.decision_variables.by_usage.fixed, 0);
        assert_eq!(stats.decision_variables.by_usage.dependent, 0);
        assert_eq!(stats.decision_variables.by_usage.irrelevant, 0);

        assert_eq!(stats.constraints.total, 0);
        assert_eq!(stats.constraints.active, 0);
        assert_eq!(stats.constraints.removed, 0);
    }

    #[test]
    fn test_instance_with_variables_stats() {
        let decision_variables = btreemap! {
            VariableID::from(1) => DecisionVariable::binary(VariableID::from(1)),
            VariableID::from(2) => DecisionVariable::binary(VariableID::from(2)),
            VariableID::from(3) => DecisionVariable::integer(VariableID::from(3)),
            VariableID::from(4) => DecisionVariable::continuous(VariableID::from(4)),
        };

        // Set objective using variable 1 and 2
        let objective = (linear!(1) + linear!(2)).into();

        let instance = Instance::new(
            Sense::Minimize,
            objective,
            decision_variables,
            BTreeMap::new(),
        )
        .unwrap();

        let stats = instance.stats();

        assert_eq!(stats.decision_variables.total, 4);
        assert_eq!(stats.decision_variables.by_kind.binary, 2);
        assert_eq!(stats.decision_variables.by_kind.integer, 1);
        assert_eq!(stats.decision_variables.by_kind.continuous, 1);
        assert_eq!(stats.decision_variables.by_usage.used_in_objective, 2);
        assert_eq!(stats.decision_variables.by_usage.used, 2);
        assert_eq!(stats.decision_variables.by_usage.irrelevant, 2); // variables 3 and 4
    }

    #[test]
    fn test_instance_with_constraints_stats() {
        let decision_variables = btreemap! {
            VariableID::from(1) => DecisionVariable::binary(VariableID::from(1)),
            VariableID::from(2) => DecisionVariable::binary(VariableID::from(2)),
            VariableID::from(3) => DecisionVariable::binary(VariableID::from(3)),
        };

        let objective = (linear!(1) + coeff!(1.0)).into();

        let constraints = btreemap! {
            ConstraintID::from(1) => Constraint::equal_to_zero(
                ConstraintID::from(1),
                (linear!(1) + linear!(2) + coeff!(-1.0)).into(),
            ),
            ConstraintID::from(2) => Constraint::equal_to_zero(
                ConstraintID::from(2),
                (linear!(3) + coeff!(-1.0)).into(),
            ),
        };

        let mut instance =
            Instance::new(Sense::Minimize, objective, decision_variables, constraints).unwrap();

        // Remove one constraint
        instance
            .relax_constraint(ConstraintID::from(2), "Test removal".to_string(), [])
            .unwrap();

        let stats = instance.stats();

        assert_eq!(stats.constraints.total, 2);
        assert_eq!(stats.constraints.active, 1);
        assert_eq!(stats.constraints.removed, 1);
        // Variables 1 and 2 are used in constraint 1 (active)
        // Note: Removed constraints are NOT counted in used_in_constraints
        assert_eq!(stats.decision_variables.by_usage.used_in_constraints, 2);
        // Variable 1 is used in both objective and constraint 1, variable 2 is used in constraint 1
        assert_eq!(stats.decision_variables.by_usage.used, 2);
    }

    #[test]
    fn test_stats_serialization() {
        let decision_variables = btreemap! {
            VariableID::from(1) => DecisionVariable::binary(VariableID::from(1)),
        };

        let objective = (linear!(1) + coeff!(1.0)).into();

        let instance = Instance::new(
            Sense::Minimize,
            objective,
            decision_variables,
            BTreeMap::new(),
        )
        .unwrap();

        let stats = instance.stats();

        // Test that stats can be serialized and deserialized
        let json = serde_json::to_string(&stats).unwrap();
        let deserialized: InstanceStats = serde_json::from_str(&json).unwrap();

        assert_eq!(stats, deserialized);
    }

    #[test]
    fn test_stats_snapshot_empty() {
        let instance = Instance::default();
        let stats = instance.stats();
        insta::assert_yaml_snapshot!(stats);
    }

    #[test]
    fn test_stats_snapshot_with_variables() {
        let decision_variables = btreemap! {
            VariableID::from(1) => DecisionVariable::binary(VariableID::from(1)),
            VariableID::from(2) => DecisionVariable::binary(VariableID::from(2)),
            VariableID::from(3) => DecisionVariable::integer(VariableID::from(3)),
            VariableID::from(4) => DecisionVariable::continuous(VariableID::from(4)),
            VariableID::from(5) => DecisionVariable::semi_integer(VariableID::from(5)),
        };

        let objective = (linear!(1) + linear!(2)).into();

        let instance = Instance::new(
            Sense::Minimize,
            objective,
            decision_variables,
            BTreeMap::new(),
        )
        .unwrap();

        let stats = instance.stats();
        insta::assert_yaml_snapshot!(stats);
    }

    #[test]
    fn test_stats_snapshot_with_constraints() {
        let decision_variables = btreemap! {
            VariableID::from(1) => DecisionVariable::binary(VariableID::from(1)),
            VariableID::from(2) => DecisionVariable::binary(VariableID::from(2)),
            VariableID::from(3) => DecisionVariable::integer(VariableID::from(3)),
        };

        let objective = (linear!(1) + coeff!(1.0)).into();

        let constraints = btreemap! {
            ConstraintID::from(1) => Constraint::equal_to_zero(
                ConstraintID::from(1),
                (linear!(1) + linear!(2) + coeff!(-1.0)).into(),
            ),
            ConstraintID::from(2) => Constraint::equal_to_zero(
                ConstraintID::from(2),
                (linear!(2) + linear!(3) + coeff!(-5.0)).into(),
            ),
            ConstraintID::from(3) => Constraint::equal_to_zero(
                ConstraintID::from(3),
                (linear!(3) + coeff!(-10.0)).into(),
            ),
        };

        let mut instance =
            Instance::new(Sense::Minimize, objective, decision_variables, constraints).unwrap();

        // Remove one constraint
        instance
            .relax_constraint(ConstraintID::from(3), "Not needed".to_string(), [])
            .unwrap();

        let stats = instance.stats();
        insta::assert_yaml_snapshot!(stats);
    }
}
