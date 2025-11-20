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

            // Delegate to each DecisionVariable
            for dv in self.decision_variables().values() {
                dv.visit_logical_memory(path, visitor);
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

            // Delegate to each Constraint
            for constraint in self.constraints().values() {
                constraint.visit_logical_memory(path, visitor);
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

            // Delegate to each RemovedConstraint
            for removed in self.removed_constraints().values() {
                removed.visit_logical_memory(path, visitor);
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
        Instance;objective;Linear;terms 104
        Instance;decision_variables 24
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
        Instance;objective;Linear;terms 104
        Instance;decision_variables 24
        Instance;constraints 24
        Instance;constraints;function;Linear;terms 104
        "###);
    }
}
