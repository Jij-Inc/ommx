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
    use crate::logical_memory::logical_memory_to_folded;
    use crate::polynomial_base::Linear;
    use crate::{coeff, linear, quadratic};

    #[test]
    fn test_empty_linear_snapshot() {
        let linear = Linear::default();
        let folded = logical_memory_to_folded("Linear", &linear);

        // Empty HashMap: only struct overhead, no entries
        insta::assert_snapshot!(folded, @"Linear;terms 32");
    }

    #[test]
    fn test_single_term_snapshot() {
        // Single term: 2*x1
        let linear = coeff!(2.0) * linear!(1);
        let folded = logical_memory_to_folded("Linear", &linear);

        insta::assert_snapshot!(folded, @"Linear;terms 104");
    }

    #[test]
    fn test_linear_three_terms_snapshot() {
        // Create: 2*x1 + 3*x2 + 5 (3 terms)
        let linear = coeff!(2.0) * linear!(1) + coeff!(3.0) * linear!(2) + coeff!(5.0);
        let folded = logical_memory_to_folded("Linear", &linear);

        insta::assert_snapshot!(folded, @"Linear;terms 104");
    }

    #[test]
    fn test_quadratic_three_terms_snapshot() {
        // Create: x1*x2 + 2*x1 + 1 (3 terms)
        let quad = coeff!(1.0) * quadratic!(1, 2) + coeff!(2.0) * quadratic!(1) + coeff!(1.0);
        let folded = logical_memory_to_folded("Quadratic", &quad);

        insta::assert_snapshot!(folded, @"Quadratic;terms 128");
    }
}
