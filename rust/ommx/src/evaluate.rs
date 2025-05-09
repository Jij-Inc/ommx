use crate::v1::{
    function::Function as FunctionEnum, linear::Term as LinearTerm, Constraint, Equality,
    EvaluatedConstraint, Function, Instance, Linear, Monomial, Optimality, Polynomial, Quadratic,
    Relaxation, RemovedConstraint, SampleSet, SampledConstraint, SampledDecisionVariable,
    SampledValues, Samples, Solution, State,
};
use anyhow::{bail, ensure, Context, Result};
use std::collections::{hash_map::Entry as HashMapEntry, BTreeMap, BTreeSet, HashMap};

/// Evaluate with a [State]
pub trait Evaluate {
    type Output;
    type SampledOutput;

    /// Evaluate to return the output with used variable ids
    fn evaluate(&self, solution: &State) -> Result<Self::Output>;

    /// Partially evaluate the function to return the used variable ids
    fn partial_evaluate(&mut self, state: &State) -> Result<()>;

    /// Evaluate for each sample
    fn evaluate_samples(&self, samples: &Samples) -> Result<Self::SampledOutput>;

    /// Decision variable IDs required for evaluation
    fn required_ids(&self) -> BTreeSet<u64>;
}

impl Evaluate for Function {
    type Output = f64;
    type SampledOutput = SampledValues;

    fn evaluate(&self, solution: &State) -> Result<f64> {
        let out = match &self.function {
            Some(FunctionEnum::Constant(c)) => *c,
            Some(FunctionEnum::Linear(linear)) => linear.evaluate(solution)?,
            Some(FunctionEnum::Quadratic(quadratic)) => quadratic.evaluate(solution)?,
            Some(FunctionEnum::Polynomial(poly)) => poly.evaluate(solution)?,
            None => 0.0,
        };
        Ok(out)
    }

    fn partial_evaluate(&mut self, state: &State) -> Result<()> {
        match &mut self.function {
            Some(FunctionEnum::Linear(linear)) => linear.partial_evaluate(state)?,
            Some(FunctionEnum::Quadratic(quadratic)) => quadratic.partial_evaluate(state)?,
            Some(FunctionEnum::Polynomial(poly)) => poly.partial_evaluate(state)?,
            _ => {}
        };
        Ok(())
    }

    fn evaluate_samples(&self, samples: &Samples) -> Result<Self::SampledOutput> {
        let out = samples.map(|s| {
            let value = self.evaluate(s)?;
            Ok(value)
        })?;
        Ok(out)
    }

    fn required_ids(&self) -> BTreeSet<u64> {
        self.used_decision_variable_ids()
    }
}

impl Evaluate for Linear {
    type Output = f64;
    type SampledOutput = SampledValues;

    fn evaluate(&self, solution: &State) -> Result<f64> {
        let mut sum = self.constant;
        for LinearTerm { id, coefficient } in &self.terms {
            let s = solution
                .entries
                .get(id)
                .with_context(|| format!("Variable id ({id}) is not found in the solution"))?;
            sum += coefficient * s;
        }
        Ok(sum)
    }

    fn partial_evaluate(&mut self, state: &State) -> Result<()> {
        let mut i = 0;
        while i < self.terms.len() {
            let LinearTerm { id, coefficient } = self.terms[i];
            if let Some(value) = state.entries.get(&id) {
                self.constant += coefficient * value;
                self.terms.swap_remove(i);
            } else {
                i += 1;
            }
        }
        Ok(())
    }

    fn evaluate_samples(&self, samples: &Samples) -> Result<Self::SampledOutput> {
        let out = samples.map(|s| {
            let value = self.evaluate(s)?;
            Ok(value)
        })?;
        Ok(out)
    }

    fn required_ids(&self) -> BTreeSet<u64> {
        self.used_decision_variable_ids()
    }
}

impl Evaluate for Quadratic {
    type Output = f64;
    type SampledOutput = SampledValues;

    fn evaluate(&self, solution: &State) -> Result<f64> {
        let mut sum = if let Some(linear) = &self.linear {
            linear.evaluate(solution)?
        } else {
            0.0
        };
        for (i, j, value) in
            itertools::multizip((self.rows.iter(), self.columns.iter(), self.values.iter()))
        {
            let u = solution
                .entries
                .get(i)
                .with_context(|| format!("Variable id ({i}) is not found in the solution"))?;
            let v = solution
                .entries
                .get(j)
                .with_context(|| format!("Variable id ({j}) is not found in the solution"))?;
            sum += value * u * v;
        }
        Ok(sum)
    }

