use crate::instance::{Instance, Sense};
use crate::logical_memory::{LogicalMemoryProfile, LogicalMemoryVisitor, Path};
use crate::v1;
use std::mem::size_of;

impl LogicalMemoryProfile for Sense {
    fn visit_logical_memory<V: LogicalMemoryVisitor>(&self, path: &mut Path, visitor: &mut V) {
        visitor.visit_leaf(path, size_of::<Sense>());
    }
}

// Implementations for protobuf types

crate::impl_logical_memory_profile! {
    v1::Parameters as "Parameters" {
        entries,
    }
}

crate::impl_logical_memory_profile! {
    v1::instance::Description as "Description" {
        name,
        description,
        authors,
        created_by,
    }
}

// Leaf impls for special constraint ID types
macro_rules! impl_id_logical_memory {
    ($id_type:ty) => {
        impl LogicalMemoryProfile for $id_type {
            fn visit_logical_memory<V: LogicalMemoryVisitor>(
                &self,
                path: &mut Path,
                visitor: &mut V,
            ) {
                visitor.visit_leaf(path, std::mem::size_of::<$id_type>());
            }
        }
    };
}
impl_id_logical_memory!(crate::IndicatorConstraintID);
impl_id_logical_memory!(crate::OneHotConstraintID);
impl_id_logical_memory!(crate::Sos1ConstraintID);

// LogicalMemoryProfile for special constraint Created types
// These are simpler than Constraint since they have no function.
macro_rules! impl_special_constraint_profile {
    ($constraint_type:ty, $name:expr) => {
        impl LogicalMemoryProfile for $constraint_type {
            fn visit_logical_memory<V: LogicalMemoryVisitor>(
                &self,
                path: &mut Path,
                visitor: &mut V,
            ) {
                // Count the whole constraint as a single leaf for simplicity
                visitor.visit_leaf(path, std::mem::size_of_val(self));
            }
        }

        impl LogicalMemoryProfile for ($constraint_type, crate::constraint::RemovedReason) {
            fn visit_logical_memory<V: LogicalMemoryVisitor>(
                &self,
                path: &mut Path,
                visitor: &mut V,
            ) {
                self.0
                    .visit_logical_memory(path.with($name).as_mut(), visitor);
                self.1.visit_logical_memory(
                    path.with(concat!($name, ".removed_reason")).as_mut(),
                    visitor,
                );
            }
        }
    };
}
impl_special_constraint_profile!(crate::IndicatorConstraint, "IndicatorConstraint");
impl_special_constraint_profile!(crate::OneHotConstraint, "OneHotConstraint");
impl_special_constraint_profile!(crate::Sos1Constraint, "Sos1Constraint");

macro_rules! impl_constraint_collection_profile {
    ($constraint_type:ty, $active_name:expr, $removed_name:expr) => {
        impl LogicalMemoryProfile
            for crate::constraint_type::ConstraintCollection<$constraint_type>
        {
            fn visit_logical_memory<V: LogicalMemoryVisitor>(
                &self,
                path: &mut Path,
                visitor: &mut V,
            ) {
                self.active()
                    .visit_logical_memory(path.with($active_name).as_mut(), visitor);
                self.removed()
                    .visit_logical_memory(path.with($removed_name).as_mut(), visitor);
                self.metadata()
                    .visit_logical_memory(path.with("metadata").as_mut(), visitor);
            }
        }
    };
}

impl_constraint_collection_profile!(crate::Constraint, "constraints", "removed_constraints");
impl_constraint_collection_profile!(
    crate::IndicatorConstraint,
    "indicator_constraints",
    "removed_indicator_constraints"
);
impl_constraint_collection_profile!(
    crate::OneHotConstraint,
    "one_hot_constraints",
    "removed_one_hot_constraints"
);
impl_constraint_collection_profile!(
    crate::Sos1Constraint,
    "sos1_constraints",
    "removed_sos1_constraints"
);

// `Instance` uses `#[derive(LogicalMemoryProfile)]` on its definition site,
// which reflects every field of the struct automatically. The declarative
// macro invocation it replaced had silently drifted — `named_functions`
// was missing — which is exactly the failure mode the derive is designed
// to prevent.

