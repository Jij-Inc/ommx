use crate::v1::{
    decision_variable::Kind,
    instance::{Description, Sense},
    Equality, Function, Instance, Parameter, ParametricInstance, RemovedConstraint,
};
use anyhow::{bail, Context, Result};
use approx::AbsDiffEq;
use maplit::hashmap;
use num::Zero;
use proptest::prelude::*;
use std::{
    borrow::Cow,
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
};

use super::{
    constraint::arbitrary_constraints,
    decision_variable::arbitrary_decision_variables,
    sorted_ids::{BinaryIdPair, BinaryIds},
};

impl Instance {
    pub fn objective(&self) -> Cow<Function> {
        match &self.objective {
            Some(f) => Cow::Borrowed(f),
            // Empty function is regarded as zero function
            None => Cow::Owned(Function::zero()),
        }
    }

    pub fn used_decision_variable_ids(&self) -> BTreeSet<u64> {
        let mut used_ids = self.objective().used_decision_variable_ids();
        for c in &self.constraints {
            used_ids.extend(c.function().used_decision_variable_ids());
        }
        for c in &self.removed_constraints {
            if let Some(c) = &c.constraint {
                used_ids.extend(c.function().used_decision_variable_ids());
            }
        }
        used_ids
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
        let used_ids = self.used_decision_variable_ids();
        let mut defined_ids = BTreeSet::new();
        for dv in &self.decision_variables {
            if !defined_ids.insert(dv.id) {
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

    pub fn arbitrary_lp() -> BoxedStrategy<Self> {
        (0..10_usize, 0..10_usize, 0..=1_u32, 0..10_u64)
            .prop_flat_map(Self::arbitrary_with)
            .boxed()
    }

    pub fn arbitrary_binary() -> BoxedStrategy<Self> {
        (0..10_usize, 0..10_usize, 0..=4_u32, 0..10_u64)
            .prop_flat_map(|(num_constraints, num_terms, max_degree, max_id)| {
                arbitrary_instance(
                    num_constraints,
                    num_terms,
                    max_degree,
                    max_id,
                    Just(Kind::Binary),
                )
            })
            .boxed()
    }

    pub fn arbitrary_binary_unconstrained() -> BoxedStrategy<Self> {
        (0..10_usize, 0..=4_u32, 0..10_u64)
            .prop_flat_map(|(num_terms, max_degree, max_id)| {
                arbitrary_instance(0, num_terms, max_degree, max_id, Just(Kind::Binary))
            })
            .boxed()
    }

    pub fn arbitrary_quadratic_binary_unconstrained() -> BoxedStrategy<Self> {
        (0..10_usize, 0..=2_u32, 0..10_u64)
            .prop_flat_map(|(num_terms, max_degree, max_id)| {
                arbitrary_instance(0, num_terms, max_degree, max_id, Just(Kind::Binary))
            })
            .boxed()
    }

    pub fn penalty_method(self) -> Result<ParametricInstance> {
        let id_base = self.defined_ids().last().map(|id| id + 1).unwrap_or(0);
        let mut objective = self.objective().into_owned();
        let mut parameters = Vec::new();
        let mut removed_constraints = Vec::new();
        for (i, c) in self.constraints.into_iter().enumerate() {
            if c.equality() != Equality::EqualToZero {
                bail!("Penalty method is only for equality constraints. Non-equality constraint is found: ID={}", c.id);
            }
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
            if c.equality() != Equality::EqualToZero {
                bail!("Uniform penalty method is only for equality constraints. Non-equality constraint is found: ID={}", c.id);
            }
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

    pub fn binary_ids(&self) -> BTreeSet<u64> {
        self.decision_variables
            .iter()
            .filter(|dv| dv.kind() == Kind::Binary)
            .map(|dv| dv.id)
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
            .position(|c| {
                c.constraint
                    .as_ref()
                    .map_or(false, |c| c.id == constraint_id)
            })
            .with_context(|| format!("Constraint ID {} not found", constraint_id))?;
        let c = self.removed_constraints.remove(index).constraint.unwrap();
        self.constraints.push(c);
        Ok(())
    }

    /// Create PUBO (Polynomial Unconstrained Binary Optimization) dictionary from the instance.
    ///
    /// Before calling this method, you should check that this instance is suitable for PUBO:
    ///
    /// - This instance has no constraints
    ///   - See [`Instance::penalty_method`] (TODO: ALM will be added) to convert into an unconstrained problem.
    /// - The objective function uses only binary decision variables.
    ///   - TODO: Binary encoding will be added.
    ///
    pub fn as_pubo_format(&self) -> Result<BTreeMap<BinaryIds, f64>> {
        if !self.constraints.is_empty() {
            bail!("The instance still has constraints. Use penalty method or other way to translate into unconstrained problem first.");
        }
        if self.sense() == Sense::Maximize {
            bail!("PUBO format is only for minimization problems.");
        }
        if !self
            .objective()
            .used_decision_variable_ids()
            .is_subset(&self.binary_ids())
        {
            bail!("The objective function uses non-binary decision variables.");
        }
        let mut out = BTreeMap::new();
        for (ids, c) in self.objective().into_iter() {
            if c.abs() > f64::EPSILON {
                let key = BinaryIds::from(ids);
                let value = out.entry(key.clone()).and_modify(|v| *v += c).or_insert(c);
                if value.abs() < f64::EPSILON {
                    out.remove(&key);
                }
            }
        }
        Ok(out)
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
            .used_decision_variable_ids()
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
}

fn arbitrary_instance(
    num_constraints: usize,
    num_terms: usize,
    max_degree: u32,
    max_id: u64,
    kind_strategy: impl Strategy<Value = Kind> + 'static + Clone,
) -> BoxedStrategy<Instance> {
    (
        proptest::option::of(Function::arbitrary_with((num_terms, max_degree, max_id))),
        arbitrary_constraints(num_constraints, (num_terms, max_degree, max_id)),
    )
        .prop_flat_map(move |(objective, constraints)| {
            let mut used_ids = objective
                .as_ref()
                .map(|f| f.used_decision_variable_ids())
                .unwrap_or_default();
            for c in &constraints {
                used_ids.extend(c.function().used_decision_variable_ids());
            }
            let relaxed = if constraints.is_empty() {
                Just(Vec::new()).boxed()
            } else {
                let constraint_ids = constraints.iter().map(|c| c.id).collect::<Vec<_>>();
                proptest::sample::subsequence(constraint_ids, 0..=constraints.len()).boxed()
            };
            (
                Just(objective),
                Just(constraints),
                arbitrary_decision_variables(used_ids, kind_strategy.clone()),
                Option::<Description>::arbitrary(),
                Sense::arbitrary(),
                relaxed,
                ".{0,3}",
                proptest::collection::hash_map(".{0,3}", ".{0,3}", 0..=2),
            )
                .prop_map(
                    |(
                        objective,
                        constraints,
                        decision_variables,
                        description,
                        sense,
                        relaxed,
                        removed_reason,
                        removed_parameters,
                    )| {
                        let mut instance = Instance {
                            objective,
                            constraints,
                            decision_variables,
                            description,
                            sense: sense as i32,
                            ..Default::default()
                        };
                        for i in relaxed {
                            instance
                                .relax_constraint(
                                    i,
                                    removed_reason.clone(),
                                    removed_parameters.clone(),
                                )
                                .unwrap();
                        }
                        instance
                    },
                )
        })
        .boxed()
}

impl Arbitrary for Instance {
    type Parameters = (usize, usize, u32, u64);
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(
        (num_constraints, num_terms, max_degree, max_id): Self::Parameters,
    ) -> Self::Strategy {
        arbitrary_instance(
            num_constraints,
            num_terms,
            max_degree,
            max_id,
            Kind::arbitrary(),
        )
    }

    fn arbitrary() -> Self::Strategy {
        (0..10_usize, 0..10_usize, 0..4_u32, 0..10_u64)
            .prop_flat_map(Self::arbitrary_with)
            .boxed()
    }
}

impl Arbitrary for Sense {
    type Parameters = ();
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(_parameter: ()) -> Self::Strategy {
        prop_oneof![Just(Sense::Minimize), Just(Sense::Maximize)].boxed()
    }
}

impl Arbitrary for Description {
    type Parameters = ();
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(_parameter: ()) -> Self::Strategy {
        (
            proptest::option::of(".{0,3}"),
            proptest::option::of(".{0,3}"),
            prop_oneof![Just(Vec::new()), proptest::collection::vec(".*", 1..3)],
            proptest::option::of(".{0,3}"),
        )
            .prop_map(|(name, description, authors, created_by)| Description {
                name,
                description,
                authors,
                created_by,
            })
            .boxed()
    }
}

/// Compare two instances as mathematical programming problems. This does not compare the metadata.
///
/// - This regards `min f` and `max -f` as the same problem.
/// - This cannot compare scaled constraints. For example, `2x + 3y <= 4` and `4x + 6y <= 8` are mathematically same,
///   but this regarded them as different problems.
///
impl AbsDiffEq for Instance {
    type Epsilon = f64;

    fn default_epsilon() -> Self::Epsilon {
        f64::default_epsilon()
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v1::Parameters;

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
                entries: p_ids.iter().map(|&id| (id, 0.0)).collect(),
            };
            let substituted = parametric_instance.clone().with_parameters(parameters).unwrap();
            prop_assert!(instance.objective().abs_diff_eq(&substituted.objective(), 1e-10));
            prop_assert_eq!(substituted.constraints.len(), 0);

            // Put every penalty weights to two
            let parameters = Parameters {
                entries: p_ids.iter().map(|&id| (id, 2.0)).collect(),
            };
            let substituted = parametric_instance.with_parameters(parameters).unwrap();
            let mut objective = instance.objective().into_owned();
            for c in &instance.constraints {
                let f = c.function().into_owned();
                objective = objective + 2.0 * f.clone() * f;
            }
            prop_assert!(objective.abs_diff_eq(&substituted.objective(), 1e-10));
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
                entries: p_ids.iter().map(|&id| (id, 0.0)).collect(),
            };
            let substituted = parametric_instance.clone().with_parameters(parameters).unwrap();
            prop_assert!(instance.objective().abs_diff_eq(&substituted.objective(), 1e-10));
            prop_assert_eq!(substituted.constraints.len(), 0);

            // Put every penalty weights to two
            let parameters = Parameters {
                entries: p_ids.iter().map(|&id| (id, 2.0)).collect(),
            };
            let substituted = parametric_instance.with_parameters(parameters).unwrap();
            let mut objective = instance.objective().into_owned();
            for c in &instance.constraints {
                let f = c.function().into_owned();
                objective = objective + 2.0 * f.clone() * f;
            }
            prop_assert!(objective.abs_diff_eq(&substituted.objective(), 1e-10));
        }

        #[test]
        fn test_pubo(instance in Instance::arbitrary_binary_unconstrained()) {
            if instance.sense() == Sense::Maximize {
                return Ok(());
            }
            let pubo = instance.as_pubo_format().unwrap();
            for (_, c) in pubo {
                prop_assert!(c.abs() > f64::EPSILON);
            }
        }

        #[test]
        fn test_qubo(instance in Instance::arbitrary_quadratic_binary_unconstrained()) {
            if instance.sense() == Sense::Maximize {
                return Ok(());
            }
            let (quad, _) = instance.as_qubo_format().unwrap();
            for (ids, c) in quad {
                prop_assert!(ids.0 <= ids.1);
                prop_assert!(c.abs() > f64::EPSILON);
            }
        }
    }
}
