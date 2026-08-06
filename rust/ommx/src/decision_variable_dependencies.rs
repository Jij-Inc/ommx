use crate::{
    check_self_assignment,
    logical_memory::{LogicalMemoryProfile, LogicalMemoryVisitor, Path},
    substitute_acyclic_via_one,
    v1::State,
    ATol, AcyclicAssignments, DependentExpr, Evaluate, Function, Sampled, Substitute,
    SubstitutionError, VariableID, VariableIDSet,
};
use fnv::FnvHashMap;
use petgraph::{algo, prelude::DiGraphMap};
use std::{collections::BTreeSet, mem::size_of};

fn build_dependency_graph<'a>(
    dependencies: impl IntoIterator<Item = (VariableID, &'a DependentExpr)>,
) -> Result<(DiGraphMap<VariableID, ()>, Vec<VariableID>), SubstitutionError> {
    let dependencies = dependencies.into_iter().collect::<Vec<_>>();
    let dependent_variables = dependencies
        .iter()
        .map(|(id, _)| *id)
        .collect::<BTreeSet<_>>();
    let mut graph = DiGraphMap::new();

    for &id in &dependent_variables {
        graph.add_node(id);
    }
    for &(dependent, expression) in &dependencies {
        for required in expression.required_ids() {
            if required == dependent {
                return Err(SubstitutionError::RecursiveAssignment { var_id: dependent });
            }
            graph.add_edge(dependent, required, ());
        }
    }

    let topological_order = algo::toposort(&graph, None)
        .map_err(|_| SubstitutionError::CyclicAssignmentDetected)?
        .into_iter()
        .filter(|id| dependent_variables.contains(id))
        .collect();
    Ok((graph, topological_order))
}

/// An acyclic set of deterministic decision-variable reconstruction rules.
///
/// Each key is a dependent decision variable and each value is the
/// [`DependentExpr`] that reconstructs it. The constructor rejects direct or
/// indirect cycles and caches a dependency-first evaluation order. Callers
/// cannot mutate individual entries; changes are rebuilt through
/// [`Self::new`] or algebraic [`Substitute`] operations so the acyclicity
/// invariant remains true.
///
/// This type owns post-solve reconstruction only. [`AcyclicAssignments`]
/// remains the algebraic, [`Function`]-valued substitution plan used to rewrite
/// optimization-model functions.
#[derive(Debug, Clone, Default)]
pub struct DecisionVariableDependencies {
    dependencies: FnvHashMap<VariableID, DependentExpr>,
    // Edges point from each dependent variable to every variable required by
    // its expression, including variables outside this dependency set.
    graph: DiGraphMap<VariableID, ()>,
    // Cached topological order in dependent-before-requirement direction.
    topological_order: Vec<VariableID>,
}

impl DecisionVariableDependencies {
    /// Validate and construct a dependency DAG.
    pub fn new(
        dependencies: impl IntoIterator<Item = (VariableID, DependentExpr)>,
    ) -> Result<Self, SubstitutionError> {
        let dependencies = dependencies.into_iter().collect::<FnvHashMap<_, _>>();
        let (graph, topological_order) =
            build_dependency_graph(dependencies.iter().map(|(&id, expr)| (id, expr)))?;
        Ok(Self {
            dependencies,
            graph,
            topological_order,
        })
    }

    /// Return whether no dependent variables are registered.
    pub fn is_empty(&self) -> bool {
        self.dependencies.is_empty()
    }

    /// Return the number of dependent variables.
    pub fn len(&self) -> usize {
        self.dependencies.len()
    }

    /// Get the reconstruction expression for a dependent variable.
    pub fn get(&self, id: &VariableID) -> Option<&DependentExpr> {
        self.dependencies.get(id)
    }

    /// Iterate over dependent-variable IDs and their reconstruction expressions.
    pub fn iter(&self) -> impl Iterator<Item = (&VariableID, &DependentExpr)> {
        self.dependencies.iter()
    }