impl Instance {
    /// Compute the logical memory profile of this instance.
    ///
    /// Returns a [`crate::MemoryProfile`] that can be rendered as a
    /// folded-stack string via [`ToString::to_string`] (for flamegraph
    /// tools) or inspected programmatically via
    /// [`crate::MemoryProfile::entries`] and
    /// [`crate::MemoryProfile::total_bytes`].
    ///
    /// The reported byte counts are a logical estimation, not exact heap
    /// profiling: allocator overhead, padding, and unused capacity are
    /// deliberately ignored. See [`crate::MemoryProfile`] for the
    /// flamegraph workflow and full caveats.
    pub fn logical_memory_profile(&self) -> crate::MemoryProfile {
        crate::logical_memory::build_profile(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constraint::CreatedData;
    use crate::logical_memory::logical_memory_to_folded;
    use crate::{coeff, linear, Constraint, ConstraintID, DecisionVariable, Equality, Function};
    use std::collections::BTreeMap;

    #[test]
    fn test_instance_empty_snapshot() {
        let instance = Instance::default();
        let folded = logical_memory_to_folded(&instance);
        // Empty instance has zero objective
        insta::assert_snapshot!(folded, @r###"
        Instance.constraint_collection;constraints;BTreeMap[stack] 24
        Instance.constraint_collection;metadata;ConstraintMetadataStore.description;FnvHashMap[stack] 32
        Instance.constraint_collection;metadata;ConstraintMetadataStore.name;FnvHashMap[stack] 32
        Instance.constraint_collection;metadata;ConstraintMetadataStore.parameters;FnvHashMap[stack] 32
        Instance.constraint_collection;metadata;ConstraintMetadataStore.provenance;FnvHashMap[stack] 32
        Instance.constraint_collection;metadata;ConstraintMetadataStore.subscripts;FnvHashMap[stack] 32
        Instance.constraint_collection;removed_constraints;BTreeMap[stack] 24
        Instance.decision_variable_dependency;AcyclicAssignments.assignments;FnvHashMap[stack] 32
        Instance.decision_variable_dependency;AcyclicAssignments.dependency 144
        Instance.decision_variables;BTreeMap[stack] 24
        Instance.description;Option[stack] 96
        Instance.indicator_constraint_collection;indicator_constraints;BTreeMap[stack] 24
        Instance.indicator_constraint_collection;metadata;ConstraintMetadataStore.description;FnvHashMap[stack] 32
        Instance.indicator_constraint_collection;metadata;ConstraintMetadataStore.name;FnvHashMap[stack] 32
        Instance.indicator_constraint_collection;metadata;ConstraintMetadataStore.parameters;FnvHashMap[stack] 32
        Instance.indicator_constraint_collection;metadata;ConstraintMetadataStore.provenance;FnvHashMap[stack] 32
        Instance.indicator_constraint_collection;metadata;ConstraintMetadataStore.subscripts;FnvHashMap[stack] 32
        Instance.indicator_constraint_collection;removed_indicator_constraints;BTreeMap[stack] 24
        Instance.named_functions;BTreeMap[stack] 24
        Instance.objective;Zero 40
        Instance.one_hot_constraint_collection;metadata;ConstraintMetadataStore.description;FnvHashMap[stack] 32
        Instance.one_hot_constraint_collection;metadata;ConstraintMetadataStore.name;FnvHashMap[stack] 32
        Instance.one_hot_constraint_collection;metadata;ConstraintMetadataStore.parameters;FnvHashMap[stack] 32
        Instance.one_hot_constraint_collection;metadata;ConstraintMetadataStore.provenance;FnvHashMap[stack] 32
        Instance.one_hot_constraint_collection;metadata;ConstraintMetadataStore.subscripts;FnvHashMap[stack] 32
        Instance.one_hot_constraint_collection;one_hot_constraints;BTreeMap[stack] 24
        Instance.one_hot_constraint_collection;removed_one_hot_constraints;BTreeMap[stack] 24
        Instance.parameters;Option[stack] 48
        Instance.sense 1
        Instance.sos1_constraint_collection;metadata;ConstraintMetadataStore.description;FnvHashMap[stack] 32
        Instance.sos1_constraint_collection;metadata;ConstraintMetadataStore.name;FnvHashMap[stack] 32
        Instance.sos1_constraint_collection;metadata;ConstraintMetadataStore.parameters;FnvHashMap[stack] 32
        Instance.sos1_constraint_collection;metadata;ConstraintMetadataStore.provenance;FnvHashMap[stack] 32
        Instance.sos1_constraint_collection;metadata;ConstraintMetadataStore.subscripts;FnvHashMap[stack] 32
        Instance.sos1_constraint_collection;removed_sos1_constraints;BTreeMap[stack] 24
        Instance.sos1_constraint_collection;sos1_constraints;BTreeMap[stack] 24
        Instance.variable_metadata;VariableMetadataStore.description;FnvHashMap[stack] 32
        Instance.variable_metadata;VariableMetadataStore.name;FnvHashMap[stack] 32
        Instance.variable_metadata;VariableMetadataStore.parameters;FnvHashMap[stack] 32
        Instance.variable_metadata;VariableMetadataStore.subscripts;FnvHashMap[stack] 32
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
        Instance.constraint_collection;constraints;BTreeMap[stack] 24
        Instance.constraint_collection;metadata;ConstraintMetadataStore.description;FnvHashMap[stack] 32
        Instance.constraint_collection;metadata;ConstraintMetadataStore.name;FnvHashMap[stack] 32
        Instance.constraint_collection;metadata;ConstraintMetadataStore.parameters;FnvHashMap[stack] 32
        Instance.constraint_collection;metadata;ConstraintMetadataStore.provenance;FnvHashMap[stack] 32
        Instance.constraint_collection;metadata;ConstraintMetadataStore.subscripts;FnvHashMap[stack] 32
        Instance.constraint_collection;removed_constraints;BTreeMap[stack] 24
        Instance.decision_variable_dependency;AcyclicAssignments.assignments;FnvHashMap[stack] 32
        Instance.decision_variable_dependency;AcyclicAssignments.dependency 144
        Instance.decision_variables;BTreeMap[key] 16
        Instance.decision_variables;BTreeMap[stack] 24
        Instance.decision_variables;DecisionVariable.bound 32
        Instance.decision_variables;DecisionVariable.id 16
        Instance.decision_variables;DecisionVariable.kind 2
        Instance.decision_variables;DecisionVariable.substituted_value;Option[stack] 32
        Instance.description;Option[stack] 96
        Instance.indicator_constraint_collection;indicator_constraints;BTreeMap[stack] 24
        Instance.indicator_constraint_collection;metadata;ConstraintMetadataStore.description;FnvHashMap[stack] 32
        Instance.indicator_constraint_collection;metadata;ConstraintMetadataStore.name;FnvHashMap[stack] 32
        Instance.indicator_constraint_collection;metadata;ConstraintMetadataStore.parameters;FnvHashMap[stack] 32
        Instance.indicator_constraint_collection;metadata;ConstraintMetadataStore.provenance;FnvHashMap[stack] 32
        Instance.indicator_constraint_collection;metadata;ConstraintMetadataStore.subscripts;FnvHashMap[stack] 32
        Instance.indicator_constraint_collection;removed_indicator_constraints;BTreeMap[stack] 24
        Instance.named_functions;BTreeMap[stack] 24
        Instance.objective;Linear;PolynomialBase.terms 80
        Instance.one_hot_constraint_collection;metadata;ConstraintMetadataStore.description;FnvHashMap[stack] 32
        Instance.one_hot_constraint_collection;metadata;ConstraintMetadataStore.name;FnvHashMap[stack] 32
        Instance.one_hot_constraint_collection;metadata;ConstraintMetadataStore.parameters;FnvHashMap[stack] 32
        Instance.one_hot_constraint_collection;metadata;ConstraintMetadataStore.provenance;FnvHashMap[stack] 32
        Instance.one_hot_constraint_collection;metadata;ConstraintMetadataStore.subscripts;FnvHashMap[stack] 32
        Instance.one_hot_constraint_collection;one_hot_constraints;BTreeMap[stack] 24
        Instance.one_hot_constraint_collection;removed_one_hot_constraints;BTreeMap[stack] 24
        Instance.parameters;Option[stack] 48
        Instance.sense 1
        Instance.sos1_constraint_collection;metadata;ConstraintMetadataStore.description;FnvHashMap[stack] 32
        Instance.sos1_constraint_collection;metadata;ConstraintMetadataStore.name;FnvHashMap[stack] 32
        Instance.sos1_constraint_collection;metadata;ConstraintMetadataStore.parameters;FnvHashMap[stack] 32
        Instance.sos1_constraint_collection;metadata;ConstraintMetadataStore.provenance;FnvHashMap[stack] 32
        Instance.sos1_constraint_collection;metadata;ConstraintMetadataStore.subscripts;FnvHashMap[stack] 32
        Instance.sos1_constraint_collection;removed_sos1_constraints;BTreeMap[stack] 24
        Instance.sos1_constraint_collection;sos1_constraints;BTreeMap[stack] 24
        Instance.variable_metadata;VariableMetadataStore.description;FnvHashMap[stack] 32
        Instance.variable_metadata;VariableMetadataStore.name;FnvHashMap[stack] 32
        Instance.variable_metadata;VariableMetadataStore.parameters;FnvHashMap[stack] 32
        Instance.variable_metadata;VariableMetadataStore.subscripts;FnvHashMap[stack] 32
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
            equality: Equality::LessThanOrEqualToZero,
            stage: CreatedData {
                function: Function::Linear(linear!(1) + linear!(2)),
            },
        };

        let mut constraints = BTreeMap::new();
        constraints.insert(ConstraintID::from(1), constraint);

        let instance = Instance::new(
            crate::instance::Sense::Minimize,
            objective,
            decision_variables,
            constraints,
        )
        .unwrap();

        let folded = logical_memory_to_folded(&instance);
        insta::assert_snapshot!(folded, @r###"
        Instance.constraint_collection;constraints;BTreeMap[key] 8
        Instance.constraint_collection;constraints;BTreeMap[stack] 24
        Instance.constraint_collection;constraints;Constraint.equality 1
        Instance.constraint_collection;constraints;Constraint.stage;CreatedData.function;Linear;PolynomialBase.terms 80
        Instance.constraint_collection;metadata;ConstraintMetadataStore.description;FnvHashMap[stack] 32
        Instance.constraint_collection;metadata;ConstraintMetadataStore.name;FnvHashMap[stack] 32
        Instance.constraint_collection;metadata;ConstraintMetadataStore.parameters;FnvHashMap[stack] 32
        Instance.constraint_collection;metadata;ConstraintMetadataStore.provenance;FnvHashMap[stack] 32
        Instance.constraint_collection;metadata;ConstraintMetadataStore.subscripts;FnvHashMap[stack] 32
        Instance.constraint_collection;removed_constraints;BTreeMap[stack] 24
        Instance.decision_variable_dependency;AcyclicAssignments.assignments;FnvHashMap[stack] 32
        Instance.decision_variable_dependency;AcyclicAssignments.dependency 144
        Instance.decision_variables;BTreeMap[key] 16
        Instance.decision_variables;BTreeMap[stack] 24
        Instance.decision_variables;DecisionVariable.bound 32
        Instance.decision_variables;DecisionVariable.id 16
        Instance.decision_variables;DecisionVariable.kind 2
        Instance.decision_variables;DecisionVariable.substituted_value;Option[stack] 32
        Instance.description;Option[stack] 96
        Instance.indicator_constraint_collection;indicator_constraints;BTreeMap[stack] 24
        Instance.indicator_constraint_collection;metadata;ConstraintMetadataStore.description;FnvHashMap[stack] 32
        Instance.indicator_constraint_collection;metadata;ConstraintMetadataStore.name;FnvHashMap[stack] 32
        Instance.indicator_constraint_collection;metadata;ConstraintMetadataStore.parameters;FnvHashMap[stack] 32
        Instance.indicator_constraint_collection;metadata;ConstraintMetadataStore.provenance;FnvHashMap[stack] 32
        Instance.indicator_constraint_collection;metadata;ConstraintMetadataStore.subscripts;FnvHashMap[stack] 32
        Instance.indicator_constraint_collection;removed_indicator_constraints;BTreeMap[stack] 24
        Instance.named_functions;BTreeMap[stack] 24
        Instance.objective;Linear;PolynomialBase.terms 80
        Instance.one_hot_constraint_collection;metadata;ConstraintMetadataStore.description;FnvHashMap[stack] 32
        Instance.one_hot_constraint_collection;metadata;ConstraintMetadataStore.name;FnvHashMap[stack] 32
        Instance.one_hot_constraint_collection;metadata;ConstraintMetadataStore.parameters;FnvHashMap[stack] 32
        Instance.one_hot_constraint_collection;metadata;ConstraintMetadataStore.provenance;FnvHashMap[stack] 32
        Instance.one_hot_constraint_collection;metadata;ConstraintMetadataStore.subscripts;FnvHashMap[stack] 32
        Instance.one_hot_constraint_collection;one_hot_constraints;BTreeMap[stack] 24
        Instance.one_hot_constraint_collection;removed_one_hot_constraints;BTreeMap[stack] 24
        Instance.parameters;Option[stack] 48
        Instance.sense 1
        Instance.sos1_constraint_collection;metadata;ConstraintMetadataStore.description;FnvHashMap[stack] 32
        Instance.sos1_constraint_collection;metadata;ConstraintMetadataStore.name;FnvHashMap[stack] 32
        Instance.sos1_constraint_collection;metadata;ConstraintMetadataStore.parameters;FnvHashMap[stack] 32
        Instance.sos1_constraint_collection;metadata;ConstraintMetadataStore.provenance;FnvHashMap[stack] 32
        Instance.sos1_constraint_collection;metadata;ConstraintMetadataStore.subscripts;FnvHashMap[stack] 32
        Instance.sos1_constraint_collection;removed_sos1_constraints;BTreeMap[stack] 24
        Instance.sos1_constraint_collection;sos1_constraints;BTreeMap[stack] 24
        Instance.variable_metadata;VariableMetadataStore.description;FnvHashMap[stack] 32
        Instance.variable_metadata;VariableMetadataStore.name;FnvHashMap[stack] 32
        Instance.variable_metadata;VariableMetadataStore.parameters;FnvHashMap[stack] 32
        Instance.variable_metadata;VariableMetadataStore.subscripts;FnvHashMap[stack] 32
        "###);
    }

    #[test]
    fn test_instance_with_multiple_variables_with_metadata_snapshot() {
        // Create 3 decision variables with names (stored in the SoA store)
        let dv1 = DecisionVariable::continuous(1.into());
        let dv2 = DecisionVariable::continuous(2.into());
        let dv3 = DecisionVariable::continuous(3.into());

        let mut decision_variables = BTreeMap::new();
        decision_variables.insert(dv1.id(), dv1);
        decision_variables.insert(dv2.id(), dv2);
        decision_variables.insert(dv3.id(), dv3);

        let mut instance = Instance::new(
            crate::instance::Sense::Minimize,
            Function::Zero,
            decision_variables,
            BTreeMap::new(),
        )
        .unwrap();
        instance.variable_metadata_mut().set_name(1.into(), "x1");
        instance.variable_metadata_mut().set_name(2.into(), "x2");
        instance
            .variable_metadata_mut()
            .set_name(3.into(), "x3_with_longer_name");

        let folded = logical_memory_to_folded(&instance);
        // Note: Same path appears multiple times, flamegraph tools will aggregate them
        insta::assert_snapshot!(folded, @r###"
        Instance.constraint_collection;constraints;BTreeMap[stack] 24
        Instance.constraint_collection;metadata;ConstraintMetadataStore.description;FnvHashMap[stack] 32
        Instance.constraint_collection;metadata;ConstraintMetadataStore.name;FnvHashMap[stack] 32
        Instance.constraint_collection;metadata;ConstraintMetadataStore.parameters;FnvHashMap[stack] 32
        Instance.constraint_collection;metadata;ConstraintMetadataStore.provenance;FnvHashMap[stack] 32
        Instance.constraint_collection;metadata;ConstraintMetadataStore.subscripts;FnvHashMap[stack] 32
        Instance.constraint_collection;removed_constraints;BTreeMap[stack] 24
        Instance.decision_variable_dependency;AcyclicAssignments.assignments;FnvHashMap[stack] 32
        Instance.decision_variable_dependency;AcyclicAssignments.dependency 144
        Instance.decision_variables;BTreeMap[key] 24
        Instance.decision_variables;BTreeMap[stack] 24
        Instance.decision_variables;DecisionVariable.bound 48
        Instance.decision_variables;DecisionVariable.id 24
        Instance.decision_variables;DecisionVariable.kind 3
        Instance.decision_variables;DecisionVariable.substituted_value;Option[stack] 48
        Instance.description;Option[stack] 96
        Instance.indicator_constraint_collection;indicator_constraints;BTreeMap[stack] 24
        Instance.indicator_constraint_collection;metadata;ConstraintMetadataStore.description;FnvHashMap[stack] 32
        Instance.indicator_constraint_collection;metadata;ConstraintMetadataStore.name;FnvHashMap[stack] 32
        Instance.indicator_constraint_collection;metadata;ConstraintMetadataStore.parameters;FnvHashMap[stack] 32
        Instance.indicator_constraint_collection;metadata;ConstraintMetadataStore.provenance;FnvHashMap[stack] 32
        Instance.indicator_constraint_collection;metadata;ConstraintMetadataStore.subscripts;FnvHashMap[stack] 32
        Instance.indicator_constraint_collection;removed_indicator_constraints;BTreeMap[stack] 24
        Instance.named_functions;BTreeMap[stack] 24
        Instance.objective;Zero 40
        Instance.one_hot_constraint_collection;metadata;ConstraintMetadataStore.description;FnvHashMap[stack] 32
        Instance.one_hot_constraint_collection;metadata;ConstraintMetadataStore.name;FnvHashMap[stack] 32
        Instance.one_hot_constraint_collection;metadata;ConstraintMetadataStore.parameters;FnvHashMap[stack] 32
        Instance.one_hot_constraint_collection;metadata;ConstraintMetadataStore.provenance;FnvHashMap[stack] 32
        Instance.one_hot_constraint_collection;metadata;ConstraintMetadataStore.subscripts;FnvHashMap[stack] 32
        Instance.one_hot_constraint_collection;one_hot_constraints;BTreeMap[stack] 24
        Instance.one_hot_constraint_collection;removed_one_hot_constraints;BTreeMap[stack] 24
        Instance.parameters;Option[stack] 48
        Instance.sense 1
        Instance.sos1_constraint_collection;metadata;ConstraintMetadataStore.description;FnvHashMap[stack] 32
        Instance.sos1_constraint_collection;metadata;ConstraintMetadataStore.name;FnvHashMap[stack] 32
        Instance.sos1_constraint_collection;metadata;ConstraintMetadataStore.parameters;FnvHashMap[stack] 32
        Instance.sos1_constraint_collection;metadata;ConstraintMetadataStore.provenance;FnvHashMap[stack] 32
        Instance.sos1_constraint_collection;metadata;ConstraintMetadataStore.subscripts;FnvHashMap[stack] 32
        Instance.sos1_constraint_collection;removed_sos1_constraints;BTreeMap[stack] 24
        Instance.sos1_constraint_collection;sos1_constraints;BTreeMap[stack] 24
        Instance.variable_metadata;VariableMetadataStore.description;FnvHashMap[stack] 32
        Instance.variable_metadata;VariableMetadataStore.name 95
        Instance.variable_metadata;VariableMetadataStore.name;FnvHashMap[key] 24
        Instance.variable_metadata;VariableMetadataStore.name;FnvHashMap[stack] 32
        Instance.variable_metadata;VariableMetadataStore.parameters;FnvHashMap[stack] 32
        Instance.variable_metadata;VariableMetadataStore.subscripts;FnvHashMap[stack] 32
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
        Instance.constraint_collection;constraints;BTreeMap[stack] 24
        Instance.constraint_collection;metadata;ConstraintMetadataStore.description;FnvHashMap[stack] 32
        Instance.constraint_collection;metadata;ConstraintMetadataStore.name;FnvHashMap[stack] 32
        Instance.constraint_collection;metadata;ConstraintMetadataStore.parameters;FnvHashMap[stack] 32
        Instance.constraint_collection;metadata;ConstraintMetadataStore.provenance;FnvHashMap[stack] 32
        Instance.constraint_collection;metadata;ConstraintMetadataStore.subscripts;FnvHashMap[stack] 32
        Instance.constraint_collection;removed_constraints;BTreeMap[stack] 24
        Instance.decision_variable_dependency;AcyclicAssignments.assignments;FnvHashMap[stack] 32
        Instance.decision_variable_dependency;AcyclicAssignments.dependency 144
        Instance.decision_variables;BTreeMap[key] 8
        Instance.decision_variables;BTreeMap[stack] 24
        Instance.decision_variables;DecisionVariable.bound 16
        Instance.decision_variables;DecisionVariable.id 8
        Instance.decision_variables;DecisionVariable.kind 1
        Instance.decision_variables;DecisionVariable.substituted_value;Option[stack] 16
        Instance.description;Description.authors 56
        Instance.description;Description.authors;Vec[stack] 24
        Instance.description;Description.created_by 39
        Instance.description;Description.description 51
        Instance.description;Description.name 37
        Instance.indicator_constraint_collection;indicator_constraints;BTreeMap[stack] 24
        Instance.indicator_constraint_collection;metadata;ConstraintMetadataStore.description;FnvHashMap[stack] 32
        Instance.indicator_constraint_collection;metadata;ConstraintMetadataStore.name;FnvHashMap[stack] 32
        Instance.indicator_constraint_collection;metadata;ConstraintMetadataStore.parameters;FnvHashMap[stack] 32
        Instance.indicator_constraint_collection;metadata;ConstraintMetadataStore.provenance;FnvHashMap[stack] 32
        Instance.indicator_constraint_collection;metadata;ConstraintMetadataStore.subscripts;FnvHashMap[stack] 32
        Instance.indicator_constraint_collection;removed_indicator_constraints;BTreeMap[stack] 24
        Instance.named_functions;BTreeMap[stack] 24
        Instance.objective;Zero 40
        Instance.one_hot_constraint_collection;metadata;ConstraintMetadataStore.description;FnvHashMap[stack] 32
        Instance.one_hot_constraint_collection;metadata;ConstraintMetadataStore.name;FnvHashMap[stack] 32
        Instance.one_hot_constraint_collection;metadata;ConstraintMetadataStore.parameters;FnvHashMap[stack] 32
        Instance.one_hot_constraint_collection;metadata;ConstraintMetadataStore.provenance;FnvHashMap[stack] 32
        Instance.one_hot_constraint_collection;metadata;ConstraintMetadataStore.subscripts;FnvHashMap[stack] 32
        Instance.one_hot_constraint_collection;one_hot_constraints;BTreeMap[stack] 24
        Instance.one_hot_constraint_collection;removed_one_hot_constraints;BTreeMap[stack] 24
        Instance.parameters;Parameters.entries 16
        Instance.parameters;Parameters.entries;HashMap[key] 16
        Instance.parameters;Parameters.entries;HashMap[stack] 48
        Instance.sense 1
        Instance.sos1_constraint_collection;metadata;ConstraintMetadataStore.description;FnvHashMap[stack] 32
        Instance.sos1_constraint_collection;metadata;ConstraintMetadataStore.name;FnvHashMap[stack] 32
        Instance.sos1_constraint_collection;metadata;ConstraintMetadataStore.parameters;FnvHashMap[stack] 32
        Instance.sos1_constraint_collection;metadata;ConstraintMetadataStore.provenance;FnvHashMap[stack] 32
        Instance.sos1_constraint_collection;metadata;ConstraintMetadataStore.subscripts;FnvHashMap[stack] 32
        Instance.sos1_constraint_collection;removed_sos1_constraints;BTreeMap[stack] 24
        Instance.sos1_constraint_collection;sos1_constraints;BTreeMap[stack] 24
        Instance.variable_metadata;VariableMetadataStore.description;FnvHashMap[stack] 32
        Instance.variable_metadata;VariableMetadataStore.name;FnvHashMap[stack] 32
        Instance.variable_metadata;VariableMetadataStore.parameters;FnvHashMap[stack] 32
        Instance.variable_metadata;VariableMetadataStore.subscripts;FnvHashMap[stack] 32
        "###);
    }
}
