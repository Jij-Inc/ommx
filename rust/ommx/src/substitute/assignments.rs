use super::error::SubstitutionError;
use crate::{decision_variable::VariableID, Evaluate, Function};
use fnv::FnvHashMap;
use petgraph::algo;
use petgraph::prelude::DiGraphMap;
use proptest::prelude::*;

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
                if required_var == assigned_var {
                    return Err(SubstitutionError::RecursiveAssignment {
                        var_id: assigned_var,
                    });
                }
                // Add edge from assigned variable to required variable
                // to keep the order of topological sorting correct
                dependency.add_edge(assigned_var, required_var, ());
            }
        }

        // Check if the dependency graph is acyclic
        if algo::is_cyclic_directed(&dependency) {
            return Err(SubstitutionError::CyclicAssignmentDetected);
        }

        Ok(Self {
            assignments,
            dependency,
        })
    }

    pub fn is_empty(&self) -> bool {
        self.assignments.is_empty()
    }

    pub fn len(&self) -> usize {
        self.assignments.len()
    }

    pub fn get(&self, id: &VariableID) -> Option<&Function> {
        self.assignments.get(id)
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

impl PartialEq for AcyclicAssignments {
    fn eq(&self, other: &Self) -> bool {
        // First check if assignments are equal
        if self.assignments != other.assignments {
            return false;
        }

        // Check if dependency graphs have the same nodes
        let self_nodes: std::collections::BTreeSet<_> = self.dependency.nodes().collect();
        let other_nodes: std::collections::BTreeSet<_> = other.dependency.nodes().collect();
        if self_nodes != other_nodes {
            return false;
        }

        // Check if dependency graphs have the same edges
        let self_edges: std::collections::BTreeSet<_> = self.dependency.all_edges().collect();
        let other_edges: std::collections::BTreeSet<_> = other.dependency.all_edges().collect();
        self_edges == other_edges
    }
}

impl Arbitrary for AcyclicAssignments {
    type Parameters = AcyclicAssignmentsParameters;
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(p: Self::Parameters) -> Self::Strategy {
        // Generate a random acyclic graph of assignments
        proptest::collection::vec(
            (
                (0..=p.function_parameters.max_id().into_inner()).prop_map(VariableID::from),
                Function::arbitrary_with(p.function_parameters),
            ),
            0..=p.max_assignments,
        )
        .prop_filter_map("Acyclic", |assignments| {
            AcyclicAssignments::new(assignments).ok()
        })
        .boxed()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct AcyclicAssignmentsParameters {
    pub max_assignments: usize,
    pub function_parameters: <Function as Arbitrary>::Parameters,
}

impl Default for AcyclicAssignmentsParameters {
    fn default() -> Self {
        Self {
            max_assignments: 10,
            function_parameters: <Function as Arbitrary>::Parameters::default(),
        }
    }
}
