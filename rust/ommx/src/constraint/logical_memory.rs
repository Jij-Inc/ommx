use crate::constraint::{Constraint, ConstraintID, RemovedConstraint};
use crate::logical_memory::{LogicalMemoryProfile, LogicalMemoryVisitor, Path};
use std::mem::size_of;

impl LogicalMemoryProfile for ConstraintID {
    fn visit_logical_memory<V: LogicalMemoryVisitor>(&self, path: &mut Path, visitor: &mut V) {
        visitor.visit_leaf(path, size_of::<ConstraintID>());
    }
}

impl LogicalMemoryProfile for Constraint {
    fn visit_logical_memory<V: LogicalMemoryVisitor>(&self, path: &mut Path, visitor: &mut V) {
        // Count each field individually to avoid double-counting
        // Use "Type.field" format for flamegraph clarity

        // id: ConstraintID (u64 wrapper)
        visitor.visit_leaf(&path.with("Constraint.id"), size_of::<crate::ConstraintID>());

        // equality: Equality (enum)
        visitor.visit_leaf(&path.with("Constraint.equality"), size_of::<crate::Equality>());

        // Delegate to Function
        self.function
            .visit_logical_memory(path.with("Constraint.function").as_mut(), visitor);

        // name: Option<String>
        self.name
            .visit_logical_memory(path.with("Constraint.name").as_mut(), visitor);

        // subscripts: Vec<i64>
        self.subscripts
            .visit_logical_memory(path.with("Constraint.subscripts").as_mut(), visitor);

        // parameters: FnvHashMap<String, String>
        self.parameters
            .visit_logical_memory(path.with("Constraint.parameters").as_mut(), visitor);

        // description: Option<String>
        self.description
            .visit_logical_memory(path.with("Constraint.description").as_mut(), visitor);
    }
}

impl LogicalMemoryProfile for RemovedConstraint {
    fn visit_logical_memory<V: LogicalMemoryVisitor>(&self, path: &mut Path, visitor: &mut V) {
        // Count each field individually to avoid double-counting
        // Use "Type.field" format for flamegraph clarity

        // Delegate to Constraint
        self.constraint
            .visit_logical_memory(path.with("RemovedConstraint.constraint").as_mut(), visitor);

        // removed_reason: String
        self.removed_reason
            .visit_logical_memory(path.with("RemovedConstraint.removed_reason").as_mut(), visitor);

        // removed_reason_parameters: FnvHashMap<String, String>
        self.removed_reason_parameters
            .visit_logical_memory(path.with("RemovedConstraint.removed_reason_parameters").as_mut(), visitor);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constraint::ConstraintID;
    use crate::logical_memory::logical_memory_to_folded;
    use crate::{coeff, linear, Function};

    #[test]
    fn test_constraint_snapshot() {
        let constraint = Constraint::equal_to_zero(
            ConstraintID::from(1),
            Function::Linear(coeff!(2.0) * linear!(1) + coeff!(3.0) * linear!(2)),
        );
        let folded = logical_memory_to_folded(&constraint);
        insta::assert_snapshot!(folded, @r###"
        Constraint.description 24
        Constraint.equality 1
        Constraint.function;Linear;PolynomialBase.terms 80
        Constraint.id 8
        Constraint.name 24
        Constraint.parameters;FnvHashMap[overhead] 32
        Constraint.subscripts;Vec[overhead] 24
        "###);
    }

    #[test]
    fn test_constraint_with_metadata_snapshot() {
        let mut constraint = Constraint::equal_to_zero(
            ConstraintID::from(1),
            Function::Linear(coeff!(2.0) * linear!(1)),
        );
        constraint.name = Some("test_constraint".to_string());
        constraint.description = Some("A test constraint".to_string());
        constraint.subscripts = vec![1, 2, 3];

        let folded = logical_memory_to_folded(&constraint);
        // Should include function, name, description, and subscripts
        insta::assert_snapshot!(folded, @r###"
        Constraint.description 41
        Constraint.equality 1
        Constraint.function;Linear;PolynomialBase.terms 56
        Constraint.id 8
        Constraint.name 39
        Constraint.parameters;FnvHashMap[overhead] 32
        Constraint.subscripts 24
        Constraint.subscripts;Vec[overhead] 24
        "###);
    }

    #[test]
    fn test_removed_constraint_snapshot() {
        let constraint = Constraint::less_than_or_equal_to_zero(
            ConstraintID::from(2),
            Function::Linear(coeff!(1.0) * linear!(3)),
        );
        let removed = RemovedConstraint {
            constraint,
            removed_reason: "infeasible".to_string(),
            removed_reason_parameters: Default::default(),
        };

        let folded = logical_memory_to_folded(&removed);
        insta::assert_snapshot!(folded, @r###"
        RemovedConstraint.constraint;Constraint.description 24
        RemovedConstraint.constraint;Constraint.equality 1
        RemovedConstraint.constraint;Constraint.function;Linear;PolynomialBase.terms 56
        RemovedConstraint.constraint;Constraint.id 8
        RemovedConstraint.constraint;Constraint.name 24
        RemovedConstraint.constraint;Constraint.parameters;FnvHashMap[overhead] 32
        RemovedConstraint.constraint;Constraint.subscripts;Vec[overhead] 24
        RemovedConstraint.removed_reason 34
        RemovedConstraint.removed_reason_parameters;FnvHashMap[overhead] 32
        "###);
    }
}