    fn partial_evaluate(&mut self, state: &State) -> Result<()> {
        let mut linear = BTreeMap::new();
        let mut constant = self.linear.as_ref().map_or(0.0, |l| l.constant);
        for term in self.linear.iter().flat_map(|l| l.terms.iter()) {
            if let Some(value) = state.entries.get(&term.id) {
                constant += term.coefficient * value;
            } else {
                *linear.entry(term.id).or_insert(0.0) += term.coefficient;
            }
        }

        ensure!(self.rows.len() == self.columns.len());
        ensure!(self.rows.len() == self.values.len());
        let mut i = 0;
        while i < self.rows.len() {
            let (row, column, value) = (self.rows[i], self.columns[i], self.values[i]);
            match (state.entries.get(&row), state.entries.get(&column)) {
                (Some(u), Some(v)) => {
                    constant += value * u * v;
                }
                (Some(u), None) => {
                    *linear.entry(column).or_insert(0.0) += value * u;
                }
                (None, Some(v)) => {
                    *linear.entry(row).or_insert(0.0) += value * v;
                }
                _ => {
                    i += 1;
                    continue;
                }
            }
            self.rows.swap_remove(i);
            self.columns.swap_remove(i);
            self.values.swap_remove(i);
        }
        if linear.is_empty() && constant == 0.0 {
            self.linear = None;
        } else {
            self.linear = Some(Linear::new(linear.into_iter(), constant));
        }
        Ok(())
    }

    fn evaluate_samples(&self, samples: &Samples) -> Result<Self::SampledOutput> {
        let out = samples.map(|s| {
            let value = self.evaluate(s)?;
            Ok(value)
        })?;
        Ok(out)
    }

    fn required_ids(&self) -> BTreeSet<u64> {
        self.used_decision_variable_ids()
    }
}

impl Evaluate for Polynomial {
    type Output = f64;
    type SampledOutput = SampledValues;

    fn evaluate(&self, solution: &State) -> Result<f64> {
        let mut sum = 0.0;
        for term in &self.terms {
            let mut v = term.coefficient;
            for id in &term.ids {
                v *= solution
                    .entries
                    .get(id)
                    .with_context(|| format!("Variable id ({id}) is not found in the solution"))?;
            }
            sum += v;
        }
        Ok(sum)
    }

    fn partial_evaluate(&mut self, state: &State) -> Result<()> {
        let mut monomials = BTreeMap::new();
        for term in self.terms.iter() {
            let mut value = term.coefficient;
            if value.abs() <= f64::EPSILON {
                continue;
            }
            let mut ids = Vec::new();
            for id in term.ids.iter() {
                if let Some(v) = state.entries.get(id) {
                    value *= v;
                } else {
                    ids.push(*id);
                }
            }
            let coefficient: &mut f64 = monomials.entry(ids.clone()).or_default();
            *coefficient += value;
            if coefficient.abs() <= f64::EPSILON {
                monomials.remove(&ids);
            }
        }
        self.terms = monomials
            .into_iter()
            .map(|(ids, coefficient)| Monomial { ids, coefficient })
            .collect();
        Ok(())
    }

    fn evaluate_samples(&self, samples: &Samples) -> Result<Self::SampledOutput> {
        let out = samples.map(|s| {
            let value = self.evaluate(s)?;
            Ok(value)
        })?;
        Ok(out)
    }

    fn required_ids(&self) -> BTreeSet<u64> {
        self.used_decision_variable_ids()
    }
}

impl Evaluate for Constraint {
    type Output = EvaluatedConstraint;
    type SampledOutput = SampledConstraint;

    fn evaluate(&self, solution: &State) -> Result<Self::Output> {
        let evaluated_value = self.function().evaluate(solution)?;
        let used_decision_variable_ids = self
            .function()
            .used_decision_variable_ids()
            .into_iter()
            .collect();
        Ok(EvaluatedConstraint {
            id: self.id,
            equality: self.equality,
            evaluated_value,
            used_decision_variable_ids,
            name: self.name.clone(),
            subscripts: self.subscripts.clone(),
            parameters: self.parameters.clone(),
            description: self.description.clone(),
            dual_variable: None,
            removed_reason: None,
            removed_reason_parameters: Default::default(),
        })
    }

