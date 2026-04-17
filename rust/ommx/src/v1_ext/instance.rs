use crate::{
    v1::{
        decision_variable::Kind, instance::Sense, Function, Instance, Optimality, Relaxation,
        SampleSet, SampledDecisionVariable, Samples, Solution, State,
    },
    Bound, Bounds, Evaluate, VariableID, VariableIDSet,
};
use anyhow::{bail, Context, Result};
use approx::AbsDiffEq;
use num::Zero;
use std::{
    borrow::Cow,
    collections::{hash_map::Entry as HashMapEntry, BTreeMap, BTreeSet, HashMap, HashSet},
};

impl Instance {
    pub fn objective(&self) -> Cow<'_, Function> {
        match &self.objective {
            Some(f) => Cow::Borrowed(f),
            // Empty function is regarded as zero function
            None => Cow::Owned(Function::zero()),
        }
    }

    pub fn get_bounds(&self) -> Result<Bounds> {
        let mut bounds = Bounds::new();
        for v in &self.decision_variables {
            let id = VariableID::from(v.id);
            if let Some(bound) = &v.bound {
                bounds.insert(id, bound.clone().try_into()?);
            } else if v.kind() == Kind::Binary {
                bounds.insert(id, Bound::new(0.0, 1.0).unwrap());
            } else {
                bounds.insert(id, Bound::default());
            }
        }
        Ok(bounds)
    }

    pub fn check_bound(&self, state: &State, atol: crate::ATol) -> Result<()> {
        let bounds = self.get_bounds()?;
        for (id, value) in state.entries.iter() {
            let id = VariableID::from(*id);
            if let Some(bound) = bounds.get(&id) {
                if !bound.contains(*value, atol) {
                    bail!("Decision variable value out of bound: ID={id}, value={value}, bound={bound}",);
                }
            }
        }
        Ok(())
    }

    pub fn get_kinds(&self) -> HashMap<VariableID, Kind> {
        self.decision_variables
            .iter()
            .map(|dv| (VariableID::from(dv.id), dv.kind()))
            .collect()
    }

    pub fn defined_ids(&self) -> BTreeSet<u64> {
        self.decision_variables
            .iter()
            .map(|dv| dv.id)
            .collect::<BTreeSet<_>>()
    }

    pub fn constraint_ids(&self) -> BTreeSet<u64> {
        self.constraints.iter().map(|c| c.id).collect()
    }

    pub fn removed_constraint_ids(&self) -> BTreeSet<u64> {
        self.removed_constraints
            .iter()
            .filter_map(|c| c.constraint.as_ref().map(|c| c.id))
            .collect()
    }

    /// Execute all validations for this instance
    pub fn validate(&self) -> Result<()> {
        self.validate_decision_variable_ids()?;
        self.validate_constraint_ids()?;
        Ok(())
    }

    /// Validate that all decision variable IDs used in the instance are defined.
    pub fn validate_decision_variable_ids(&self) -> Result<()> {
        let used_ids = self.required_ids();
        let mut defined_ids = VariableIDSet::default();
        for dv in &self.decision_variables {
            if !defined_ids.insert(dv.id.into()) {
                bail!("Duplicated definition of decision variable ID: {}", dv.id);
            }
        }
        if !used_ids.is_subset(&defined_ids) {
            let undefined_ids = used_ids.difference(&defined_ids).collect::<Vec<_>>();
            bail!("Undefined decision variable IDs: {:?}", undefined_ids);
        }
        Ok(())
    }

    /// Test all constraints and removed constraints have unique IDs.
    pub fn validate_constraint_ids(&self) -> Result<()> {
        let mut map = HashSet::new();
        for c in &self.constraints {
            if !map.insert(c.id) {
                bail!("Duplicated constraint ID: {}", c.id);
            }
        }
        for c in &self.removed_constraints {
            if let Some(c) = &c.constraint {
                if !map.insert(c.id) {
                    bail!("Duplicated constraint ID: {}", c.id);
                }
            }
        }
        Ok(())
    }

    pub fn binary_ids(&self) -> VariableIDSet {
        self.decision_variables
            .iter()
            .filter(|dv| dv.kind() == Kind::Binary)
            .map(|dv| dv.id.into())
            .collect()
    }

    pub fn relax_constraint(
        &mut self,
        constraint_id: u64,
        removed_reason: String,
        removed_reason_parameters: HashMap<String, String>,
    ) -> Result<()> {
        let index = self
            .constraints
            .iter()
            .position(|c| c.id == constraint_id)
            .with_context(|| format!("Constraint ID {constraint_id} not found"))?;
        let c = self.constraints.remove(index);
        self.removed_constraints.push(crate::v1::RemovedConstraint {
            constraint: Some(c),
            removed_reason,
            removed_reason_parameters,
        });
        Ok(())
    }

    pub fn restore_constraint(&mut self, constraint_id: u64) -> Result<()> {
        let index = self
            .removed_constraints
            .iter()
            .position(|c| c.constraint.as_ref().is_some_and(|c| c.id == constraint_id))
            .with_context(|| format!("Constraint ID {constraint_id} not found"))?;
        let c = self.removed_constraints.remove(index).constraint.unwrap();
        self.constraints.push(c);
        Ok(())
    }

    /// Convert the instance into a minimization problem.
    ///
    /// This is based on the fact that maximization problem with negative objective function is equivalent to minimization problem.
    pub fn as_minimization_problem(&mut self) {
        if self.sense() == Sense::Minimize {
            return;
        }
        self.sense = Sense::Minimize as i32;
        self.objective = Some(-self.objective().into_owned());
    }

    pub fn as_maximization_problem(&mut self) {
        if self.sense() == Sense::Maximize {
            return;
        }
        self.sense = Sense::Maximize as i32;
        self.objective = Some(-self.objective().into_owned());
    }

    /// Substitute dependent decision variables with given [Function]s.
    pub fn substitute(&mut self, replacement: HashMap<u64, Function>) -> Result<()> {
        if let Some(obj) = self.objective.as_mut() {
            *obj = obj.substitute(&replacement)?;
        }
        for c in &mut self.constraints {
            if let Some(f) = c.function.as_mut() {
                *f = f.substitute(&replacement)?;
            }
        }
        for c in &mut self.removed_constraints {
            if let Some(c) = &mut c.constraint {
                if let Some(f) = c.function.as_mut() {
                    *f = f.substitute(&replacement)?;
                }
            }
        }
        for (_id, f) in self.decision_variable_dependency.iter_mut() {
            *f = f.substitute(&replacement)?;
        }
        self.decision_variable_dependency.extend(replacement);
        Ok(())
    }
}

