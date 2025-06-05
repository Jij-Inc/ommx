use super::error::SubstitutionError;
use crate::{
    check_self_assignment,
    decision_variable::VariableID,
    substitute_acyclic_via_one,
    v1::{Samples, State},
    ATol, Evaluate, Function, Substitute, VariableIDSet,
};
use anyhow::Result;
use fnv::FnvHashMap;
use petgraph::algo;
use petgraph::prelude::DiGraphMap;
use proptest::prelude::*;

/// Represents a set of assignment rules (`VariableID` -> `Function`)
/// that has been validated to be free of any circular dependencies.
#[derive(Debug, Clone, Default)]
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

    pub fn iter(&self) -> impl Iterator<Item = (&VariableID, &Function)> {
        self.assignments.iter()
    }

    fn sorted_ids(&self) -> Vec<VariableID> {
        algo::toposort(&self.dependency, None).expect("Graph should be acyclic by construction")
    }

    // Get the assignments in a topologically sorted order.
    pub fn substitution_order_iter(&self) -> impl Iterator<Item = (VariableID, &Function)> {
        self.sorted_ids()
            .into_iter()
            .filter_map(move |var_id| self.assignments.get(&var_id).map(|linear| (var_id, linear)))
    }

    pub fn evaluation_order_iter(&self) -> impl Iterator<Item = (VariableID, &Function)> {
        self.sorted_ids()
            .into_iter()
            .rev()
            .filter_map(move |var_id| self.assignments.get(&var_id).map(|linear| (var_id, linear)))
    }

    pub fn keys(&self) -> impl Iterator<Item = VariableID> + '_ {
        self.assignments.keys().copied()
    }

    /// Merge another `AcyclicAssignments` into this one.
    /// Returns an error if the merge would create a cyclic dependency.
    pub fn merge(&mut self, other: AcyclicAssignments) -> Result<(), SubstitutionError> {
        let current = std::mem::take(&mut self.assignments);
        *self = Self::new(current.into_iter().chain(other.assignments.into_iter()))?;
        Ok(())
    }

    /// Create a new `AcyclicAssignments` by merging two existing ones.
    /// Returns an error if the merge would create a cyclic dependency.
    pub fn merged(mut self, other: AcyclicAssignments) -> Result<Self, SubstitutionError> {
        self.merge(other)?;
        Ok(self)
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

impl IntoIterator for AcyclicAssignments {
    type Item = (VariableID, Function);
    type IntoIter = <FnvHashMap<VariableID, Function> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.assignments.into_iter()
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

impl Evaluate for AcyclicAssignments {
    type Output = State;
    type SampledOutput = FnvHashMap<VariableID, crate::v1::SampledValues>;

    fn evaluate(&self, state: &State, atol: ATol) -> Result<Self::Output> {
        let mut extended_state = state.clone();

        // Evaluate assignments in topological order
        //
        // When the assignment is x1 <- x2 + x3, x4 <- x1 + 2, and state is {x2: 1, x3: 2},
        // we first evaluate x1 = 3, then x4 = 4. Finally returns extended state {x1: 3, x2: 1, x3: 2, x4: 4}.
        for (var_id, function) in self.evaluation_order_iter() {
            let value = function.evaluate(&extended_state, atol)?;
            extended_state.entries.insert(var_id.into_inner(), value);
        }
        Ok(extended_state)
    }

    fn partial_evaluate(&mut self, state: &State, atol: ATol) -> Result<()> {
        // Create new assignments with partial evaluation applied
        let mut new_assignments = Vec::new();

        for (var_id, function) in self.assignments.iter() {
            let mut function_clone = function.clone();
            function_clone.partial_evaluate(state, atol)?;
            new_assignments.push((*var_id, function_clone));
        }

        // Rebuild using new method to ensure acyclicity is maintained
        *self = Self::new(new_assignments)?;
        Ok(())
    }

    fn evaluate_samples(&self, samples: &Samples, atol: ATol) -> Result<Self::SampledOutput> {
        let mut result = FnvHashMap::default();

        // For each assignment in topological order
        for (var_id, function) in self.substitution_order_iter() {
            let sampled_values = function.evaluate_samples(samples, atol)?;
            result.insert(var_id, sampled_values);
        }

        Ok(result)
    }

    fn required_ids(&self) -> VariableIDSet {
        self.assignments
            .values()
            .flat_map(|function| function.required_ids())
            .collect()
    }
}

impl Substitute for AcyclicAssignments {
    type Output = Self;

    fn substitute_acyclic(
        self,
        acyclic: &crate::AcyclicAssignments,
    ) -> Result<Self::Output, crate::SubstitutionError> {
        substitute_acyclic_via_one(self, acyclic)
    }

    fn substitute_one(
        self,
        assigned: VariableID,
        function: &Function,
    ) -> Result<Self::Output, SubstitutionError> {
        check_self_assignment(assigned, function)?;
        // Apply substitution to each assignment function
        let mut new_assignments = Vec::new();
        for (var_id, func) in self.assignments {
            let substituted_func = func.substitute_one(assigned, function)?;
            new_assignments.push((var_id, substituted_func));
        }
        new_assignments.push((assigned, function.clone()));

        // Create new AcyclicAssignments with substituted functions
        // This will rebuild the dependency graph and check for cycles
        Self::new(new_assignments)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{assign, coeff, linear};

    #[test]
    fn test_substitute_acyclic_success() {
        // Create initial assignments: x1 <- x2 + x3
        let initial = assign! {
            1 <- linear!(2) + linear!(3)
        };

        // Substitute x3 <- x4 + 1
        let substitution = assign! {
            3 <- linear!(4) + coeff!(1.0)
        };

        // Expected result: x1 <- x2 + x4 + 1, x3 <- x4 + 1
        let expected = assign! {
            1 <- linear!(2) + linear!(4) + coeff!(1.0),
            3 <- linear!(4) + coeff!(1.0)
        };

        let result = initial.substitute_acyclic(&substitution).unwrap();
        assert_eq!(result, expected);
    }

    #[test]
    fn test_substitute_acyclic_fail() {
        // Create initial assignments: x1 <- x2 + x3
        let initial = assign! {
            1 <- linear!(2) + linear!(3)
        };

        // Substitute x3 <- x1 + 1
        // This causes a cyclic dependency
        let substitution = assign! {
            3 <- linear!(1) + coeff!(1.0)
        };

        insta::assert_snapshot!(
            initial.substitute_acyclic(&substitution).unwrap_err(),
            @"Recursive assignment detected: variable 1 cannot be assigned to a function that depends on itself"
        );
    }

    #[test]
    fn test_evaluate_topological_order() {
        // Test case based on the comment in evaluate method:
        // When the assignment is x1 <- x2 + x3, x4 <- x1 + 2, and state is {x2: 1, x3: 2},
        // we first evaluate x1 = 3, then x4 = 4. Finally returns extended state {x1: 3, x2: 1, x3: 2, x4: 4}.

        let assignments = assign! {
            1 <- linear!(2) + linear!(3),  // x1 <- x2 + x3
            4 <- linear!(1) + coeff!(2.0)  // x4 <- x1 + 2
        };

        let state = State::from_iter(vec![(2, 1.0), (3, 2.0)]); // {x2: 1, x3: 2}

        let result = assignments.evaluate(&state, ATol::default()).unwrap();

        // Expected extended state: {x1: 3, x2: 1, x3: 2, x4: 4}
        assert_eq!(result.entries[&1], 3.0); // x1 = x2 + x3 = 1 + 2 = 3
        assert_eq!(result.entries[&2], 1.0); // x2 = 1 (original)
        assert_eq!(result.entries[&3], 2.0); // x3 = 2 (original)
        assert_eq!(result.entries[&4], 5.0); // x4 = x1 + 2 = 3 + 2 = 5
    }
}
