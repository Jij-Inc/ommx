use crate::instance::Instance;
use crate::logical_memory::{LogicalMemoryProfile, LogicalMemoryVisitor, Path};
use crate::v1;

// Implementations for protobuf types

impl LogicalMemoryProfile for v1::Parameters {
    fn visit_logical_memory<V: LogicalMemoryVisitor>(&self, path: &mut Path, visitor: &mut V) {
        // HashMap stack overhead
        let map_overhead = size_of::<std::collections::HashMap<u64, f64>>();
        visitor.visit_leaf(path, map_overhead);

        // Keys (u64)
        let key_size = size_of::<u64>();
        let keys_bytes = self.entries.len() * key_size;
        visitor.visit_leaf(&path.with("keys"), keys_bytes);

        // Values (f64)
        let value_size = size_of::<f64>();
        let values_bytes = self.entries.len() * value_size;
        visitor.visit_leaf(&path.with("values"), values_bytes);
    }
}

impl LogicalMemoryProfile for v1::instance::Description {
    fn visit_logical_memory<V: LogicalMemoryVisitor>(&self, path: &mut Path, visitor: &mut V) {
        // name: Option<String>
        if let Some(name) = &self.name {
            let bytes = size_of::<String>() + name.len();
            visitor.visit_leaf(&path.with("name"), bytes);
        }

        // description: Option<String>
        if let Some(description) = &self.description {
            let bytes = size_of::<String>() + description.len();
            visitor.visit_leaf(&path.with("description"), bytes);
        }

        // authors: Vec<String>
        {
            let mut guard = path.with("authors");
            let vec_overhead = size_of::<Vec<String>>();
            visitor.visit_leaf(&guard, vec_overhead);

            for author in &self.authors {
                let bytes = size_of::<String>() + author.len();
                visitor.visit_leaf(&guard.with("String"), bytes);
            }
        }

        // created_by: Option<String>
        if let Some(created_by) = &self.created_by {
            let bytes = size_of::<String>() + created_by.len();
            visitor.visit_leaf(&path.with("created_by"), bytes);
        }
    }
}

