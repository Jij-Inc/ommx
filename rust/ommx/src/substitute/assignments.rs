use crate::{decision_variable::VariableID, Evaluate, Function};
use fnv::FnvHashMap;
use petgraph::algo;
use petgraph::prelude::DiGraphMap;
use proptest::prelude::*;

use super::error::SubstitutionError;

/// Represents a set of assignment rules (`VariableID` -> `Function`)
/// that has been validated to be free of any circular dependencies.
#[derive(Debug, Clone)]
pub struct AcyclicAssignments {
    assignments: FnvHashMap<VariableID, Function>,
    // The directed graph representing dependencies between assignments, assigned -> required.
    dependency: DiGraphMap<VariableID, ()>,
}

impl AcyclicAssignments {
    pub fn new(
        iter: impl IntoIterator<Item = (VariableID, Function)>,
    ) -> Result<Self, SubstitutionError> {
        let assignments: FnvHashMap<VariableID, Function> = iter.into_iter().collect();
        let mut dependency = DiGraphMap::new();

        // Add all variables being assigned to as nodes
        for &var_id in assignments.keys() {
            dependency.add_node(var_id);
        }

        // Add edges for dependencies
        for (&assigned_var, linear) in &assignments {
            for required_var in linear.required_ids() {
                // Add edge from required_var to assigned_var (dependency direction)
                dependency.add_edge(assigned_var, required_var, ());
            }
        }

        // Check if the dependency graph is acyclic
        if algo::is_cyclic_directed(&dependency) {
            // Find a variable that participates in a cycle for error reporting
            // We can use any variable that's part of a strongly connected component
            for &var_id in assignments.keys() {
                return Err(SubstitutionError::CyclicAssignmentDetected { var_id });
            }
            // This should never be reached if assignments is non-empty
            unreachable!("Found cycle but no variables in assignments");
        }

        Ok(Self {
            assignments,
            dependency,
        })
    }

    pub fn is_empty(&self) -> bool {
        self.assignments.is_empty()
    }

    // Get the assignments in a topologically sorted order.
    pub fn sorted_iter(&self) -> impl Iterator<Item = (VariableID, &Function)> {
        // Get topological order of the dependency graph
        let topo_order = algo::toposort(&self.dependency, None)
            .expect("Graph should be acyclic by construction");

        // Create iterator that yields assignments in topological order
        topo_order
            .into_iter()
            .filter_map(move |var_id| self.assignments.get(&var_id).map(|linear| (var_id, linear)))
    }

    pub fn keys(&self) -> impl Iterator<Item = VariableID> + '_ {
        self.assignments.keys().copied()
    }
}

impl Arbitrary for AcyclicAssignments {
    type Parameters = ();
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
        // Generate a random acyclic graph of assignments
        proptest::collection::vec(
            (
                (0..100_u64).prop_map(VariableID::from),
                Function::arbitrary(),
            ),
            0..=10,
        )
        .prop_filter_map("Acyclic", |assignments| {
            AcyclicAssignments::new(assignments).ok()
        })
        .boxed()
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
                (PolynomialBase::single_term(
                    LinearMonomial::Variable(x2_id),
                    Coefficient::try_from(1.0).unwrap(),
                ) + PolynomialBase::from(Coefficient::try_from(1.0).unwrap()))
                .into(),
            ),
            (
                x2_id,
                (PolynomialBase::single_term(
                    LinearMonomial::Variable(x3_id),
                    Coefficient::try_from(1.0).unwrap(),
                ) + PolynomialBase::from(Coefficient::try_from(2.0).unwrap()))
                .into(),
            ),
        ];

        // When substituting this assignment to x1,
        // x1 <- x2 + 1
        // x2 <- x3 + 2
        // and yields x1 = (x3 + 2) + 1 = x3 + 3
        let acyclic_assignments = AcyclicAssignments::new(assignments).unwrap();

        let mut iter = acyclic_assignments.sorted_iter();
        let (id, _) = iter.next().unwrap();
        assert_eq!(id, x1_id);
        let (id, _) = iter.next().unwrap();
        assert_eq!(id, x2_id);
        assert!(iter.next().is_none(), "There should be no more assignments");
    }

    #[test]
    fn test_acyclic_linear_assignments_cyclic() {
        let x1_id = VariableID::from(1);
        let x2_id = VariableID::from(2);

        // Create x1 = x2 + 1, x2 = x1 + 2 (cyclic, should fail)
        let assignments = vec![
            (
                x1_id,
                (PolynomialBase::single_term(
                    LinearMonomial::Variable(x2_id),
                    Coefficient::try_from(1.0).unwrap(),
                ) + PolynomialBase::from(Coefficient::try_from(1.0).unwrap()))
                .into(),
            ),
            (
                x2_id,
                (PolynomialBase::single_term(
                    LinearMonomial::Variable(x1_id),
                    Coefficient::try_from(1.0).unwrap(),
                ) + PolynomialBase::from(Coefficient::try_from(2.0).unwrap()))
                .into(),
            ),
        ];

        let result = AcyclicAssignments::new(assignments);
        assert!(result.is_err());
    }

    #[test]
    fn test_acyclic_linear_assignments_self_reference() {
        let x1_id = VariableID::from(1);

        // Create x1 = x1 + 2 (self-reference, should fail)
        let assignments = vec![(
            x1_id,
            (PolynomialBase::single_term(
                LinearMonomial::Variable(x1_id),
                Coefficient::try_from(1.0).unwrap(),
            ) + PolynomialBase::from(Coefficient::try_from(2.0).unwrap()))
            .into(),
        )];

        let result = AcyclicAssignments::new(assignments);
        assert!(result.is_err());
    }
}