/// Compare two instances as mathematical programming problems. This does not compare the metadata.
///
/// - This regards `min f` and `max -f` as the same problem.
/// - This cannot compare scaled constraints. For example, `2x + 3y <= 4` and `4x + 6y <= 8` are mathematically same,
///   but this regarded them as different problems.
///
impl AbsDiffEq for Instance {
    type Epsilon = crate::ATol;

    fn default_epsilon() -> Self::Epsilon {
        crate::ATol::default()
    }

    fn abs_diff_eq(&self, other: &Self, epsilon: Self::Epsilon) -> bool {
        let f = self.objective();
        let g = other.objective();
        match (self.sense.try_into(), other.sense.try_into()) {
            (Ok(Sense::Minimize), Ok(Sense::Minimize))
            | (Ok(Sense::Maximize), Ok(Sense::Maximize)) => {
                if !f.abs_diff_eq(&g, epsilon) {
                    return false;
                }
            }
            (Ok(Sense::Minimize), Ok(Sense::Maximize))
            | (Ok(Sense::Maximize), Ok(Sense::Minimize)) => {
                if !f.abs_diff_eq(&-g.as_ref(), epsilon) {
                    return false;
                }
            }
            _ => return false,
        }

        if self.constraints.len() != other.constraints.len() {
            return false;
        }
        // The constraints may not ordered in the same way
        let lhs = self
            .constraints
            .iter()
            .map(|c| (c.id, (c.equality, c.function())))
            .collect::<BTreeMap<_, _>>();
        for c in &other.constraints {
            if let Some((eq, f)) = lhs.get(&c.id) {
                if *eq != c.equality {
                    return false;
                }
                if !f.abs_diff_eq(&c.function(), epsilon) {
                    return false;
                }
            } else {
                return false;
            }
        }
        true
    }
}

impl Evaluate for Instance {
    type Output = Solution;
    type SampledOutput = SampleSet;

