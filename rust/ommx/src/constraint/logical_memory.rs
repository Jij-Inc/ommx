use crate::constraint::{Constraint, Created};
use crate::logical_memory::{LogicalMemoryProfile, LogicalMemoryVisitor, Path};

// ConstraintContext, CreatedData, and RemovedReason use
// `#[derive(LogicalMemoryProfile)]` on their definition sites.

// Constraint<Created> - manually implemented because generic types
// cannot be used with the simple ident-based macro form.
//
// Per-constraint context lives on the enclosing collection's
// ConstraintContextStore (visited at the collection level), so this
// per-element profile only accounts for the intrinsic data.
impl LogicalMemoryProfile for Constraint<Created> {
    fn visit_logical_memory<V: LogicalMemoryVisitor>(&self, path: &mut Path, visitor: &mut V) {
        self.equality
            .visit_logical_memory(path.with("Constraint.equality").as_mut(), visitor);
        self.stage
            .visit_logical_memory(path.with("Constraint.stage").as_mut(), visitor);
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
            ((coeff!(2.0) * linear!(1)).unwrap() + (coeff!(3.0) * linear!(2)).unwrap()).unwrap(),
        ));
        let folded = logical_memory_to_folded(&constraint);
        insta::assert_snapshot!(folded);
    }

    // The previous `test_constraint_with_context_snapshot` exercised
    // per-element `Constraint` context storage, which was retired in v3.
    // Per-constraint context is now visited at the
    // `ConstraintCollection::context()` SoA-store level (see
    // `instance/logical_memory.rs`), so the equivalent snapshot lives
    // there.
}
