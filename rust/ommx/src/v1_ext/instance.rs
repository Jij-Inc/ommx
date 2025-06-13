use crate::{
    v1::{
        decision_variable::Kind, instance::Sense, DecisionVariable, Equality, Function, Instance,
        Linear, Optimality, Parameter, ParametricInstance, Relaxation, RemovedConstraint,
        SampleSet, SampledDecisionVariable, Samples, Solution, State,
    },
    BinaryIdPair, BinaryIds, Bound, Bounds, ConstraintID, Evaluate, InfeasibleDetected, VariableID,
    VariableIDSet,
};
use anyhow::{bail, ensure, Context, Result};
use approx::AbsDiffEq;
use maplit::hashmap;
use num::Zero;
use std::{
    borrow::Cow,
    collections::{hash_map::Entry as HashMapEntry, BTreeMap, BTreeSet, HashMap, HashSet},
};

impl Instance {
    pub fn objective(&self) -> Cow<Function> {
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

    pub fn penalty_method(self) -> Result<ParametricInstance> {
        let id_base = self.defined_ids().last().map(|id| id + 1).unwrap_or(0);
        let mut objective = self.objective().into_owned();
        let mut parameters = Vec::new();
        let mut removed_constraints = Vec::new();
        for (i, c) in self.constraints.into_iter().enumerate() {
            let parameter = Parameter {
                id: id_base + i as u64,
                name: Some("penalty_weight".to_string()),
                subscripts: vec![c.id as i64],
                ..Default::default()
            };
            let f = c.function().into_owned();
            objective = objective + &parameter * f.clone() * f;
            removed_constraints.push(RemovedConstraint {
                constraint: Some(c),
                removed_reason: "penalty_method".to_string(),
                removed_reason_parameters: hashmap! { "parameter_id".to_string() => parameter.id.to_string() },
            });
            parameters.push(parameter);
        }
        Ok(ParametricInstance {
            description: self.description,
            objective: Some(objective),
            constraints: Vec::new(),
            decision_variables: self.decision_variables.clone(),
            sense: self.sense,
            parameters,
            constraint_hints: self.constraint_hints,
            removed_constraints,
            decision_variable_dependency: self.decision_variable_dependency,
        })
    }

    pub fn uniform_penalty_method(self) -> Result<ParametricInstance> {
        let id_base = self.defined_ids().last().map(|id| id + 1).unwrap_or(0);
        let mut objective = self.objective().into_owned();
        let parameter = Parameter {
            id: id_base,
            name: Some("uniform_penalty_weight".to_string()),
            ..Default::default()
        };
        let mut removed_constraints = Vec::new();
        let mut quad_sum = Function::zero();
        for c in self.constraints.into_iter() {
            let f = c.function().into_owned();
            quad_sum = quad_sum + f.clone() * f;
            removed_constraints.push(RemovedConstraint {
                constraint: Some(c),
                removed_reason: "uniform_penalty_method".to_string(),
                removed_reason_parameters: Default::default(),
            });
        }
        objective = objective + &parameter * quad_sum;
        Ok(ParametricInstance {
            description: self.description,
            objective: Some(objective),
            constraints: Vec::new(),
            decision_variables: self.decision_variables.clone(),
            sense: self.sense,
            parameters: vec![parameter],
            constraint_hints: self.constraint_hints,
            removed_constraints,
            decision_variable_dependency: self.decision_variable_dependency,
        })
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
            .with_context(|| format!("Constraint ID {} not found", constraint_id))?;
        let c = self.constraints.remove(index);
        self.removed_constraints.push(RemovedConstraint {
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
            .with_context(|| format!("Constraint ID {} not found", constraint_id))?;
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

    /// Create QUBO (Quadratic Unconstrained Binary Optimization) dictionary from the instance.
    ///
    /// Before calling this method, you should check that this instance is suitable for QUBO:
    ///
    /// - This instance has no constraints
    ///   - See [`Instance::penalty_method`] (TODO: ALM will be added) to convert into an unconstrained problem.
    /// - The objective function uses only binary decision variables.
    ///   - TODO: Binary encoding will be added.
    /// - The degree of the objective is at most 2.
    ///
    pub fn as_qubo_format(&self) -> Result<(BTreeMap<BinaryIdPair, f64>, f64)> {
        if self.sense() == Sense::Maximize {
            bail!("QUBO format is only for minimization problems.");
        }
        if !self.constraints.is_empty() {
            bail!("The instance still has constraints. Use penalty method or other way to translate into unconstrained problem first.");
        }
        if !self
            .objective()
            .required_ids()
            .is_subset(&self.binary_ids())
        {
            bail!("The objective function uses non-binary decision variables.");
        }
        let mut constant = 0.0;
        let mut quad = BTreeMap::new();
        for (ids, c) in self.objective().into_iter() {
            if c.abs() <= f64::EPSILON {
                continue;
            }
            if ids.is_empty() {
                constant += c;
            } else {
                let key = BinaryIdPair::try_from(ids)?;
                let value = quad.entry(key).and_modify(|v| *v += c).or_insert(c);
                if value.abs() < f64::EPSILON {
                    quad.remove(&key);
                }
            }
        }
        Ok((quad, constant))
    }

    /// Create HUBO (Higher-Order Unconstrained Binary Optimization) dictionary from the instance.
    ///
    /// Before calling this method, you should check that this instance is suitable for QUBO:
    ///
    /// - This instance has no constraints
    ///   - See [`Instance::penalty_method`] (TODO: ALM will be added) to convert into an unconstrained problem.
    /// - The objective function uses only binary decision variables.
    ///   - TODO: Binary encoding will be added.
    ///
    pub fn as_hubo_format(&self) -> Result<(BTreeMap<BinaryIds, f64>, f64)> {
        if self.sense() == Sense::Maximize {
            bail!("HUBO format is only for minimization problems.");
        }
        if !self.constraints.is_empty() {
            bail!("The instance still has constraints. Use penalty method or other way to translate into unconstrained problem first.");
        }
        if !self
            .objective()
            .required_ids()
            .is_subset(&self.binary_ids())
        {
            bail!("The objective function uses non-binary decision variables.");
        }
        let mut constant = 0.0;
        let mut quad = BTreeMap::new();
        for (ids, c) in self.objective().into_iter() {
            if c.abs() <= f64::EPSILON {
                continue;
            }
            if ids.is_empty() {
                constant += c;
            } else {
                let key = BinaryIds::from(ids);
                let value = quad.entry(key.clone()).and_modify(|v| *v += c).or_insert(c);
                if value.abs() < f64::EPSILON {
                    quad.remove(&key);
                }
            }
        }
        Ok((quad, constant))
    }

    /// Encode an integer decision variable into binary decision variables.
    ///
    /// Note that this method does not substitute the yielded binary representation into the objective and constraints.
    /// Call [`Instance::substitute`] with the returned [`Linear`] representation.
    ///
    /// Mutability
    /// ----------
    /// - This adds new binary decision variables introduced for binary encoding to the instance.
    ///
    /// Errors
    /// ------
    /// Returns [anyhow::Error] in the following cases:
    ///
    /// - The given decision variable ID is not found
    /// - The specified decision variable is not an integer type.
    /// - The bound of the decision variable is not set or not finite.
    ///
    pub fn log_encode(&mut self, decision_variable_id: u64) -> Result<Linear> {
        let v = self
            .decision_variables
            .iter()
            .find(|dv| dv.id == decision_variable_id)
            .with_context(|| format!("Decision variable ID {} not found", decision_variable_id))?;
        if v.kind() != Kind::Integer {
            bail!(
                "The decision variable is not an integer type: ID={}",
                decision_variable_id
            );
        }

        let bound = v.bound.as_ref().with_context(|| {
            format!(
                "Bound must be set and finite for log-encoding: ID={}",
                decision_variable_id
            )
        })?;

        // Bound of integer may be non-integer value
        let upper = bound.upper.floor();
        let lower = bound.lower.ceil();
        let u_l = upper - lower;
        ensure!(
            u_l >= 0.0,
            "No feasible integer found in the bound: ID={}, lower={}, upper={}",
            decision_variable_id,
            bound.lower,
            bound.upper
        );

        // There is only one feasible integer, and no need to encode
        if u_l == 0.0 {
            return Ok(Linear::from(lower));
        }

        // Log-encoding
        let n = (u_l + 1.0).log2().ceil() as usize;
        let id_base = self
            .defined_ids()
            .last()
            .map(|id| id + 1)
            .expect("At least one decision variable here");

        let mut terms = Vec::new();
        for i in 0..n {
            let id = id_base + i as u64;
            terms.push((
                id,
                if i == n - 1 {
                    u_l - 2.0f64.powi(i as i32) + 1.0
                } else {
                    2.0f64.powi(i as i32)
                },
            ));
            self.decision_variables.push(DecisionVariable {
                id,
                name: Some("ommx.log_encode".to_string()),
                subscripts: vec![decision_variable_id as i64, i as i64],
                kind: Kind::Binary as i32,
                bound: Some(crate::v1::Bound {
                    lower: 0.0,
                    upper: 1.0,
                }),
                ..Default::default()
            });
        }
        Ok(Linear::new(terms.into_iter(), lower))
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

    /// Convert inequality `f(x) <= 0` into equality `f(x) + s/a = 0` with an *integer* slack variable `s`.
    ///
    /// Arguments
    /// ---------
    /// - `constraint_id`: The ID of the constraint to be converted.
    /// - `max_integer_range`: The maximum integer range of the slack variable.
    /// - `atol`: Absolute tolerance for approximating the coefficient to rational number.
    ///
    /// Since any `x: f64` can be approximated by an rational number (`x ~ p/q`) within some tolerance,
    /// multiplying the lcm `a` of every denominator of coefficients `q_1, ...` yields `a * f(x)` whose coefficients are all integer.
    /// However, this cause very large coefficients and thus the slack variable may have very large range,
    /// which is not practical for solvers.
    /// `max_integer_range` is used to limit the range of the slack variable, and the method returns error if exceeded it.
    ///
    /// Mutability
    /// ----------
    /// - This evaluates the bound of `f(x)` as `[lower, upper]`, and then:
    ///   - if `lower > 0`, this constraint never be satisfied, and the method returns [`InfeasibleDetected::InequalityConstraintBound`].
    ///   - if `upper <= 0`, this constraint is always satisfied, and the constraint is moved to `removed_constraints`.
    /// - This creates a new decision variable for the slack variable.
    ///   - Its name is `ommx.slack`
    ///   - Its subscript is single element `[constraint_id]`
    ///   - Its bound is determined from `f(x)`
    ///   - Its kind are discussed below
    /// - The constraint is changed as equality with keeping the constraint ID.
    ///   - Its function will be converted `f(x)` to `f(x) + s/a`
    ///
    /// Error
    /// -----
    /// - The constraint ID is not found, or is not inequality
    /// - The constraint contains continuous decision variables
    /// - The slack variable range exceeds `max_integer_range`
    ///
    pub fn convert_inequality_to_equality_with_integer_slack(
        &mut self,
        constraint_id: u64,
        max_integer_range: u64,
        atol: crate::ATol,
    ) -> Result<()> {
        let bounds = self.get_bounds()?;
        let kinds = self.get_kinds();
        let next_id = self.defined_ids().last().map(|id| id + 1).unwrap_or(0);

        let constraint = self
            .constraints
            .iter_mut()
            .find(|c| c.id == constraint_id)
            .with_context(|| format!("Constraint ID {} not found", constraint_id))?;
        let function = constraint
            .function
            .as_ref()
            .with_context(|| format!("Constraint ID {} does not have a function", constraint_id))?;

        // If the constraint contains continuous decision variables, integer slack variable cannot be introduced
        for id in function.required_ids() {
            let kind = kinds
                .get(&id)
                .with_context(|| format!("Decision variable ID {id:?} not found"))?;
            if !matches!(kind, Kind::Binary | Kind::Integer) {
                bail!("The constraint contains continuous decision variables: ID={id:?}");
            }
        }

        // Evaluate minimal integer coefficient multiplier `a` which make all coefficients of `a * f(x)` integer
        let a = function
            .content_factor()
            .context("Cannot normalize the coefficients to integers")?;
        let af = a * function.clone();

        // Check the bound of `a*f`
        // - If `lower > 0`, the constraint is infeasible
        // - If `upper <= 0`, the constraint is always satisfied, thus moved to `removed_constraints`
        let bound = af.evaluate_bound(&bounds);
        let bound = bound.as_integer_bound(atol).ok_or_else(|| {
            InfeasibleDetected::InequalityConstraintBound {
                id: ConstraintID::from(constraint_id),
                bound,
            }
        })?;
        if bound.lower() > 0.0 {
            bail!(InfeasibleDetected::InequalityConstraintBound {
                id: ConstraintID::from(constraint_id),
                bound,
            });
        }
        if bound.upper() <= 0.0 {
            // The constraint is always satisfied
            self.relax_constraint(
                constraint_id,
                "convert_inequality_to_equality_with_integer_slack".to_string(),
                Default::default(),
            )?;
            return Ok(());
        }
        let bound = Bound::new(0.0, -bound.lower()).unwrap();
        if bound.width() > max_integer_range as f64 {
            bail!(
                "The range of the slack variable exceeds the limit: evaluated({width}) > limit({max_integer_range})",
                width = bound.width()
            );
        }

        self.decision_variables.push(DecisionVariable {
            id: next_id,
            name: Some("ommx.slack".to_string()),
            subscripts: vec![constraint_id as i64],
            kind: Kind::Integer as i32,
            bound: Some(bound.into()),
            ..Default::default()
        });
        constraint.function = Some(function.clone() + Linear::single_term(next_id, 1.0 / a));
        constraint.set_equality(Equality::EqualToZero);

        Ok(())
    }

    /// Add integer slack variable to inequality
    ///
    /// This converts an inequality `f(x) <= 0` to `f(x) + b*s <= 0` where `s` is an integer slack variable.
    ///
    /// Mutability
    /// ----------
    /// - This evaluates the bound of `f(x)` as `[lower, upper]`, and then:
    ///   - if `lower > 0`, this constraint never be satisfied, and the method returns [`InfeasibleDetected::InequalityConstraintBound`].
    ///   - if `upper <= 0`, this constraint is always satisfied, and the constraint is moved to `removed_constraints`.
    /// - This adds a new decision variable for the slack variable.
    ///   - Its name is `ommx.slack`
    ///   - Its subscript is single element `[constraint_id]`
    ///   - Its bound is `[0, slack_upper_bound]`
    ///   - Its kind is integer
    ///
    /// Errors
    /// ------
    /// - The constraint ID is not found, or is not inequality
    /// - The constraint contains continuous decision variables
    ///
    pub fn add_integer_slack_to_inequality(
        &mut self,
        constraint_id: u64,
        slack_upper_bound: u64,
    ) -> Result<Option<f64>> {
        let slack_id = self.defined_ids().last().map(|id| id + 1).unwrap_or(0);
        let bounds = self.get_bounds()?;
        let kinds = self.get_kinds();
        let constraint = self
            .constraints
            .iter_mut()
            .find(|c| c.id == constraint_id)
            .with_context(|| format!("Constraint ID {} not found", constraint_id))?;
        if constraint.equality() != Equality::LessThanOrEqualToZero {
            bail!("The constraint is not inequality: ID={}", constraint_id);
        }
        let f = constraint
            .function
            .as_ref()
            .with_context(|| format!("Constraint ID {} does not have a function", constraint_id))?;

        for id in f.required_ids() {
            let kind = kinds
                .get(&id)
                .with_context(|| format!("Decision variable ID {id:?} not found"))?;
            if !matches!(kind, Kind::Binary | Kind::Integer) {
                bail!("The constraint contains continuous decision variables: ID={id:?}");
            }
        }

        let bound = f.evaluate_bound(&bounds);
        if bound.lower() > 0.0 {
            bail!(InfeasibleDetected::InequalityConstraintBound {
                id: ConstraintID::from(constraint_id),
                bound,
            });
        }
        if bound.upper() <= 0.0 {
            // The constraint is always satisfied
            self.relax_constraint(
                constraint_id,
                "add_integer_slack_to_inequality".to_string(),
                Default::default(),
            )?;
            return Ok(None);
        }
        let b = -bound.lower() / slack_upper_bound as f64;

        self.decision_variables.push(DecisionVariable {
            id: slack_id,
            name: Some("ommx.slack".to_string()),
            subscripts: vec![constraint_id as i64],
            kind: Kind::Integer as i32,
            bound: Some(crate::v1::Bound {
                lower: 0.0,
                upper: slack_upper_bound as f64,
            }),
            ..Default::default()
        });
        constraint.function = Some(f.clone() + Linear::single_term(slack_id, b));
        Ok(Some(b))
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
        for c in &self.removed_constraints {
            let c = c.evaluate(state, atol)?;
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
        for constraints in &mut self.removed_constraints {
            constraints.partial_evaluate(state, atol)?;
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
        for c in &self.removed_constraints {
            let v = c.evaluate_samples(samples, atol)?;
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
        random::InstanceParameters,
        v1::{Parameters, State},
        Evaluate,
    };
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn test_instance_arbitrary_any(instance in Instance::arbitrary()) {
            instance.validate().unwrap();
        }

        #[test]
        fn test_penalty_method(instance in Instance::arbitrary()) {
            let Ok(parametric_instance) = instance.clone().penalty_method() else { return Ok(()); };
            let dv_ids = parametric_instance.defined_decision_variable_ids();
            let p_ids = parametric_instance.defined_parameter_ids();
            prop_assert!(dv_ids.is_disjoint(&p_ids));

            let used_ids = parametric_instance.used_ids().unwrap();
            let all_ids = dv_ids.union(&p_ids).cloned().collect();
            prop_assert!(used_ids.is_subset(&all_ids));

            // Put every penalty weights to zero
            let parameters = Parameters {
                entries: p_ids.iter().map(|&id| (id.into_inner(), 0.0)).collect(),
            };
            let substituted = parametric_instance.clone().with_parameters(parameters, crate::ATol::default()).unwrap();
            prop_assert!(instance.objective().abs_diff_eq(&substituted.objective(), crate::ATol::default()));
            prop_assert_eq!(substituted.constraints.len(), 0);

            // Put every penalty weights to two
            let parameters = Parameters {
                entries: p_ids.iter().map(|&id| (id.into_inner(), 2.0)).collect(),
            };
            let substituted = parametric_instance.with_parameters(parameters, crate::ATol::default()).unwrap();
            let mut objective = instance.objective().into_owned();
            for c in &instance.constraints {
                let f = c.function().into_owned();
                objective = objective + 2.0 * f.clone() * f;
            }
            prop_assert!(objective.abs_diff_eq(&substituted.objective(), crate::ATol::default()));
        }

        #[test]
        fn test_uniform_penalty_method(instance in Instance::arbitrary()) {
            let Ok(parametric_instance) = instance.clone().uniform_penalty_method() else { return Ok(()); };
            let dv_ids = parametric_instance.defined_decision_variable_ids();
            let p_ids = parametric_instance.defined_parameter_ids();
            prop_assert!(dv_ids.is_disjoint(&p_ids));
            prop_assert_eq!(p_ids.len(), 1);

            let used_ids = parametric_instance.used_ids().unwrap();
            let all_ids = dv_ids.union(&p_ids).cloned().collect();
            prop_assert!(used_ids.is_subset(&all_ids));

            // Put every penalty weights to zero
            let parameters = Parameters {
                entries: p_ids.iter().map(|&id| (id.into_inner(), 0.0)).collect(),
            };
            let substituted = parametric_instance.clone().with_parameters(parameters, crate::ATol::default()).unwrap();
            prop_assert!(instance.objective().abs_diff_eq(&substituted.objective(), crate::ATol::default()));
            prop_assert_eq!(substituted.constraints.len(), 0);

            // Put every penalty weights to two
            let parameters = Parameters {
                entries: p_ids.iter().map(|&id| (id.into_inner(), 2.0)).collect(),
            };
            let substituted = parametric_instance.with_parameters(parameters, crate::ATol::default()).unwrap();
            let mut objective = instance.objective().into_owned();
            for c in &instance.constraints {
                let f = c.function().into_owned();
                objective = objective + 2.0 * f.clone() * f;
            }
            prop_assert!(objective.abs_diff_eq(&substituted.objective(), crate::ATol::default()));
        }

        #[test]
        fn test_qubo(instance in Instance::arbitrary_with(InstanceParameters::default_qubo())) {
            if instance.sense() == Sense::Maximize {
                return Ok(());
            }
            let (quad, _) = instance.as_qubo_format().unwrap();
            for (ids, c) in quad {
                prop_assert!(ids.0 <= ids.1);
                prop_assert!(c.abs() > f64:: EPSILON);
            }
        }

        #[test]
        fn test_hubo(instance in Instance::arbitrary_with(InstanceParameters::default_hubo())) {
            if instance.sense() == Sense::Maximize {
                return Ok(());
            }
            let degree = instance.objective().degree();
            let (quad, _) = instance.as_hubo_format().unwrap();
            for (ids, c) in quad {
                prop_assert!(ids.len() <= degree as usize);
                prop_assert!(c.abs() > f64:: EPSILON);
            }
        }

        #[test]
        fn log_encode((lower, upper) in (-10.0_f64..10.0, -10.0_f64..10.0)
            .prop_filter("At least one integer", |(lower, upper)| lower.ceil() <= upper.floor())
        ) {
            let mut instance = Instance::default();
            instance.decision_variables.push(DecisionVariable {
                id: 0,
                name: Some("x".to_string()),
                kind: Kind::Integer as i32,
                bound: Some(crate::v1::Bound { lower, upper }),
                ..Default::default()
            });
            let encoded = instance.log_encode(0).unwrap();

            // Test the ID of yielded decision variables are not duplicated
            instance.validate().unwrap();

            // Get decision variables introduced for log-encoding
            let aux_bits = instance
                .decision_variables
                .iter()
                .filter_map(|dv| {
                    if dv.name == Some("ommx.log_encode".to_string()) && dv.subscripts[0] == 0 {
                        Some(dv.id)
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>();

            if lower.ceil() == upper.floor() {
                // No need to encode
                prop_assert_eq!(encoded.as_constant().unwrap(), lower.ceil());
                prop_assert_eq!(aux_bits.len(), 0);
                return Ok(());
            }

            let state = State { entries: aux_bits.iter().map(|&id| (id, 0.0)).collect::<HashMap<_, _>>() };
            let lower_evaluated = encoded.evaluate(&state, crate::ATol::default()).unwrap();
            prop_assert_eq!(lower_evaluated, lower.ceil());

            let state = State { entries: aux_bits.iter().map(|&id| (id, 1.0)).collect::<HashMap<_, _>>() };
            let upper_evaluated = encoded.evaluate(&state, crate::ATol::default()).unwrap();
            prop_assert_eq!(upper_evaluated, upper.floor());
        }

        /// Compare the result of partial_evaluate and substitute with `Function::Constant`.
        #[test]
        fn substitute_fixed_value(instance in Instance::arbitrary(), value in -3.0..3.0) {
            for id in instance.defined_ids() {
                let mut partially_evaluated = instance.clone();
                partially_evaluated.partial_evaluate(&State { entries: hashmap! { id => value } }, crate::ATol::default()).unwrap();
                let mut substituted = instance.clone();
                substituted.substitute(hashmap! { id => Function::from(value) }).unwrap();
                prop_assert!(partially_evaluated.abs_diff_eq(&substituted, crate::ATol::default()));
            }
        }
    }

    #[test]
    fn test_eval_dependencies() {
        let mut state = State::from_iter(vec![(1, 1.0), (2, 2.0), (3, 3.0)]);
        let dependencies = hashmap! {
            4 => Function::from(Linear::new([(1, 1.0), (2, 2.0)].into_iter(), 0.0)),
            5 => Function::from(Linear::new([(4, 1.0), (3, 3.0)].into_iter(), 0.0)),
        };
        eval_dependencies(&dependencies, &mut state, crate::ATol::default()).unwrap();
        assert_eq!(state.entries[&4], 1.0 + 2.0 * 2.0);
        assert_eq!(state.entries[&5], 1.0 + 2.0 * 2.0 + 3.0 * 3.0);

        // circular dependency
        let mut state = State::from_iter(vec![(1, 1.0), (2, 2.0), (3, 3.0)]);
        let dependencies = hashmap! {
            4 => Function::from(Linear::new([(1, 1.0), (5, 2.0)].into_iter(), 0.0)),
            5 => Function::from(Linear::new([(4, 1.0), (3, 3.0)].into_iter(), 0.0)),
        };
        assert!(eval_dependencies(&dependencies, &mut state, crate::ATol::default()).is_err());

        // non-existing dependency
        let mut state = State::from_iter(vec![(1, 1.0), (2, 2.0), (3, 3.0)]);
        let dependencies = hashmap! {
            4 => Function::from(Linear::new([(1, 1.0), (6, 2.0)].into_iter(), 0.0)),
            5 => Function::from(Linear::new([(4, 1.0), (3, 3.0)].into_iter(), 0.0)),
        };
        assert!(eval_dependencies(&dependencies, &mut state, crate::ATol::default()).is_err());
    }
}