    fn partial_evaluate(&mut self, state: &State) -> Result<()> {
        let Some(f) = self.function.as_mut() else {
            // Since empty function means zero constant, we can return an empty set
            return Ok(());
        };
        f.partial_evaluate(state)
    }

    fn evaluate_samples(&self, samples: &Samples) -> Result<Self::SampledOutput> {
        let evaluated_values = self.function().evaluate_samples(samples)?;
        let feasible: HashMap<u64, bool> = evaluated_values
            .iter()
            .map(|(sample_id, value)| {
                if self.equality() == Equality::EqualToZero {
                    return Ok((*sample_id, value.abs() < 1e-6));
                }
                if self.equality() == Equality::LessThanOrEqualToZero {
                    return Ok((*sample_id, *value < 1e-6));
                }
                bail!("Unsupported equality: {:?}", self.equality());
            })
            .collect::<Result<_>>()?;
        Ok(SampledConstraint {
            id: self.id,
            evaluated_values: Some(evaluated_values),
            used_decision_variable_ids: self
                .function()
                .used_decision_variable_ids()
                .into_iter()
                .collect(),
            name: self.name.clone(),
            subscripts: self.subscripts.clone(),
            parameters: self.parameters.clone(),
            description: self.description.clone(),
            equality: self.equality,
            feasible,
            removed_reason: None,
            removed_reason_parameters: Default::default(),
        })
    }

    fn required_ids(&self) -> BTreeSet<u64> {
        self.function
            .as_ref()
            .map_or(BTreeSet::new(), |f| f.used_decision_variable_ids())
    }
}

impl Evaluate for RemovedConstraint {
    type Output = EvaluatedConstraint;
    type SampledOutput = SampledConstraint;

    fn evaluate(&self, solution: &State) -> Result<Self::Output> {
        let mut out = self
            .constraint
            .as_ref()
            .context("RemovedConstraint does not contain constraint")?
            .evaluate(solution)?;
        out.removed_reason = Some(self.removed_reason.clone());
        out.removed_reason_parameters = self.removed_reason_parameters.clone();
        Ok(out)
    }

    fn partial_evaluate(&mut self, state: &State) -> Result<()> {
        self.constraint
            .as_mut()
            .context("RemovedConstraint does not contain constraint")?
            .partial_evaluate(state)
    }

    fn evaluate_samples(&self, samples: &Samples) -> Result<Self::SampledOutput> {
        let mut evaluated = self
            .constraint
            .as_ref()
            .expect("RemovedConstraint does not contain constraint")
            .evaluate_samples(samples)?;
        evaluated.removed_reason = Some(self.removed_reason.clone());
        evaluated.removed_reason_parameters = self.removed_reason_parameters.clone();
        Ok(evaluated)
    }

    fn required_ids(&self) -> BTreeSet<u64> {
        self.constraint
            .as_ref()
            .map_or(BTreeSet::new(), |c| c.required_ids())
    }
}

impl Evaluate for Instance {
    type Output = Solution;
    type SampledOutput = SampleSet;