    fn evaluate(&self, state: &State, atol: crate::ATol) -> Result<Self::Output> {
        self.check_bound(state, atol)?;
        let mut evaluated_constraints = Vec::new();
        let mut feasible_relaxed = true;
        for c in &self.constraints {
            let c = c.evaluate(state, atol)?;
            // Only check non-removed constraints for feasibility
            if feasible_relaxed {
                feasible_relaxed = c.is_feasible(atol)?;
            }
            evaluated_constraints.push(c);
        }
        let mut feasible = feasible_relaxed;
        for rc in &self.removed_constraints {
            let inner = rc
                .constraint
                .as_ref()
                .context("RemovedConstraint does not contain constraint")?;
            let mut c = inner.evaluate(state, atol)?;
            c.removed_reason = Some(rc.removed_reason.clone());
            c.removed_reason_parameters = rc.removed_reason_parameters.clone();
            if feasible {
                feasible = c.is_feasible(atol)?;
            }
            evaluated_constraints.push(c);
        }

        let objective = self.objective().evaluate(state, atol)?;

        let mut state = state.clone();
        for v in &self.decision_variables {
            if let Some(value) = v.substituted_value {
                state.entries.insert(v.id, value);
            }
        }
        eval_dependencies(&self.decision_variable_dependency, &mut state, atol)?;
        for v in &self.decision_variables {
            if let HashMapEntry::Vacant(e) = state.entries.entry(v.id) {
                let bound: crate::Bound = v.try_into()?;
                e.insert(bound.nearest_to_zero());
            }
        }
        Ok(Solution {
            decision_variables: self.decision_variables.clone(),
            state: Some(state),
            evaluated_constraints,
            feasible_relaxed: Some(feasible_relaxed),
            feasible,
            objective,
            optimality: Optimality::Unspecified.into(),
            relaxation: Relaxation::Unspecified.into(),
            ..Default::default()
        })
    }

    fn partial_evaluate(&mut self, state: &State, atol: crate::ATol) -> Result<()> {
        for v in &mut self.decision_variables {
            if let Some(value) = state.entries.get(&v.id) {
                v.substituted_value = Some(*value);
            }
        }
        if let Some(f) = self.objective.as_mut() {
            f.partial_evaluate(state, atol)?
        }
        for constraints in &mut self.constraints {
            constraints.partial_evaluate(state, atol)?;
        }
        for rc in &mut self.removed_constraints {
            rc.constraint
                .as_mut()
                .context("RemovedConstraint does not contain constraint")?
                .partial_evaluate(state, atol)?;
        }
        for d in self.decision_variable_dependency.values_mut() {
            d.partial_evaluate(state, atol)?;
        }
        Ok(())
    }

    fn evaluate_samples(
        &self,
        samples: &Samples,
        atol: crate::ATol,
    ) -> Result<Self::SampledOutput> {
        let mut feasible_relaxed: HashMap<u64, bool> =
            samples.ids().map(|id| (*id, true)).collect();

        // Constraints
        let mut constraints = Vec::new();
        for c in &self.constraints {
            let evaluated = c.evaluate_samples(samples, atol)?;
            for (sample_id, feasible_) in evaluated.is_feasible(atol)? {
                if !feasible_ {
                    feasible_relaxed.insert(sample_id, false);
                }
            }
            constraints.push(evaluated);
        }
        let mut feasible = feasible_relaxed.clone();
        for rc in &self.removed_constraints {
            let inner = rc
                .constraint
                .as_ref()
                .context("RemovedConstraint does not contain constraint")?;
            let mut v = inner.evaluate_samples(samples, atol)?;
            v.removed_reason = Some(rc.removed_reason.clone());
            v.removed_reason_parameters = rc.removed_reason_parameters.clone();
            for (sample_id, feasible_) in v.is_feasible(atol)? {
                if !feasible_ {
                    feasible.insert(sample_id, false);
                }
            }
            constraints.push(v);
        }

        // Objective
        let objectives = self.objective().evaluate_samples(samples, atol)?;

        // Reconstruct decision variable values
        let mut samples = samples.clone();
        for state in samples.states_mut() {
            eval_dependencies(&self.decision_variable_dependency, state?, atol)?;
        }
        let mut transposed = samples.transpose();
        let decision_variables: Vec<SampledDecisionVariable> = self
            .decision_variables
            .iter()
            .map(|d| -> Result<_> {
                Ok(SampledDecisionVariable {
                    decision_variable: Some(d.clone()),
                    samples: transposed.remove(&d.id),
                })
            })
            .collect::<Result<_>>()?;

        Ok(SampleSet {
            decision_variables,
            objectives: Some(objectives),
            constraints,
            feasible_relaxed,
            feasible,
            sense: self.sense,
            ..Default::default()
        })
    }

