use crate::constraint::{Constraint, ConstraintID, Created, Equality, Provenance, RemovedReason};
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

impl LogicalMemoryProfile for Provenance {
    fn visit_logical_memory<V: LogicalMemoryVisitor>(&self, path: &mut Path, visitor: &mut V) {
        visitor.visit_leaf(path, size_of::<Provenance>());
    }
}

// ConstraintMetadata, CreatedData, and RemovedReason use
// `#[derive(LogicalMemoryProfile)]` on their definition sites.

// Constraint<Created> - manually implemented because generic types
// cannot be used with the simple ident-based macro form.
impl LogicalMemoryProfile for Constraint<Created> {
    fn visit_logical_memory<V: LogicalMemoryVisitor>(&self, path: &mut Path, visitor: &mut V) {
        self.equality
            .visit_logical_memory(path.with("Constraint.equality").as_mut(), visitor);
        self.metadata
            .visit_logical_memory(path.with("Constraint.metadata").as_mut(), visitor);
        self.stage
            .visit_logical_memory(path.with("Constraint.stage").as_mut(), visitor);
    }
}

impl LogicalMemoryProfile for (Constraint<Created>, RemovedReason) {
    fn visit_logical_memory<V: LogicalMemoryVisitor>(&self, path: &mut Path, visitor: &mut V) {
        self.0
            .visit_logical_memory(path.with("RemovedConstraint").as_mut(), visitor);
        self.1.visit_logical_memory(
            path.with("RemovedConstraint.removed_reason").as_mut(),
            visitor,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logical_memory::logical_memory_to_folded;
    use crate::{coeff, linear, Function};

    #[test]
    fn test_constraint_snapshot() {
        let constraint = Constraint::equal_to_zero(Function::Linear(
            coeff!(2.0) * linear!(1) + coeff!(3.0) * linear!(2),
        ));
        let folded = logical_memory_to_folded(&constraint);
        insta::assert_snapshot!(folded);
    }

    #[test]
    fn test_constraint_with_metadata_snapshot() {
        let mut constraint = Constraint::equal_to_zero(Function::Linear(coeff!(2.0) * linear!(1)));
        constraint.metadata.name = Some("test_constraint".to_string());
        constraint.metadata.description = Some("A test constraint".to_string());
        constraint.metadata.subscripts = vec![1, 2, 3];

        let folded = logical_memory_to_folded(&constraint);
        insta::assert_snapshot!(folded);
    }
}