    /// Iterate over dependent-variable IDs.
    pub fn keys(&self) -> impl Iterator<Item = VariableID> + '_ {
        self.dependencies.keys().copied()
    }

    /// Return the union of IDs referenced by all reconstruction expressions.
    ///
    /// This intentionally includes IDs that are themselves keys in this DAG,
    /// matching the [`Evaluate::required_ids`] behavior of
    /// [`AcyclicAssignments`]. Use [`Self::keys`] to distinguish internally
    /// reconstructed coordinates from external inputs.
    pub fn required_ids(&self) -> VariableIDSet {
        self.dependencies
            .values()
            .flat_map(Evaluate::required_ids)
            .collect()
    }

    /// Iterate in dependency-first evaluation order.
    ///
    /// If `x1` depends on `x2`, `x2` is yielded before `x1` so inserting each
    /// computed value into an evaluation state makes the next expression ready.
    pub fn evaluation_order_iter(&self) -> impl Iterator<Item = (VariableID, &DependentExpr)> {
        self.topological_order.iter().copied().rev().map(move |id| {
            let expression = self
                .dependencies
                .get(&id)
                .expect("topological_order only contains dependent variables");
            (id, expression)
        })
    }
}

impl PartialEq for DecisionVariableDependencies {
    fn eq(&self, other: &Self) -> bool {
        if self.dependencies != other.dependencies {
            return false;
        }
        let self_nodes = self.graph.nodes().collect::<BTreeSet<_>>();
        let other_nodes = other.graph.nodes().collect::<BTreeSet<_>>();
        if self_nodes != other_nodes {
            return false;
        }
        let self_edges = self.graph.all_edges().collect::<BTreeSet<_>>();
        let other_edges = other.graph.all_edges().collect::<BTreeSet<_>>();
        self_edges == other_edges
    }
}

impl IntoIterator for DecisionVariableDependencies {
    type Item = (VariableID, DependentExpr);
    type IntoIter = <FnvHashMap<VariableID, DependentExpr> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.dependencies.into_iter()
    }
}

impl From<AcyclicAssignments> for DecisionVariableDependencies {
    fn from(assignments: AcyclicAssignments) -> Self {
        Self::new(
            assignments
                .into_iter()
                .map(|(id, function)| (id, DependentExpr::from(function))),
        )
        .expect("a Function-valued acyclic assignment remains acyclic as DependentExpr")
    }
}

impl Evaluate for DecisionVariableDependencies {
    type Output = State;
    type SampledOutput = FnvHashMap<VariableID, Sampled<f64>>;

    fn evaluate(&self, state: &State, atol: ATol) -> crate::Result<Self::Output> {
        let mut extended = state.clone();
        for (id, expression) in self.evaluation_order_iter() {
            let value = expression.evaluate(&extended, atol).inspect_err(|error| {
                tracing::error!(?id, error = %error, "failed to evaluate dependent variable");
            })?;
            if !value.is_finite() {
                crate::bail!(
                    { id = ?id, value },
                    "dependent variable {id:?} evaluated to a non-finite value: {value}"
                );
            }
            extended.entries.insert(id.into_inner(), value);
        }
        Ok(extended)
    }

    fn evaluate_samples(
        &self,
        samples: &Sampled<State>,
        atol: ATol,
    ) -> crate::Result<Self::SampledOutput> {
        // Evaluate each unique source state once. `try_map_ref` preserves the
        // input SampleID grouping while every DAG evaluation extends its own
        // state in dependency-first order.
        let extended = samples.try_map_ref(|state| self.evaluate(state, atol))?;
        let mut output = FnvHashMap::default();
        for id in self.keys() {
            let values = extended.try_map_ref(|state| {
                state.entries.get(&id.into_inner()).copied().ok_or_else(|| {
                    crate::error!(
                        { id = ?id },
                        "evaluated dependency state is missing variable {id:?}"
                    )
                })
            })?;
            output.insert(id, values);
        }
        Ok(output)
    }

    fn partial_evaluate(&mut self, state: &State, atol: ATol) -> crate::Result<()> {
        let mut updated = Vec::with_capacity(self.dependencies.len());
        for (&id, expression) in &self.dependencies {
            let mut expression = expression.clone();
            expression.partial_evaluate(state, atol)?;
            updated.push((id, expression));
        }
        *self = Self::new(updated)?;
        Ok(())
    }

    fn required_ids(&self) -> VariableIDSet {
        Self::required_ids(self)
    }
}

impl Substitute for DecisionVariableDependencies {
    type Output = Self;

