use crate::constraint::{
    Constraint, ConstraintID, ConstraintMetadata, Created, CreatedData, Equality,
    RemovedConstraint, RemovedData,
};
use crate::logical_memory::{LogicalMemoryProfile, LogicalMemoryVisitor, Path};
use std::mem::size_of;

impl LogicalMemoryProfile for ConstraintID {
    fn visit_logical_memory<V: LogicalMemoryVisitor>(&self, path: &mut Path, visitor: &mut V) {
        visitor.visit_leaf(path, size_of::<ConstraintID>());
    }
}

impl LogicalMemoryProfile for Equality {
    fn visit_logical_memory<V: LogicalMemoryVisitor>(&self, path: &mut Path, visitor: &mut V) {
        visitor.visit_leaf(path, size_of::<Equality>());
    }
}

crate::impl_logical_memory_profile! {
    ConstraintMetadata {
        name,
        subscripts,
        parameters,
        description,
    }
}

crate::impl_logical_memory_profile! {
    CreatedData {
        function,
    }
}

crate::impl_logical_memory_profile! {
    RemovedData {
        function,
        removed_reason,
        removed_reason_parameters,
    }
}

// Constraint<Created> - manually implemented because generic types
// cannot be used with the simple ident-based macro form.
impl LogicalMemoryProfile for Constraint<Created> {
    fn visit_logical_memory<V: LogicalMemoryVisitor>(&self, path: &mut Path, visitor: &mut V) {
        self.id
            .visit_logical_memory(path.with("Constraint.id").as_mut(), visitor);
        self.equality
            .visit_logical_memory(path.with("Constraint.equality").as_mut(), visitor);
        self.metadata
            .visit_logical_memory(path.with("Constraint.metadata").as_mut(), visitor);
        self.stage
            .visit_logical_memory(path.with("Constraint.stage").as_mut(), visitor);
    }
}

// Constraint<Removed> (aka RemovedConstraint)
impl LogicalMemoryProfile for RemovedConstraint {
    fn visit_logical_memory<V: LogicalMemoryVisitor>(&self, path: &mut Path, visitor: &mut V) {
        self.id
            .visit_logical_memory(path.with("RemovedConstraint.id").as_mut(), visitor);
        self.equality
            .visit_logical_memory(path.with("RemovedConstraint.equality").as_mut(), visitor);
        self.metadata
            .visit_logical_memory(path.with("RemovedConstraint.metadata").as_mut(), visitor);
        self.stage
            .visit_logical_memory(path.with("RemovedConstraint.stage").as_mut(), visitor);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constraint::ConstraintID;
    use crate::logical_memory::logical_memory_to_folded;
    use crate::{coeff, linear, Function};
    use fnv::FnvHashMap;

    #[test]
    fn test_constraint_snapshot() {
        let constraint = Constraint::equal_to_zero(
            ConstraintID::from(1),
            Function::Linear(coeff!(2.0) * linear!(1) + coeff!(3.0) * linear!(2)),
        );
        let folded = logical_memory_to_folded(&constraint);
        insta::assert_snapshot!(folded);
    }

    #[test]
    fn test_constraint_with_metadata_snapshot() {
        let mut constraint = Constraint::equal_to_zero(
            ConstraintID::from(1),
            Function::Linear(coeff!(2.0) * linear!(1)),
        );
        constraint.metadata.name = Some("test_constraint".to_string());
        constraint.metadata.description = Some("A test constraint".to_string());
        constraint.metadata.subscripts = vec![1, 2, 3];

        let folded = logical_memory_to_folded(&constraint);
        insta::assert_snapshot!(folded);
    }

    #[test]
    fn test_removed_constraint_snapshot() {
        let constraint = Constraint::less_than_or_equal_to_zero(
            ConstraintID::from(2),
            Function::Linear(coeff!(1.0) * linear!(3)),
        );
        let removed = RemovedConstraint {
            id: constraint.id,
            equality: constraint.equality,
            metadata: constraint.metadata,
            stage: RemovedData {
                function: constraint.stage.function,
                removed_reason: "infeasible".to_string(),
                removed_reason_parameters: FnvHashMap::default(),
            },
        };

        let folded = logical_memory_to_folded(&removed);
        insta::assert_snapshot!(folded);
    }
}
