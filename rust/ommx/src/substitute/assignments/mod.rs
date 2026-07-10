mod logical_memory;

use super::error::SubstitutionError;
use crate::{
    check_self_assignment, decision_variable::VariableID, substitute_acyclic_via_one, v1::State,
    ATol, Evaluate, Function, Substitute, VariableIDSet,
};
use fnv::FnvHashMap;
use petgraph::algo;
use petgraph::prelude::DiGraphMap;
use proptest::prelude::*;

fn build_dependency_graph<'a>(
    assignments: impl IntoIterator<Item = (VariableID, &'a Function)>,
) -> Result<(DiGraphMap<VariableID, ()>, Vec<VariableID>), SubstitutionError> {
    let assignments = assignments.into_iter().collect::<Vec<_>>();
    let assigned_variables = assignments
        .iter()
        .map(|(id, _)| *id)
        .collect::<std::collections::BTreeSet<_>>();
    let mut dependency = DiGraphMap::new();

    for &var_id in &assigned_variables {
        dependency.add_node(var_id);
    }
    for &(assigned_var, function) in &assignments {
        for required_var in function.required_ids() {
            if required_var == assigned_var {
                return Err(SubstitutionError::RecursiveAssignment {
                    var_id: assigned_var,
                });
            }
            dependency.add_edge(assigned_var, required_var, ());
        }
    }

    let topological_order = algo::toposort(&dependency, None)
        .map_err(|_| SubstitutionError::CyclicAssignmentDetected)?
        .into_iter()
        .filter(|var_id| assigned_variables.contains(var_id))
        .collect();
    Ok((dependency, topological_order))
}

/// Represents a set of assignment rules (`VariableID` -> `Function`)
/// that has been validated to be free of any circular dependencies.
#[derive(Debug, Clone, Default)]
pub struct AcyclicAssignments {
    assignments: FnvHashMap<VariableID, Function>,
    // The directed graph representing dependencies between assignments, assigned -> required.
    dependency: DiGraphMap<VariableID, ()>,
    // Topological order of `dependency`, cached at construction time.
    topological_order: Vec<VariableID>,
}

