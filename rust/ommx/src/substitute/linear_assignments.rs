use crate::{decision_variable::VariableID, Evaluate, Linear};
use fnv::FnvHashMap;
use petgraph::algo;
use petgraph::prelude::DiGraphMap;
use proptest::prelude::*;

use super::error::RecursiveAssignmentError;

/// Represents a set of assignment rules (`VariableID` -> `Linear`)
/// that has been validated to be free of any circular dependencies.
#[derive(Debug, Clone)]
pub struct AcyclicLinearAssignments {
    assignments: FnvHashMap<VariableID, Linear>,
    dependency: DiGraphMap<VariableID, ()>,
}

impl AcyclicLinearAssignments {
    pub fn new(
        iter: impl IntoIterator<Item = (VariableID, Linear)>,
    ) -> Result<Self, RecursiveAssignmentError> {
        let assignments: FnvHashMap<VariableID, Linear> = iter.into_iter().collect();
        let mut dependency = DiGraphMap::new();

        // Add all variables being assigned to as nodes
        for &var_id in assignments.keys() {
            dependency.add_node(var_id);
        }

        // Add edges for dependencies
        for (&assigned_var, linear) in &assignments {
            for required_var in linear.required_ids() {
                // Add edge from required_var to assigned_var (dependency direction)
                dependency.add_edge(required_var, assigned_var, ());
            }
        }

        // Check if the dependency graph is acyclic
        if algo::is_cyclic_directed(&dependency) {
            // Find a variable that participates in a cycle for error reporting
            // We can use any variable that's part of a strongly connected component
            for &var_id in assignments.keys() {
                return Err(RecursiveAssignmentError { var_id });
            }
            // This should never be reached if assignments is non-empty
            unreachable!("Found cycle but no variables in assignments");
        }

        Ok(Self {
            assignments,
            dependency,
        })
    }

    // Get the assignments in a topologically sorted order.
    pub fn sorted_iter(&self) -> impl Iterator<Item = (VariableID, &Linear)> {
        // Get topological order of the dependency graph
        let topo_order = algo::toposort(&self.dependency, None)
            .expect("Graph should be acyclic by construction");

        // Create iterator that yields assignments in topological order
        topo_order
            .into_iter()
            .filter_map(move |var_id| self.assignments.get(&var_id).map(|linear| (var_id, linear)))
    }
}

impl Arbitrary for AcyclicLinearAssignments {
    type Parameters = ();
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
        // Generate a random acyclic graph of assignments
        let strategy = proptest::collection::vec(
            ((0..100_u64).prop_map(VariableID::from), Linear::arbitrary()),
            0..=10,
        )
        .prop_filter_map("Acyclic", |assignments| {
            AcyclicLinearAssignments::new(assignments).ok()
        })
        .boxed();

        strategy
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Coefficient, LinearMonomial, PolynomialBase};

    #[test]
    fn test_acyclic_linear_assignments_acyclic() {
        let x1_id = VariableID::from(1);
        let x2_id = VariableID::from(2);
        let x3_id = VariableID::from(3);

        // Create x1 = x2 + 1, x2 = x3 + 2 (valid, no cycles)
        let assignments = vec![
            (
                x1_id,
                PolynomialBase::single_term(
                    LinearMonomial::Variable(x2_id),
                    Coefficient::try_from(1.0).unwrap(),
                ) + PolynomialBase::from(Coefficient::try_from(1.0).unwrap()),
            ),
            (
                x2_id,
                PolynomialBase::single_term(
                    LinearMonomial::Variable(x3_id),
                    Coefficient::try_from(1.0).unwrap(),
                ) + PolynomialBase::from(Coefficient::try_from(2.0).unwrap()),
            ),
        ];

        let acyclic_assignments = AcyclicLinearAssignments::new(assignments).unwrap();

        // Test that sorted_iter works
        let sorted: Vec<_> = acyclic_assignments.sorted_iter().collect();
        assert_eq!(sorted.len(), 2);

        // x2 should come before x1 in topological order since x1 depends on x2
        let var_order: Vec<_> = sorted.iter().map(|(var_id, _)| *var_id).collect();
        let x2_pos = var_order.iter().position(|&v| v == x2_id).unwrap();
        let x1_pos = var_order.iter().position(|&v| v == x1_id).unwrap();
        assert!(
            x2_pos < x1_pos,
            "x2 should come before x1 in topological order"
        );
    }

    #[test]
    fn test_acyclic_linear_assignments_cyclic() {
        let x1_id = VariableID::from(1);
        let x2_id = VariableID::from(2);

        // Create x1 = x2 + 1, x2 = x1 + 2 (cyclic, should fail)
        let assignments = vec![
            (
                x1_id,
                PolynomialBase::single_term(
                    LinearMonomial::Variable(x2_id),
                    Coefficient::try_from(1.0).unwrap(),
                ) + PolynomialBase::from(Coefficient::try_from(1.0).unwrap()),
            ),
            (
                x2_id,
                PolynomialBase::single_term(
                    LinearMonomial::Variable(x1_id),
                    Coefficient::try_from(1.0).unwrap(),
                ) + PolynomialBase::from(Coefficient::try_from(2.0).unwrap()),
            ),
        ];

        let result = AcyclicLinearAssignments::new(assignments);
        assert!(result.is_err());
    }

    #[test]
    fn test_acyclic_linear_assignments_self_reference() {
        let x1_id = VariableID::from(1);

        // Create x1 = x1 + 2 (self-reference, should fail)
        let assignments = vec![(
            x1_id,
            PolynomialBase::single_term(
                LinearMonomial::Variable(x1_id),
                Coefficient::try_from(1.0).unwrap(),
            ) + PolynomialBase::from(Coefficient::try_from(2.0).unwrap()),
        )];

        let result = AcyclicLinearAssignments::new(assignments);
        assert!(result.is_err());
    }
}
