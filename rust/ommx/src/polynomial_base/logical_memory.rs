use crate::logical_memory::{LogicalMemoryProfile, LogicalMemoryVisitor};
use crate::polynomial_base::{Monomial, PolynomialBase};
use crate::Coefficient;
use fnv::FnvHashMap;
use std::mem::size_of;

impl<M: Monomial> LogicalMemoryProfile for PolynomialBase<M> {
    fn visit_logical_memory<V: LogicalMemoryVisitor>(
        &self,
        path: &mut Vec<&'static str>,
        visitor: &mut V,
    ) {
        path.push("terms");

        // Calculate FnvHashMap memory usage:
        // 1. HashMap struct overhead
        // 2. Allocated capacity for entries (keys + values)
        //
        // Note: FnvHashMap uses the same layout as std::HashMap,
        // so we calculate: capacity * (size_of::<M>() + size_of::<Coefficient>())
        // plus the HashMap struct itself.

        let map_overhead = size_of::<FnvHashMap<M, Coefficient>>();
        let entry_size = size_of::<(M, Coefficient)>();
        let entries_bytes = self.terms.capacity() * entry_size;

        let total_bytes = map_overhead + entries_bytes;

        visitor.visit_leaf(path, total_bytes);
        path.pop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logical_memory::{logical_memory_to_folded, logical_total_bytes};
    use crate::polynomial_base::Linear;
    use crate::{coeff, linear, quadratic};

    #[test]
    fn test_empty_linear() {
        use crate::polynomial_base::LinearMonomial;

        let linear = Linear::default();
        let total = logical_total_bytes("Linear", &linear);

        // Empty HashMap still has struct overhead
        let expected = size_of::<FnvHashMap<LinearMonomial, Coefficient>>();
        assert_eq!(total, expected);
    }

    #[test]
    fn test_linear_with_terms() {
        // Create: 2*x1 + 3*x2 + 5
        let linear = coeff!(2.0) * linear!(1) + coeff!(3.0) * linear!(2) + coeff!(5.0);

        let folded = logical_memory_to_folded("Linear", &linear);
        assert!(folded.contains("Linear;terms"));

        let total = logical_total_bytes("Linear", &linear);
        assert!(total > 0);
    }

    #[test]
    fn test_quadratic() {
        // Create: x1*x2 + 2*x1 + 1
        let quad = coeff!(1.0) * quadratic!(1, 2) + coeff!(2.0) * quadratic!(1) + coeff!(1.0);

        let folded = logical_memory_to_folded("Quadratic", &quad);
        assert!(folded.contains("Quadratic;terms"));

        let total = logical_total_bytes("Quadratic", &quad);
        assert!(total > 0);
    }

    #[test]
    fn test_folded_format() {
        let linear = coeff!(2.0) * linear!(1);
        let folded = logical_memory_to_folded("Linear", &linear);

        // Should have format: "Linear;terms <bytes>"
        let parts: Vec<&str> = folded.split_whitespace().collect();
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0], "Linear;terms");

        let bytes: usize = parts[1].parse().expect("Should be a valid number");
        assert!(bytes > 0);
    }

    #[test]
    fn test_single_term_snapshot() {
        // Single term: 2*x1
        let linear = coeff!(2.0) * linear!(1);
        let folded = logical_memory_to_folded("Linear", &linear);

        // Snapshot test to verify exact output format
        insta::assert_snapshot!(folded, @"Linear;terms 104");
    }
}