    fn evaluate(&self, state: &State) -> Result<Self::Output> {
        self.check_bound(state, 1e-7)?;
        let mut evaluated_constraints = Vec::new();
        let mut feasible_relaxed = true;
        for c in &self.constraints {
            let c = c.evaluate(state)?;
            // Only check non-removed constraints for feasibility
            if feasible_relaxed {
                feasible_relaxed = c.is_feasible(1e-6)?;
            }
            evaluated_constraints.push(c);
        }
        let mut feasible = feasible_relaxed;
        for c in &self.removed_constraints {
            let c = c.evaluate(state)?;
            if feasible {
                feasible = c.is_feasible(1e-6)?;
            }
            evaluated_constraints.push(c);
        }

        let objective = self.objective().evaluate(state)?;

        let mut state = state.clone();
        for v in &self.decision_variables {
            if let Some(value) = v.substituted_value {
                state.entries.insert(v.id, value);
            }
        }
        eval_dependencies(&self.decision_variable_dependency, &mut state)?;
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

    fn partial_evaluate(&mut self, state: &State) -> Result<()> {
        for v in &mut self.decision_variables {
            if let Some(value) = state.entries.get(&v.id) {
                v.substituted_value = Some(*value);
            }
        }
        if let Some(f) = self.objective.as_mut() {
            f.partial_evaluate(state)?
        }
        for constraints in &mut self.constraints {
            constraints.partial_evaluate(state)?;
        }
        for constraints in &mut self.removed_constraints {
            constraints.partial_evaluate(state)?;
        }
        for d in self.decision_variable_dependency.values_mut() {
            d.partial_evaluate(state)?;
        }
        Ok(())
    }

    fn evaluate_samples(&self, samples: &Samples) -> Result<Self::SampledOutput> {
        let mut feasible_relaxed: HashMap<u64, bool> =
            samples.ids().map(|id| (*id, true)).collect();

        // Constraints
        let mut constraints = Vec::new();
        for c in &self.constraints {
            let evaluated = c.evaluate_samples(samples)?;
            for (sample_id, feasible_) in evaluated.is_feasible(1e-6)? {
                if !feasible_ {
                    feasible_relaxed.insert(sample_id, false);
                }
            }
            constraints.push(evaluated);
        }
        let mut feasible = feasible_relaxed.clone();
        for c in &self.removed_constraints {
            let v = c.evaluate_samples(samples)?;
            for (sample_id, feasible_) in v.is_feasible(1e-6)? {
                if !feasible_ {
                    feasible.insert(sample_id, false);
                }
            }
            constraints.push(v);
        }

        // Objective
        let objectives = self.objective().evaluate_samples(samples)?;

        // Reconstruct decision variable values
        let mut samples = samples.clone();
        for state in samples.states_mut() {
            eval_dependencies(&self.decision_variable_dependency, state?)?;
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

    fn required_ids(&self) -> BTreeSet<u64> {
        self.used_decision_variable_ids()
    }
}

// FIXME: This would be better by using a topological sort
fn eval_dependencies(dependencies: &HashMap<u64, Function>, state: &mut State) -> Result<()> {
    let mut bucket: Vec<_> = dependencies.iter().collect();
    let mut last_size = bucket.len();
    let mut not_evaluated = Vec::new();
    loop {
        while let Some((id, f)) = bucket.pop() {
            match f.evaluate(state) {
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
    use crate::random::*;
    use approx::*;
    use maplit::*;
    use proptest::prelude::*;

    #[test]
    fn test_eval_dependencies() {
        let mut state = State::from_iter(vec![(1, 1.0), (2, 2.0), (3, 3.0)]);
        let dependencies = hashmap! {
            4 => Function::from(Linear::new([(1, 1.0), (2, 2.0)].into_iter(), 0.0)),
            5 => Function::from(Linear::new([(4, 1.0), (3, 3.0)].into_iter(), 0.0)),
        };
        eval_dependencies(&dependencies, &mut state).unwrap();
        assert_eq!(state.entries[&4], 1.0 + 2.0 * 2.0);
        assert_eq!(state.entries[&5], 1.0 + 2.0 * 2.0 + 3.0 * 3.0);

        // circular dependency
        let mut state = State::from_iter(vec![(1, 1.0), (2, 2.0), (3, 3.0)]);
        let dependencies = hashmap! {
            4 => Function::from(Linear::new([(1, 1.0), (5, 2.0)].into_iter(), 0.0)),
            5 => Function::from(Linear::new([(4, 1.0), (3, 3.0)].into_iter(), 0.0)),
        };
        assert!(eval_dependencies(&dependencies, &mut state).is_err());

        // non-existing dependency
        let mut state = State::from_iter(vec![(1, 1.0), (2, 2.0), (3, 3.0)]);
        let dependencies = hashmap! {
            4 => Function::from(Linear::new([(1, 1.0), (6, 2.0)].into_iter(), 0.0)),
            5 => Function::from(Linear::new([(4, 1.0), (3, 3.0)].into_iter(), 0.0)),
        };
        assert!(eval_dependencies(&dependencies, &mut state).is_err());
    }

    #[test]
    fn linear_partial_evaluate() {
        let mut linear = Linear::new([(1, 1.0), (2, 2.0), (3, 3.0), (4, 4.0)].into_iter(), 5.0);
        let state = State {
            entries: hashmap! { 1 => 1.0, 2 => 2.0, 3 => 3.0, 5 => 5.0, 6 => 6.0 },
        };
        linear.partial_evaluate(&state).unwrap();
        assert_eq!(linear.constant, 5.0 + 1.0 * 1.0 + 2.0 * 2.0 + 3.0 * 3.0);
        assert_eq!(linear.terms.len(), 1);
        assert_eq!(linear.terms[0].id, 4);
        assert_eq!(linear.terms[0].coefficient, 4.0);
    }

    macro_rules! pair_with_state {
        ($t:ty) => {
            (<$t>::arbitrary(), <$t>::arbitrary()).prop_flat_map(|(f, g)| {
                let ids = f
                    .used_decision_variable_ids()
                    .union(&g.used_decision_variable_ids())
                    .cloned()
                    .collect();
                (Just(f), Just(g), arbitrary_state(ids))
            })
        };
    }

    /// f(x) + g(x) = (f + g)(x)
    macro_rules! evaluate_add_commutativity {
        ($t:ty, $name:ident) => {
            proptest! {
                #[test]
                fn $name((f, g, s) in pair_with_state!($t)) {
                    let f_value = f.evaluate(&s).unwrap();
                    let g_value = g.evaluate(&s).unwrap();
                    let h_value = (f + g).evaluate(&s).unwrap();
                    prop_assert!(abs_diff_eq!(dbg!(f_value + g_value), dbg!(h_value), epsilon = 1e-9));
                }
            }
        };
    }
    /// f(x) * g(x) = (f * g)(x)
    macro_rules! evaluate_mul_commutativity {
        ($t:ty, $name:ident) => {
            proptest! {
                #[test]
                fn $name((f, g, s) in pair_with_state!($t)) {
                    let f_value = f.evaluate(&s).unwrap();
                    let g_value = g.evaluate(&s).unwrap();
                    let h_value = (f * g).evaluate(&s).unwrap();
                    prop_assert!(abs_diff_eq!(dbg!(f_value * g_value), dbg!(h_value), epsilon = 1e-9));
                }
            }
        };
    }
    evaluate_add_commutativity!(Linear, linear_evaluate_add_commutativity);
    evaluate_mul_commutativity!(Linear, linear_evaluate_mul_commutativity);
    evaluate_add_commutativity!(Quadratic, quadratic_evaluate_add_commutativity);
    evaluate_mul_commutativity!(Quadratic, quadratic_evaluate_mul_commutativity);
    evaluate_add_commutativity!(Polynomial, polynomial_evaluate_add_commutativity);
    evaluate_mul_commutativity!(Polynomial, polynomial_evaluate_mul_commutativity);
    evaluate_add_commutativity!(Function, function_evaluate_add_commutativity);
    evaluate_mul_commutativity!(Function, function_evaluate_mul_commutativity);

    macro_rules! function_with_state {
        ($t:ty) => {
            <$t>::arbitrary().prop_flat_map(|f| {
                let ids = f.used_decision_variable_ids();
                (Just(f), arbitrary_state(ids))
            })
        };
    }

    macro_rules! partial_evaluate_to_constant {
        ($t:ty, $name:ident) => {
            proptest! {
                #[test]
                fn $name((mut f, s) in function_with_state!($t)) {
                    let v = f.evaluate(&s).unwrap();
                    f.partial_evaluate(&s).unwrap();
                    let c = dbg!(f).as_constant().expect("Non constant");
                    prop_assert!(abs_diff_eq!(v, c, epsilon = 1e-9));
                }
            }
        };
    }
    partial_evaluate_to_constant!(Linear, linear_partial_evaluate_to_constant);
    partial_evaluate_to_constant!(Quadratic, quadratic_partial_evaluate_to_constant);
    partial_evaluate_to_constant!(Polynomial, polynomial_partial_evaluate_to_constant);
    partial_evaluate_to_constant!(Function, function_partial_evaluate_to_constant);

    fn split_state(state: State) -> BoxedStrategy<(State, State)> {
        let ids: Vec<(u64, f64)> = state.entries.into_iter().collect();
        let flips = proptest::collection::vec(bool::arbitrary(), ids.len());
        (Just(ids), flips)
            .prop_map(|(ids, flips)| {
                let mut a = State::default();
                let mut b = State::default();
                for (flip, (id, value)) in flips.into_iter().zip(ids.into_iter()) {
                    if flip {
                        a.entries.insert(id, value);
                    } else {
                        b.entries.insert(id, value);
                    }
                }
                (a, b)
            })
            .boxed()
    }

    macro_rules! function_with_split_state {
        ($t:ty) => {
            <$t>::arbitrary().prop_flat_map(|f| {
                let ids = f.used_decision_variable_ids();
                (Just(f), arbitrary_state(ids))
                    .prop_flat_map(|(f, s)| (Just(f), Just(s.clone()), split_state(s)))
            })
        };
    }

    macro_rules! half_partial_evaluate {
        ($t:ty, $name:ident) => {
            proptest! {
                #[test]
                fn $name((mut f, s, (s1, s2)) in function_with_split_state!($t)) {
                    let v = f.evaluate(&s).unwrap();
                    f.partial_evaluate(&s1).unwrap();
                    let u = f.evaluate(&s2).unwrap();
                    prop_assert!(abs_diff_eq!(v, u, epsilon = 1e-9));
                }
            }
        };
    }
    half_partial_evaluate!(Linear, linear_half_partial_evaluate);
    half_partial_evaluate!(Quadratic, quadratic_half_partial_evaluate);
    half_partial_evaluate!(Polynomial, polynomial_half_partial_evaluate);
    half_partial_evaluate!(Function, function_half_partial_evaluate);

    fn instance_with_state() -> BoxedStrategy<(Instance, State)> {
        Instance::arbitrary()
            .prop_flat_map(|instance| {
                let bounds = instance.get_bounds().expect("Invalid Bound in Instance");
                let state = arbitrary_state_within_bounds(&bounds, 100.0);
                (Just(instance), state)
            })
            .boxed()
    }

    proptest! {
        #[test]
        fn evaluate_instance((instance, state) in instance_with_state()) {
            let solution = instance.evaluate(&state).unwrap();
            let mut cids = instance.constraint_ids();
            cids.extend(instance.removed_constraint_ids());
            prop_assert!(solution.constraint_ids() == cids);
        }
    }

    proptest! {
        #[test]
        fn partial_eval_instance(mut instance in Instance::arbitrary(), state in any::<State>()) {
            instance.partial_evaluate(&state).unwrap();
            for v in &instance.decision_variables {
                if let Some(value) = state.entries.get(&v.id) {
                    prop_assert_eq!(v.substituted_value, Some(*value));
                } else {
                    prop_assert_eq!(v.substituted_value, None);
                }
            }
        }
    }

    fn instance_with_split_state() -> BoxedStrategy<(Instance, State, (State, State))> {
        Instance::arbitrary()
            .prop_flat_map(|instance| {
                let bounds = instance.get_bounds().expect("Invalid Bound in Instance");
                let state = arbitrary_state_within_bounds(&bounds, 100.0);
                (Just(instance), state).prop_flat_map(|(instance, state)| {
                    (Just(instance), Just(state.clone()), split_state(state))
                })
            })
            .boxed()
    }

    proptest! {
        #[test]
        fn partial_eval_instance_to_solution((mut instance, state, (s1, s2)) in instance_with_split_state()) {
            let solution = instance.evaluate(&state).unwrap();
            instance.partial_evaluate(&s1).unwrap();
            let solution1 = instance.evaluate(&s2).unwrap();
            prop_assert_eq!(solution.decision_variable_ids(), solution1.decision_variable_ids());
            prop_assert_eq!(solution.constraint_ids(), solution1.constraint_ids());
            prop_assert_eq!(solution.state, solution1.state);
        }
    }

    proptest! {
        #[test]
        fn evaluate_samples((instance, state) in instance_with_state()) {
            let solution = instance.evaluate(&state).unwrap();

            let mut samples = Samples::default();
            samples.add_sample(0, state);
            let sample_set = instance.evaluate_samples(&samples).unwrap();

            prop_assert_eq!(solution, sample_set.get(0).unwrap());
        }
    }

    proptest! {
        #[test]
        fn substitute((f, mut g, mut s) in pair_with_state!(Function)) {
            // Determine ID to be substituted
            let ids = f.used_decision_variable_ids();
            let Some(id) = ids.iter().next().cloned() else { return Ok(()) };
            g.partial_evaluate(&State { entries: hashmap!{ id => 1.0 }}).unwrap();
            let substituted = f.substitute(&hashmap!{ id => g.clone() }).unwrap();

            let g_value = g.evaluate(&s).unwrap();
            s.entries.insert(id, g_value);

            let f_value = f.evaluate(&s).unwrap();
            let substituted_value = substituted.evaluate(&s).unwrap();

            prop_assert!(abs_diff_eq!(f_value, substituted_value, epsilon = 1e-9));
        }
    }
}