impl LogicalMemoryProfile for Instance {
    fn visit_logical_memory<V: LogicalMemoryVisitor>(&self, path: &mut Path, visitor: &mut V) {
        // Count each field individually to avoid double-counting
        // Use "Type.field" format for flamegraph clarity

        // sense: Sense (enum)
        visitor.visit_leaf(&path.with("Instance.sense"), size_of::<crate::instance::Sense>());

        // Delegate to objective Function
        self.objective()
            .visit_logical_memory(path.with("Instance.objective").as_mut(), visitor);

        // decision_variables: BTreeMap<VariableID, DecisionVariable>
        self.decision_variables()
            .visit_logical_memory(path.with("Instance.decision_variables").as_mut(), visitor);

        // constraints: BTreeMap<ConstraintID, Constraint>
        self.constraints()
            .visit_logical_memory(path.with("Instance.constraints").as_mut(), visitor);

        // removed_constraints: BTreeMap<ConstraintID, RemovedConstraint>
        self.removed_constraints()
            .visit_logical_memory(path.with("Instance.removed_constraints").as_mut(), visitor);

        // decision_variable_dependency: AcyclicAssignments
        self.decision_variable_dependency()
            .visit_logical_memory(path.with("Instance.decision_variable_dependency").as_mut(), visitor);

        // constraint_hints: ConstraintHints
        self.constraint_hints()
            .visit_logical_memory(path.with("Instance.constraint_hints").as_mut(), visitor);

        // parameters: Option<v1::Parameters>
        if let Some(parameters) = &self.parameters {
            parameters.visit_logical_memory(path.with("Instance.parameters").as_mut(), visitor);
        }

        // description: Option<v1::instance::Description>
        if let Some(description) = &self.description {
            description.visit_logical_memory(path.with("Instance.description").as_mut(), visitor);
        }
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
        let folded = logical_memory_to_folded(&instance);
        // Empty instance has zero objective
        insta::assert_snapshot!(folded, @r###"
        Instance.constraint_hints;ConstraintHints.one_hot_constraints;Vec[stack] 24
        Instance.constraint_hints;ConstraintHints.sos1_constraints;Vec[stack] 24
        Instance.constraints;BTreeMap[stack] 24
        Instance.decision_variable_dependency;AcyclicAssignments.assignments;FnvHashMap[stack] 32
        Instance.decision_variable_dependency;AcyclicAssignments.dependency 144
        Instance.decision_variables;BTreeMap[stack] 24
        Instance.objective;Zero 40
        Instance.removed_constraints;BTreeMap[stack] 24
        Instance.sense 1
        "###);
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

        let folded = logical_memory_to_folded(&instance);
        insta::assert_snapshot!(folded, @r###"
        Instance.constraint_hints;ConstraintHints.one_hot_constraints;Vec[stack] 24
        Instance.constraint_hints;ConstraintHints.sos1_constraints;Vec[stack] 24
        Instance.constraints;BTreeMap[stack] 24
        Instance.decision_variable_dependency;AcyclicAssignments.assignments;FnvHashMap[stack] 32
        Instance.decision_variable_dependency;AcyclicAssignments.dependency 144
        Instance.decision_variables;BTreeMap[key] 16
        Instance.decision_variables;BTreeMap[stack] 24
        Instance.decision_variables;DecisionVariable.bound 32
        Instance.decision_variables;DecisionVariable.id 16
        Instance.decision_variables;DecisionVariable.kind 2
        Instance.decision_variables;DecisionVariable.metadata;DecisionVariableMetadata.description;Option[stack] 48
        Instance.decision_variables;DecisionVariable.metadata;DecisionVariableMetadata.name;Option[stack] 48
        Instance.decision_variables;DecisionVariable.metadata;DecisionVariableMetadata.parameters;FnvHashMap[stack] 64
        Instance.decision_variables;DecisionVariable.metadata;DecisionVariableMetadata.subscripts;Vec[stack] 48
        Instance.decision_variables;DecisionVariable.substituted_value;Option[stack] 32
        Instance.objective;Linear;PolynomialBase.terms 80
        Instance.removed_constraints;BTreeMap[stack] 24
        Instance.sense 1
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

        let folded = logical_memory_to_folded(&instance);
        insta::assert_snapshot!(folded, @r###"
        Instance.constraint_hints;ConstraintHints.one_hot_constraints;Vec[stack] 24
        Instance.constraint_hints;ConstraintHints.sos1_constraints;Vec[stack] 24
        Instance.constraints;BTreeMap[key] 8
        Instance.constraints;BTreeMap[stack] 24
        Instance.constraints;Constraint.description;Option[stack] 24
        Instance.constraints;Constraint.equality 1
        Instance.constraints;Constraint.function;Linear;PolynomialBase.terms 80
        Instance.constraints;Constraint.id 8
        Instance.constraints;Constraint.name;Option[stack] 24
        Instance.constraints;Constraint.parameters;FnvHashMap[stack] 32
        Instance.constraints;Constraint.subscripts;Vec[stack] 24
        Instance.decision_variable_dependency;AcyclicAssignments.assignments;FnvHashMap[stack] 32
        Instance.decision_variable_dependency;AcyclicAssignments.dependency 144
        Instance.decision_variables;BTreeMap[key] 16
        Instance.decision_variables;BTreeMap[stack] 24
        Instance.decision_variables;DecisionVariable.bound 32
        Instance.decision_variables;DecisionVariable.id 16
        Instance.decision_variables;DecisionVariable.kind 2
        Instance.decision_variables;DecisionVariable.metadata;DecisionVariableMetadata.description;Option[stack] 48
        Instance.decision_variables;DecisionVariable.metadata;DecisionVariableMetadata.name;Option[stack] 48
        Instance.decision_variables;DecisionVariable.metadata;DecisionVariableMetadata.parameters;FnvHashMap[stack] 64
        Instance.decision_variables;DecisionVariable.metadata;DecisionVariableMetadata.subscripts;Vec[stack] 48
        Instance.decision_variables;DecisionVariable.substituted_value;Option[stack] 32
        Instance.objective;Linear;PolynomialBase.terms 80
        Instance.removed_constraints;BTreeMap[stack] 24
        Instance.sense 1
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

        let folded = logical_memory_to_folded(&instance);
        // Note: Same path appears multiple times, flamegraph tools will aggregate them
        insta::assert_snapshot!(folded, @r###"
        Instance.constraint_hints;ConstraintHints.one_hot_constraints;Vec[stack] 24
        Instance.constraint_hints;ConstraintHints.sos1_constraints;Vec[stack] 24
        Instance.constraints;BTreeMap[stack] 24
        Instance.decision_variable_dependency;AcyclicAssignments.assignments;FnvHashMap[stack] 32
        Instance.decision_variable_dependency;AcyclicAssignments.dependency 144
        Instance.decision_variables;BTreeMap[key] 24
        Instance.decision_variables;BTreeMap[stack] 24
        Instance.decision_variables;DecisionVariable.bound 48
        Instance.decision_variables;DecisionVariable.id 24
        Instance.decision_variables;DecisionVariable.kind 3
        Instance.decision_variables;DecisionVariable.metadata;DecisionVariableMetadata.description;Option[stack] 72
        Instance.decision_variables;DecisionVariable.metadata;DecisionVariableMetadata.name 95
        Instance.decision_variables;DecisionVariable.metadata;DecisionVariableMetadata.parameters;FnvHashMap[stack] 96
        Instance.decision_variables;DecisionVariable.metadata;DecisionVariableMetadata.subscripts;Vec[stack] 72
        Instance.decision_variables;DecisionVariable.substituted_value;Option[stack] 48
        Instance.objective;Zero 40
        Instance.removed_constraints;BTreeMap[stack] 24
        Instance.sense 1
        "###);
    }

    #[test]
    fn test_instance_with_parameters_and_description_snapshot() {
        let dv1 = DecisionVariable::continuous(1.into());
        let mut decision_variables = BTreeMap::new();
        decision_variables.insert(dv1.id(), dv1);

        let mut instance = Instance::new(
            crate::instance::Sense::Minimize,
            Function::Zero,
            decision_variables,
            BTreeMap::new(),
        )
        .unwrap();

        // Set parameters
        let mut parameters = v1::Parameters {
            entries: std::collections::HashMap::new(),
        };
        parameters.entries.insert(1, 10.0);
        parameters.entries.insert(2, 20.0);
        instance.parameters = Some(parameters);

        // Set description
        let description = v1::instance::Description {
            name: Some("Test Instance".to_string()),
            description: Some("A test optimization problem".to_string()),
            authors: vec!["Alice".to_string(), "Bob".to_string()],
            created_by: Some("OMMX Test Suite".to_string()),
        };
        instance.description = Some(description);

        let folded = logical_memory_to_folded(&instance);
        insta::assert_snapshot!(folded, @r###"
        Instance.constraint_hints;ConstraintHints.one_hot_constraints;Vec[stack] 24
        Instance.constraint_hints;ConstraintHints.sos1_constraints;Vec[stack] 24
        Instance.constraints;BTreeMap[stack] 24
        Instance.decision_variable_dependency;AcyclicAssignments.assignments;FnvHashMap[stack] 32
        Instance.decision_variable_dependency;AcyclicAssignments.dependency 144
        Instance.decision_variables;BTreeMap[key] 8
        Instance.decision_variables;BTreeMap[stack] 24
        Instance.decision_variables;DecisionVariable.bound 16
        Instance.decision_variables;DecisionVariable.id 8
        Instance.decision_variables;DecisionVariable.kind 1
        Instance.decision_variables;DecisionVariable.metadata;DecisionVariableMetadata.description;Option[stack] 24
        Instance.decision_variables;DecisionVariable.metadata;DecisionVariableMetadata.name;Option[stack] 24
        Instance.decision_variables;DecisionVariable.metadata;DecisionVariableMetadata.parameters;FnvHashMap[stack] 32
        Instance.decision_variables;DecisionVariable.metadata;DecisionVariableMetadata.subscripts;Vec[stack] 24
        Instance.decision_variables;DecisionVariable.substituted_value;Option[stack] 16
        Instance.description;authors 24
        Instance.description;authors;String 56
        Instance.description;created_by 39
        Instance.description;description 51
        Instance.description;name 37
        Instance.objective;Zero 40
        Instance.parameters 48
        Instance.parameters;keys 16
        Instance.parameters;values 16
        Instance.removed_constraints;BTreeMap[stack] 24
        Instance.sense 1
        "###);
    }
}
