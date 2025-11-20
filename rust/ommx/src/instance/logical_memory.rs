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

            // Aggregate all DecisionVariable metadata to avoid emitting one line per variable
            let mut total_name_bytes = 0;
            let mut total_description_bytes = 0;
            let mut total_subscripts_bytes = 0;
            let mut total_parameters_bytes = 0;

            for dv in self.decision_variables().values() {
                if let Some(name) = &dv.metadata.name {
                    total_name_bytes += size_of::<String>() + name.capacity();
                }
                if let Some(description) = &dv.metadata.description {
                    total_description_bytes += size_of::<String>() + description.capacity();
                }
                if !dv.metadata.subscripts.is_empty() {
                    total_subscripts_bytes +=
                        size_of::<Vec<i64>>() + dv.metadata.subscripts.capacity() * size_of::<i64>();
                }
                if !dv.metadata.parameters.is_empty() {
                    use fnv::FnvHashMap;
                    let params_overhead = size_of::<FnvHashMap<String, String>>();
                    let mut entries_bytes = 0;
                    for (k, v) in &dv.metadata.parameters {
                        entries_bytes += size_of::<(String, String)>();
                        entries_bytes += k.capacity();
                        entries_bytes += v.capacity();
                    }
                    total_parameters_bytes += params_overhead + entries_bytes;
                }
            }

            path.push("metadata");
            if total_name_bytes > 0 {
                path.push("name");
                visitor.visit_leaf(path, total_name_bytes);
                path.pop();
            }
            if total_description_bytes > 0 {
                path.push("description");
                visitor.visit_leaf(path, total_description_bytes);
                path.pop();
            }
            if total_subscripts_bytes > 0 {
                path.push("subscripts");
                visitor.visit_leaf(path, total_subscripts_bytes);
                path.pop();
            }
            if total_parameters_bytes > 0 {
                path.push("parameters");
                visitor.visit_leaf(path, total_parameters_bytes);
                path.pop();
            }
            path.pop(); // "metadata"

            path.pop(); // "decision_variables"
        }

        // BTreeMap<ConstraintID, Constraint> constraints
        if !self.constraints().is_empty() {
            path.push("constraints");

            // BTreeMap overhead
            let map_overhead = size_of::<
                std::collections::BTreeMap<crate::ConstraintID, crate::Constraint>,
            >();
            visitor.visit_leaf(path, map_overhead);

            // Aggregate all Constraint data
            for constraint in self.constraints().values() {
                // Delegate to Function
                path.push("function");
                constraint.function.visit_logical_memory(path, visitor);
                path.pop();
            }

            // Aggregate metadata fields
            let mut total_name_bytes = 0;
            let mut total_description_bytes = 0;
            let mut total_subscripts_bytes = 0;
            let mut total_parameters_bytes = 0;

            for constraint in self.constraints().values() {
                if let Some(name) = &constraint.name {
                    total_name_bytes += size_of::<String>() + name.capacity();
                }
                if let Some(description) = &constraint.description {
                    total_description_bytes += size_of::<String>() + description.capacity();
                }
                if !constraint.subscripts.is_empty() {
                    total_subscripts_bytes +=
                        size_of::<Vec<i64>>() + constraint.subscripts.capacity() * size_of::<i64>();
                }
                if !constraint.parameters.is_empty() {
                    use fnv::FnvHashMap;
                    let params_overhead = size_of::<FnvHashMap<String, String>>();
                    let mut entries_bytes = 0;
                    for (k, v) in &constraint.parameters {
                        entries_bytes += size_of::<(String, String)>();
                        entries_bytes += k.capacity();
                        entries_bytes += v.capacity();
                    }
                    total_parameters_bytes += params_overhead + entries_bytes;
                }
            }

            path.push("metadata");
            if total_name_bytes > 0 {
                path.push("name");
                visitor.visit_leaf(path, total_name_bytes);
                path.pop();
            }
            if total_description_bytes > 0 {
                path.push("description");
                visitor.visit_leaf(path, total_description_bytes);
                path.pop();
            }
            if total_subscripts_bytes > 0 {
                path.push("subscripts");
                visitor.visit_leaf(path, total_subscripts_bytes);
                path.pop();
            }
            if total_parameters_bytes > 0 {
                path.push("parameters");
                visitor.visit_leaf(path, total_parameters_bytes);
                path.pop();
            }
            path.pop(); // "metadata"

            path.pop(); // "constraints"
        }

        // BTreeMap<ConstraintID, RemovedConstraint> removed_constraints
        if !self.removed_constraints().is_empty() {
            path.push("removed_constraints");

            // BTreeMap overhead
            let map_overhead = size_of::<
                std::collections::BTreeMap<crate::ConstraintID, crate::RemovedConstraint>,
            >();
            visitor.visit_leaf(path, map_overhead);

            // Aggregate constraint functions
            for removed in self.removed_constraints().values() {
                path.push("constraint");
                path.push("function");
                removed.constraint.function.visit_logical_memory(path, visitor);
                path.pop();
                path.pop();
            }

            // Aggregate constraint metadata
            let mut total_constraint_name_bytes = 0;
            let mut total_constraint_description_bytes = 0;
            let mut total_constraint_subscripts_bytes = 0;
            let mut total_constraint_parameters_bytes = 0;

            for removed in self.removed_constraints().values() {
                if let Some(name) = &removed.constraint.name {
                    total_constraint_name_bytes += size_of::<String>() + name.capacity();
                }
                if let Some(description) = &removed.constraint.description {
                    total_constraint_description_bytes +=
                        size_of::<String>() + description.capacity();
                }
                if !removed.constraint.subscripts.is_empty() {
                    total_constraint_subscripts_bytes += size_of::<Vec<i64>>()
                        + removed.constraint.subscripts.capacity() * size_of::<i64>();
                }
                if !removed.constraint.parameters.is_empty() {
                    use fnv::FnvHashMap;
                    let params_overhead = size_of::<FnvHashMap<String, String>>();
                    let mut entries_bytes = 0;
                    for (k, v) in &removed.constraint.parameters {
                        entries_bytes += size_of::<(String, String)>();
                        entries_bytes += k.capacity();
                        entries_bytes += v.capacity();
                    }
                    total_constraint_parameters_bytes += params_overhead + entries_bytes;
                }
            }

            path.push("constraint");
            path.push("metadata");
            if total_constraint_name_bytes > 0 {
                path.push("name");
                visitor.visit_leaf(path, total_constraint_name_bytes);
                path.pop();
            }
            if total_constraint_description_bytes > 0 {
                path.push("description");
                visitor.visit_leaf(path, total_constraint_description_bytes);
                path.pop();
            }
            if total_constraint_subscripts_bytes > 0 {
                path.push("subscripts");
                visitor.visit_leaf(path, total_constraint_subscripts_bytes);
                path.pop();
            }
            if total_constraint_parameters_bytes > 0 {
                path.push("parameters");
                visitor.visit_leaf(path, total_constraint_parameters_bytes);
                path.pop();
            }
            path.pop(); // "metadata"
            path.pop(); // "constraint"

            // Aggregate removed_reason and removed_reason_parameters
            let mut total_removed_reason_bytes = 0;
            let mut total_removed_reason_parameters_bytes = 0;

            for removed in self.removed_constraints().values() {
                total_removed_reason_bytes +=
                    size_of::<String>() + removed.removed_reason.capacity();

                if !removed.removed_reason_parameters.is_empty() {
                    use fnv::FnvHashMap;
                    let params_overhead = size_of::<FnvHashMap<String, String>>();
                    let mut entries_bytes = 0;
                    for (k, v) in &removed.removed_reason_parameters {
                        entries_bytes += size_of::<(String, String)>();
                        entries_bytes += k.capacity();
                        entries_bytes += v.capacity();
                    }
                    total_removed_reason_parameters_bytes += params_overhead + entries_bytes;
                }
            }

            if total_removed_reason_bytes > 0 {
                path.push("removed_reason");
                visitor.visit_leaf(path, total_removed_reason_bytes);
                path.pop();
            }
            if total_removed_reason_parameters_bytes > 0 {
                path.push("removed_reason_parameters");
                visitor.visit_leaf(path, total_removed_reason_parameters_bytes);
                path.pop();
            }

            path.pop(); // "removed_constraints"
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
        Instance;objective;Zero 40
        Instance;decision_variables 24
        Instance;decision_variables;metadata;name 95
        "###);
    }
}