    fn required_ids(&self) -> VariableIDSet {
        let mut used_ids = self.objective().required_ids();
        for c in &self.constraints {
            used_ids.extend(c.function().required_ids());
        }
        for c in &self.removed_constraints {
            if let Some(c) = &c.constraint {
                used_ids.extend(c.function().required_ids());
            }
        }
        used_ids
    }
}

// FIXME: This would be better by using a topological sort
fn eval_dependencies(
    dependencies: &HashMap<u64, Function>,
    state: &mut State,
    atol: crate::ATol,
) -> Result<()> {
    let mut bucket: Vec<_> = dependencies.iter().collect();
    let mut last_size = bucket.len();
    let mut not_evaluated = Vec::new();
    loop {
        while let Some((id, f)) = bucket.pop() {
            match f.evaluate(state, atol) {
                Ok(value) => {
                    state.entries.insert(*id, value);
                }
                Err(_) => {
                    not_evaluated.push((id, f));
                }
            }
        }
        if not_evaluated.is_empty() {
            return Ok(());
        }
        if last_size == not_evaluated.len() {
            bail!("Cannot evaluate any dependent variables.");
        }
        last_size = not_evaluated.len();
        bucket.append(&mut not_evaluated);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        v1::{Linear, State},
        Evaluate,
    };
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn test_instance_arbitrary_any(instance in Instance::arbitrary()) {
            instance.validate().unwrap();
        }

        /// Compare the result of partial_evaluate and substitute with `Function::Constant`.
        #[test]
        fn substitute_fixed_value(instance in Instance::arbitrary(), value in -3.0..3.0) {
            for id in instance.defined_ids() {
                let mut partially_evaluated = instance.clone();
                partially_evaluated.partial_evaluate(&State { entries: [(id, value)].into_iter().collect() }, crate::ATol::default()).unwrap();
                let mut substituted = instance.clone();
                substituted.substitute([(id, Function::from(value))].into_iter().collect()).unwrap();
                prop_assert!(partially_evaluated.abs_diff_eq(&substituted, crate::ATol::default()));
            }
        }
    }

    #[test]
    fn test_eval_dependencies() {
        let mut state = State::from_iter(vec![(1, 1.0), (2, 2.0), (3, 3.0)]);
        let dependencies = [
            (
                4,
                Function::from(Linear::new([(1, 1.0), (2, 2.0)].into_iter(), 0.0)),
            ),
            (
                5,
                Function::from(Linear::new([(4, 1.0), (3, 3.0)].into_iter(), 0.0)),
            ),
        ]
        .into_iter()
        .collect();
        eval_dependencies(&dependencies, &mut state, crate::ATol::default()).unwrap();
        assert_eq!(state.entries[&4], 1.0 + 2.0 * 2.0);
        assert_eq!(state.entries[&5], 1.0 + 2.0 * 2.0 + 3.0 * 3.0);

        // circular dependency
        let mut state = State::from_iter(vec![(1, 1.0), (2, 2.0), (3, 3.0)]);
        let dependencies = [
            (
                4,
                Function::from(Linear::new([(1, 1.0), (5, 2.0)].into_iter(), 0.0)),
            ),
            (
                5,
                Function::from(Linear::new([(4, 1.0), (3, 3.0)].into_iter(), 0.0)),
            ),
        ]
        .into_iter()
        .collect();
        assert!(eval_dependencies(&dependencies, &mut state, crate::ATol::default()).is_err());

        // non-existing dependency
        let mut state = State::from_iter(vec![(1, 1.0), (2, 2.0), (3, 3.0)]);
        let dependencies = [
            (
                4,
                Function::from(Linear::new([(1, 1.0), (6, 2.0)].into_iter(), 0.0)),
            ),
            (
                5,
                Function::from(Linear::new([(4, 1.0), (3, 3.0)].into_iter(), 0.0)),
            ),
        ]
        .into_iter()
        .collect();
        assert!(eval_dependencies(&dependencies, &mut state, crate::ATol::default()).is_err());
    }
}