impl AcyclicAssignments {
    pub fn new(
        iter: impl IntoIterator<Item = (VariableID, Function)>,
    ) -> Result<Self, SubstitutionError> {
        let assignments: FnvHashMap<VariableID, Function> = iter.into_iter().collect();
        let (dependency, topological_order) =
            build_dependency_graph(assignments.iter().map(|(&id, function)| (id, function)))?;

        Ok(Self {
            assignments,
            dependency,
            topological_order,
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

    /// Apply an acyclic substitution atomically without cloning unaffected assignments.
    ///
    /// Crate-internal root operations use this storage effect after planning
    /// every other fallible owner-level rewrite. Only assignment functions
    /// whose right-hand sides reference substituted variables are cloned and
    /// rewritten; the dependency graph is rebuilt from borrowed unchanged rows.
    pub(crate) fn substitute_acyclic_in_place_atomic(
        &mut self,
        acyclic: &AcyclicAssignments,
    ) -> Result<(), SubstitutionError> {
        if acyclic.is_empty() {
            return Ok(());
        }
        if self.is_empty() {
            *self = acyclic.clone();
            return Ok(());
        }

        let substituted_variables = acyclic.keys().collect::<std::collections::BTreeSet<_>>();
        let mut replacements = FnvHashMap::default();
        for (&id, function) in &self.assignments {
            // Incoming assignments replace rows with the same ID. Do not
            // evaluate an obsolete right-hand side: besides wasting work, it
            // could fail even though that row is absent from the final value.
            if substituted_variables.contains(&id) {
                continue;
            }
            if !function.required_ids().is_disjoint(&substituted_variables) {
                replacements.insert(id, function.clone().substitute_acyclic(acyclic)?);
            }
        }

        // The consuming implementation normalizes newly inserted assignments
        // through later substitutions when `self` is non-empty. Preserve that
        // representation while cloning only the incoming assignment set.
        let incoming = acyclic
            .assignments
            .iter()
            .map(|(&id, function)| Ok((id, function.clone().substitute_acyclic(acyclic)?)))
            .collect::<Result<FnvHashMap<_, _>, SubstitutionError>>()?;

        let mut final_assignments = Vec::with_capacity(self.assignments.len() + incoming.len());
        for (&id, function) in &self.assignments {
            if incoming.contains_key(&id) {
                continue;
            }
            final_assignments.push((id, replacements.get(&id).unwrap_or(function)));
        }
        final_assignments.extend(incoming.iter().map(|(&id, function)| (id, function)));
        let (dependency, topological_order) = build_dependency_graph(final_assignments)?;

        self.assignments.extend(replacements);
        self.assignments.extend(incoming);
        self.dependency = dependency;
        self.topological_order = topological_order;
        Ok(())
    }

    /// Get the assignments in substitution order (variables that need to be replaced first).
    ///
    /// This order is used when performing substitution operations where assigned
    /// variables that depend on other assigned variables are substituted first.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use ommx::{assign, linear, coeff, AcyclicAssignments};
    /// let assignments = assign! {
    ///     1 <- linear!(2) + linear!(3),  // x1 <- x2 + x3
    ///     4 <- linear!(1) + coeff!(2.0)  // x4 <- x1 + 2
    /// };
    ///
    /// let order: Vec<_> = assignments.substitution_order_iter()
    ///     .map(|(id, _)| id.into_inner())
    ///     .collect();
    ///     /// // x4 comes before x1 in substitution order because x4 has deeper dependencies
    /// assert_eq!(order, vec![4, 1]);
    /// ```
    pub fn substitution_order_iter(&self) -> impl Iterator<Item = (VariableID, &Function)> {
        self.topological_order.iter().copied().map(move |var_id| {
            let function = self
                .assignments
                .get(&var_id)
                .expect("topological_order only contains assigned variables");
            (var_id, function)
        })
    }

    /// Get the assignments in evaluation order (variables that should be evaluated first).
    ///
    /// This order is used when evaluating assignments where variables that are
    /// required by others should be evaluated first.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use ommx::{assign, linear, coeff, AcyclicAssignments, v1::State, ATol, Evaluate};
    /// let assignments = assign! {
    ///     1 <- linear!(2) + linear!(3),  // x1 <- x2 + x3
    ///     4 <- linear!(1) + coeff!(2.0)  // x4 <- x1 + 2
    /// };
    ///
    /// let order: Vec<_> = assignments.evaluation_order_iter()
    ///     .map(|(id, _)| id.into_inner())
    ///     .collect();
    ///     /// // x1 comes before x4 in evaluation order because x1 must be evaluated before x4
    /// assert_eq!(order, vec![1, 4]);
    ///
    /// // When evaluating with state {x2: 1, x3: 2}:
    /// let state = State::from_iter(vec![(2, 1.0), (3, 2.0)]);
    /// let result = assignments.evaluate(&state, ATol::default()).unwrap();
    ///
    /// // First x1 = x2 + x3 = 1 + 2 = 3 is computed
    /// // Then x4 = x1 + 2 = 3 + 2 = 5 is computed
    /// assert_eq!(result.entries[&1], 3.0);
    /// assert_eq!(result.entries[&4], 5.0);
    /// ```
    pub fn evaluation_order_iter(&self) -> impl Iterator<Item = (VariableID, &Function)> {
        self.topological_order
            .iter()
            .copied()
            .rev()
            .map(move |var_id| {
                let function = self
                    .assignments
                    .get(&var_id)
                    .expect("topological_order only contains assigned variables");
                (var_id, function)
            })
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
    type SampledOutput = FnvHashMap<VariableID, crate::Sampled<f64>>;

    fn evaluate(&self, state: &State, atol: ATol) -> crate::Result<Self::Output> {
        let mut extended_state = state.clone();

        // Evaluate assignments in dependency-first order, the reverse of the dependency graph's
        // topological order.
        //
        // When the assignment is x1 <- x2 + x3, x4 <- x1 + 2, and state is {x2: 1, x3: 2},
        // we first evaluate x1 = 3, then x4 = 5. Finally returns extended state {x1: 3, x2: 1, x3: 2, x4: 5}.
        for (var_id, function) in self.evaluation_order_iter() {
            let value = function.evaluate(&extended_state, atol)?;
            if !value.is_finite() {
                return Err(crate::error!(
                    "Assignment for variable {var_id:?} evaluated to non-finite value: {value}"
                ));
            }
            extended_state.entries.insert(var_id.into_inner(), value);
        }
        Ok(extended_state)
    }

    fn partial_evaluate(&mut self, state: &State, atol: ATol) -> crate::Result<()> {
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

    fn evaluate_samples(
        &self,
        samples: &crate::Sampled<State>,
        atol: ATol,
    ) -> crate::Result<Self::SampledOutput> {
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
        // If self is empty, just return a clone of acyclic (the new assignments)
        if self.is_empty() {
            return Ok(acyclic.clone());
        }
        // If acyclic is empty, nothing to substitute
        if acyclic.is_empty() {
            return Ok(self);
        }
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
    use ::approx::assert_abs_diff_eq;
    use std::collections::BTreeSet;

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
            1 <- ((linear!(2) + linear!(4)).unwrap() + coeff!(1.0)).unwrap(),
            3 <- linear!(4) + coeff!(1.0)
        };

        let result = initial.substitute_acyclic(&substitution).unwrap();
        assert_eq!(result.assignments.len(), expected.assignments.len());
        for (var_id, expected_function) in expected.iter() {
            assert_abs_diff_eq!(result.get(var_id).unwrap(), expected_function);
        }
        assert_eq!(
            result.dependency.all_edges().collect::<BTreeSet<_>>(),
            expected.dependency.all_edges().collect::<BTreeSet<_>>()
        );
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
    fn substitute_acyclic_in_place_atomic_matches_consuming_result() {
        let initial = assign! {
            1 <- linear!(2) + linear!(5),
            2 <- linear!(6),
            9 <- linear!(10)
        };
        let substitution = assign! {
            2 <- linear!(3) + coeff!(1.0),
            3 <- linear!(4)
        };
        let expected = initial.clone().substitute_acyclic(&substitution).unwrap();
        let mut actual = initial;

        actual
            .substitute_acyclic_in_place_atomic(&substitution)
            .unwrap();

        assert_eq!(actual, expected);
    }

    #[test]
    fn substitute_acyclic_in_place_atomic_preserves_value_on_error() {
        let huge = crate::Coefficient::try_from(f64::MAX).unwrap();
        let mut initial = AcyclicAssignments::new([(
            VariableID::from(1),
            Function::from((huge * linear!(2)).unwrap()),
        )])
        .unwrap();
        let substitution = assign! {
            2 <- (coeff!(2.0) * linear!(3)).unwrap()
        };
        let before = initial.clone();

        let err = initial
            .substitute_acyclic_in_place_atomic(&substitution)
            .unwrap_err();

        assert!(err.to_string().contains("Coefficient must be finite"));
        assert_eq!(initial, before);
    }

    #[test]
    fn substitute_acyclic_in_place_atomic_skips_overwritten_rhs() {
        let huge = crate::Coefficient::try_from(f64::MAX).unwrap();
        let mut initial = AcyclicAssignments::new([(
            VariableID::from(1),
            Function::from((huge * linear!(2)).unwrap()),
        )])
        .unwrap();
        let substitution = assign! {
            1 <- linear!(3),
            2 <- (coeff!(2.0) * linear!(4)).unwrap()
        };

        initial
            .substitute_acyclic_in_place_atomic(&substitution)
            .unwrap();

        assert_eq!(initial, substitution);
    }

    #[test]
    fn test_evaluate_topological_order() {
        // Test case based on the comment in evaluate method:
        // When the assignment is x1 <- x2 + x3, x4 <- x1 + 2, and state is {x2: 1, x3: 2},
        // we first evaluate x1 = 3, then x4 = 5. Finally returns extended state {x1: 3, x2: 1, x3: 2, x4: 5}.

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

    #[test]
    fn test_topological_order_contains_only_assigned_variables() {
        let assignments = assign! {
            1 <- linear!(2) + linear!(3),
            4 <- linear!(1) + linear!(5)
        };

        assert_eq!(assignments.topological_order.len(), assignments.len());
        assert!(assignments
            .topological_order
            .iter()
            .all(|var_id| assignments.assignments.contains_key(var_id)));
    }

    #[test]
    fn test_evaluate_rejects_non_finite_assignment_value() {
        let assignments = assign! {
            1 <- linear!(2)
        };
        let state = State::from_iter([(2, f64::INFINITY)]);

        let err = assignments.evaluate(&state, ATol::default()).unwrap_err();
        assert!(err.to_string().contains("evaluated to non-finite value"));
    }
}
