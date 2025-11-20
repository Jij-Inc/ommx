use crate::instance::Instance;
use crate::logical_memory::{LogicalMemoryProfile, LogicalMemoryVisitor};
use std::mem::size_of;

impl LogicalMemoryProfile for Instance {
    fn visit_logical_memory<V: LogicalMemoryVisitor>(
        &self,
        path: &mut Vec<&'static str>,
        visitor: &mut V,
    ) {
        // Delegate to objective Function
        path.push("objective");
        self.objective().visit_logical_memory(path, visitor);
        path.pop();

        // BTreeMap<VariableID, DecisionVariable> decision_variables
        if !self.decision_variables().is_empty() {
            path.push("decision_variables");

            // BTreeMap overhead
            let map_overhead =
                size_of::<std::collections::BTreeMap<crate::VariableID, crate::DecisionVariable>>(
                );
            visitor.visit_leaf(path, map_overhead);

            // Keys (VariableID)
            path.push("keys");
            let key_size = size_of::<crate::VariableID>();
            let keys_bytes = self.decision_variables().len() * key_size;
            visitor.visit_leaf(path, keys_bytes);
            path.pop();

            // Delegate to each DecisionVariable (struct + heap allocations)
            // Note: FoldedCollector will automatically aggregate same paths
            for dv in self.decision_variables().values() {
                path.push("DecisionVariable");
                dv.visit_logical_memory(path, visitor);
                path.pop();
            }

            path.pop();
        }

        // BTreeMap<ConstraintID, Constraint> constraints
        if !self.constraints().is_empty() {
            path.push("constraints");

            // BTreeMap overhead
            let map_overhead = size_of::<
                std::collections::BTreeMap<crate::ConstraintID, crate::Constraint>,
            >();
            visitor.visit_leaf(path, map_overhead);

            // Keys (ConstraintID)
            path.push("keys");
            let key_size = size_of::<crate::ConstraintID>();
            let keys_bytes = self.constraints().len() * key_size;
            visitor.visit_leaf(path, keys_bytes);
            path.pop();

            // Delegate to each Constraint (struct + function + metadata heap allocations)
            // Note: FoldedCollector will automatically aggregate same paths
            for constraint in self.constraints().values() {
                path.push("Constraint");
                constraint.visit_logical_memory(path, visitor);
                path.pop();
            }

            path.pop();
        }

        // BTreeMap<ConstraintID, RemovedConstraint> removed_constraints
        if !self.removed_constraints().is_empty() {
            path.push("removed_constraints");

            // BTreeMap overhead
            let map_overhead = size_of::<
                std::collections::BTreeMap<crate::ConstraintID, crate::RemovedConstraint>,
            >();
            visitor.visit_leaf(path, map_overhead);

            // Keys (ConstraintID)
            path.push("keys");
            let key_size = size_of::<crate::ConstraintID>();
            let keys_bytes = self.removed_constraints().len() * key_size;
            visitor.visit_leaf(path, keys_bytes);
            path.pop();

            // Delegate to each RemovedConstraint (struct + heap allocations)
            // Note: FoldedCollector will automatically aggregate same paths
            for removed in self.removed_constraints().values() {
                path.push("RemovedConstraint");
                removed.visit_logical_memory(path, visitor);
                path.pop();
            }

            path.pop();
        }

        // AcyclicAssignments decision_variable_dependency
        if !self.decision_variable_dependency().is_empty() {
            path.push("decision_variable_dependency");
            self.decision_variable_dependency()
                .visit_logical_memory(path, visitor);
            path.pop();
        }

        // ConstraintHints constraint_hints
        if !self.constraint_hints().is_empty() {
            path.push("constraint_hints");
            self.constraint_hints()
                .visit_logical_memory(path, visitor);
            path.pop();
        }

        // Option<v1::Parameters> parameters
        // Option<v1::instance::Description> description
        // These are protobuf types - we could add estimates but skip for now
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logical_memory::logical_memory_to_folded;
    use crate::{coeff, linear, Constraint, ConstraintID, DecisionVariable, Equality, Function};
    use std::collections::BTreeMap;

    #[test]
    fn test_instance_empty_snapshot() {
        let instance = Instance::default();
        let folded = logical_memory_to_folded("Instance", &instance);
        // Empty instance has zero objective
        insta::assert_snapshot!(folded, @"Instance;objective;Zero 40");
    }

    #[test]
    fn test_instance_with_objective_and_variables_snapshot() {
        let dv1 = DecisionVariable::continuous(1.into());
        let dv2 = DecisionVariable::continuous(2.into());

        let mut decision_variables = BTreeMap::new();
        decision_variables.insert(dv1.id(), dv1);
        decision_variables.insert(dv2.id(), dv2);

        let objective = Function::Linear(coeff!(2.0) * linear!(1) + coeff!(3.0) * linear!(2));

        let instance = Instance::new(
            crate::instance::Sense::Minimize,
            objective,
            decision_variables,
            BTreeMap::new(),
        )
        .unwrap();

        let folded = logical_memory_to_folded("Instance", &instance);
        insta::assert_snapshot!(folded, @r###"
        Instance;decision_variables 24
        Instance;decision_variables;DecisionVariable 304
        Instance;decision_variables;keys 16
        Instance;objective;Linear;terms 104
        "###);
    }

    #[test]
    fn test_instance_with_constraints_snapshot() {
        let dv1 = DecisionVariable::continuous(1.into());
        let dv2 = DecisionVariable::continuous(2.into());

        let mut decision_variables = BTreeMap::new();
        decision_variables.insert(dv1.id(), dv1);
        decision_variables.insert(dv2.id(), dv2);

        let objective = Function::Linear(coeff!(2.0) * linear!(1) + coeff!(3.0) * linear!(2));

        let constraint = Constraint {
            id: ConstraintID::from(1),
            function: Function::Linear(linear!(1) + linear!(2)),
            equality: Equality::LessThanOrEqualToZero,
            name: None,
            subscripts: vec![],
            parameters: Default::default(),
            description: None,
        };

        let mut constraints = BTreeMap::new();
        constraints.insert(constraint.id, constraint);

        let instance = Instance::new(
            crate::instance::Sense::Minimize,
            objective,
            decision_variables,
            constraints,
        )
        .unwrap();

        let folded = logical_memory_to_folded("Instance", &instance);
        insta::assert_snapshot!(folded, @r###"
        Instance;constraints 24
        Instance;constraints;Constraint 160
        Instance;constraints;Constraint;function;Linear;terms 104
        Instance;constraints;keys 8
        Instance;decision_variables 24
        Instance;decision_variables;DecisionVariable 304
        Instance;decision_variables;keys 16
        Instance;objective;Linear;terms 104
        "###);
    }

    #[test]
    fn test_instance_with_multiple_variables_with_metadata_snapshot() {
        // Create 3 decision variables with names to demonstrate aggregation
        let mut dv1 = DecisionVariable::continuous(1.into());
        dv1.metadata.name = Some("x1".to_string());

        let mut dv2 = DecisionVariable::continuous(2.into());
        dv2.metadata.name = Some("x2".to_string());

        let mut dv3 = DecisionVariable::continuous(3.into());
        dv3.metadata.name = Some("x3_with_longer_name".to_string());

        let mut decision_variables = BTreeMap::new();
        decision_variables.insert(dv1.id(), dv1);
        decision_variables.insert(dv2.id(), dv2);
        decision_variables.insert(dv3.id(), dv3);

        let instance = Instance::new(
            crate::instance::Sense::Minimize,
            Function::Zero,
            decision_variables,
            BTreeMap::new(),
        )
        .unwrap();

        let folded = logical_memory_to_folded("Instance", &instance);
        // Note: Same path appears multiple times, flamegraph tools will aggregate them
        insta::assert_snapshot!(folded, @r###"
        Instance;decision_variables 24
        Instance;decision_variables;DecisionVariable 456
        Instance;decision_variables;DecisionVariable;metadata;name 95
        Instance;decision_variables;keys 24
        Instance;objective;Zero 40
        "###);
    }
}