    fn substitute_acyclic(
        self,
        acyclic: &AcyclicAssignments,
    ) -> Result<Self::Output, SubstitutionError> {
        if self.is_empty() {
            return Ok(acyclic.clone().into());
        }
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
        let mut updated = Vec::with_capacity(self.dependencies.len() + 1);
        for (id, expression) in self.dependencies {
            if id != assigned {
                updated.push((id, expression.substitute_one(assigned, function)?));
            }
        }
        updated.push((assigned, DependentExpr::from(function.clone())));
        Self::new(updated)
    }
}

impl LogicalMemoryProfile for DecisionVariableDependencies {
    fn visit_logical_memory<V: LogicalMemoryVisitor>(&self, path: &mut Path, visitor: &mut V) {
        self.dependencies.visit_logical_memory(
            path.with("DecisionVariableDependencies.dependencies")
                .as_mut(),
            visitor,
        );

        let graph_overhead = size_of::<DiGraphMap<VariableID, ()>>();
        let node_bytes = self.graph.node_count() * size_of::<VariableID>();
        let edge_bytes = self.graph.edge_count() * (size_of::<VariableID>() * 2);
        visitor.visit_leaf(
            &path.with("DecisionVariableDependencies.graph"),
            graph_overhead + node_bytes + edge_bytes,
        );

        self.topological_order.visit_logical_memory(
            path.with("DecisionVariableDependencies.topological_order")
                .as_mut(),
            visitor,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{coeff, linear, SampleID};

    fn state(entries: impl IntoIterator<Item = (u64, f64)>) -> State {
        entries.into_iter().collect()
    }

    fn chained_dependencies() -> DecisionVariableDependencies {
        DecisionVariableDependencies::new([
            (
                VariableID::from(1),
                DependentExpr::nonzero_indicator(Function::from(linear!(2))),
            ),
            (
                VariableID::from(2),
                DependentExpr::from(Function::from(linear!(3))),
            ),
        ])
        .unwrap()
    }

    #[test]
    fn rejects_recursive_and_cyclic_dependencies() {
        let recursive = DecisionVariableDependencies::new([(
            VariableID::from(1),
            DependentExpr::from(Function::from(linear!(1))),
        )])
        .unwrap_err();
        assert!(matches!(
            recursive,
            SubstitutionError::RecursiveAssignment { var_id }
                if var_id == VariableID::from(1)
        ));

        let cyclic = DecisionVariableDependencies::new([
            (
                VariableID::from(1),
                DependentExpr::from(Function::from(linear!(2))),
            ),
            (
                VariableID::from(2),
                DependentExpr::from(Function::from(linear!(1))),
            ),
        ])
        .unwrap_err();
        assert!(matches!(
            cyclic,
            SubstitutionError::CyclicAssignmentDetected
        ));
    }

    #[test]
    fn evaluates_in_dependency_first_order() {
        let dependencies = chained_dependencies();
        let order = dependencies
            .evaluation_order_iter()
            .map(|(id, _)| id)
            .collect::<Vec<_>>();
        assert_eq!(order, vec![VariableID::from(2), VariableID::from(1)]);

        let zero = dependencies
            .evaluate(&state([(3, 0.0)]), ATol::default())
            .unwrap();
        assert_eq!(zero.entries[&2], 0.0);
        assert_eq!(zero.entries[&1], 0.0);

        let atol = ATol::new(1.0e-6).unwrap();
        let near_zero = dependencies.evaluate(&state([(3, 5.0e-7)]), atol).unwrap();
        assert_eq!(near_zero.entries[&2], 5.0e-7);
        assert_eq!(near_zero.entries[&1], 0.0);

        let nonzero = dependencies.evaluate(&state([(3, 1.0e-6)]), atol).unwrap();
        assert_eq!(nonzero.entries[&2], 1.0e-6);
        assert_eq!(nonzero.entries[&1], 1.0);
    }

    #[test]
    fn rejects_non_finite_reconstructed_values() {
        let dependencies = DecisionVariableDependencies::new([(
            VariableID::from(1),
            DependentExpr::from(Function::from(linear!(2))),
        )])
        .unwrap();

        let error = dependencies
            .evaluate(&state([(2, f64::INFINITY)]), ATol::default())
            .unwrap_err();

        assert!(error.to_string().contains("non-finite"));
    }

    #[test]
    fn required_ids_includes_internal_dependency_references() {
        let dependencies = chained_dependencies();
        assert_eq!(
            dependencies.required_ids(),
            VariableIDSet::from([VariableID::from(2), VariableID::from(3)])
        );
        assert_eq!(
            dependencies.keys().collect::<VariableIDSet>(),
            VariableIDSet::from([VariableID::from(1), VariableID::from(2)])
        );
    }

    #[test]
    fn evaluate_samples_resolves_dag_and_preserves_groups() {
        let dependencies = chained_dependencies();
        let samples = Sampled::new(
            vec![
                vec![SampleID::from(10), SampleID::from(11)],
                vec![SampleID::from(20)],
            ],
            [state([(3, 0.0)]), state([(3, 2.0)])],
        )
        .unwrap();

        let evaluated = dependencies
            .evaluate_samples(&samples, ATol::default())
            .unwrap();

        assert_eq!(evaluated[&VariableID::from(1)].num_samples(), 3);
        assert_eq!(
            evaluated[&VariableID::from(1)].get(SampleID::from(10)),
            Some(&0.0)
        );
        assert_eq!(
            evaluated[&VariableID::from(1)].get(SampleID::from(11)),
            Some(&0.0)
        );
        assert_eq!(
            evaluated[&VariableID::from(1)].get(SampleID::from(20)),
            Some(&1.0)
        );
        assert_eq!(
            evaluated[&VariableID::from(2)].get(SampleID::from(20)),
            Some(&2.0)
        );
    }

    #[test]
    fn partial_evaluate_rebuilds_the_validated_dag() {
        let mut dependencies = DecisionVariableDependencies::new([(
            VariableID::from(1),
            DependentExpr::nonzero_indicator(Function::from((linear!(2) + linear!(3)).unwrap())),
        )])
        .unwrap();

        dependencies
            .partial_evaluate(&state([(3, -1.0)]), ATol::default())
            .unwrap();
        assert_eq!(dependencies.required_ids(), [VariableID::from(2)].into());

        dependencies
            .partial_evaluate(&state([(2, 1.0)]), ATol::default())
            .unwrap();
        assert!(dependencies.required_ids().is_empty());
        assert_eq!(
            dependencies
                .evaluate(&State::default(), ATol::default())
                .unwrap()
                .entries[&1],
            0.0
        );
    }

    #[test]
    fn structural_substitution_preserves_non_polynomial_nodes() {
        let dependencies = DecisionVariableDependencies::new([(
            VariableID::from(10),
            DependentExpr::nonzero_indicator(Function::from((linear!(1) + coeff!(1.0)).unwrap())),
        )])
        .unwrap();
        let assignments = AcyclicAssignments::new([(
            VariableID::from(1),
            Function::from((linear!(2) - coeff!(1.0)).unwrap()),
        )])
        .unwrap();

        let substituted = dependencies.substitute_acyclic(&assignments).unwrap();

        assert_eq!(
            substituted.keys().collect::<VariableIDSet>(),
            VariableIDSet::from([VariableID::from(1), VariableID::from(10)])
        );
        assert!(matches!(
            substituted.get(&VariableID::from(10)),
            Some(DependentExpr::NonzeroIndicator(_))
        ));
        let evaluated = substituted
            .evaluate(&state([(2, 0.0)]), ATol::default())
            .unwrap();
        assert_eq!(evaluated.entries[&1], -1.0);
        assert_eq!(evaluated.entries[&10], 0.0);
    }

    #[test]
    fn converts_function_assignments_without_changing_semantics() {
        let assignments = AcyclicAssignments::new([
            (
                VariableID::from(1),
                Function::from((linear!(2) + coeff!(1.0)).unwrap()),
            ),
            (
                VariableID::from(2),
                Function::from((linear!(3) + coeff!(2.0)).unwrap()),
            ),
        ])
        .unwrap();
        let expected = assignments
            .evaluate(&state([(3, 4.0)]), ATol::default())
            .unwrap();

        let dependencies = DecisionVariableDependencies::from(assignments);
        let actual = dependencies
            .evaluate(&state([(3, 4.0)]), ATol::default())
            .unwrap();

        assert_eq!(actual, expected);
    }
}
