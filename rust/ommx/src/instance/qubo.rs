use super::{Instance, Sense};
use crate::{BinaryIdPair, BinaryIds, Evaluate};
use anyhow::{bail, Result};
use std::collections::BTreeMap;

impl Instance {
    /// Create QUBO (Quadratic Unconstrained Binary Optimization) dictionary from the instance.
    ///
    /// Before calling this method, you should check that this instance is suitable for QUBO:
    ///
    /// - This instance has no constraints
    ///   - Use penalty method (TODO: ALM will be added) to convert into an unconstrained problem.
    /// - The objective function uses only binary decision variables.
    ///   - TODO: Binary encoding will be added.
    /// - The degree of the objective is at most 2.
    ///
    #[tracing::instrument(skip_all)]
    pub fn as_qubo_format(&self) -> Result<(BTreeMap<BinaryIdPair, f64>, f64)> {
        if self.sense() == Sense::Maximize {
            bail!("QUBO format is only for minimization problems.");
        }
        if !self.constraints().is_empty() {
            bail!("The instance still has constraints. Use penalty method or other way to translate into unconstrained problem first.");
        }
        let non_standard = self.required_capabilities();
        if !non_standard.is_empty() {
            bail!(
                "QUBO format does not support these constraint types: {non_standard:?}. Convert via penalty method or equivalent first."
            );
        }
        if !self
            .objective()
            .required_ids()
            .is_subset(&self.binary_ids())
        {
            bail!("The objective function uses non-binary decision variables.");
        }
        let mut constant = 0.0;
        let mut quad: BTreeMap<BinaryIdPair, f64> = BTreeMap::new();
        for (ids, coefficient) in self.objective().iter() {
            let c = coefficient.into_inner();
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
    /// Before calling this method, you should check that this instance is suitable for HUBO:
    ///
    /// - This instance has no constraints
    ///   - Use penalty method (TODO: ALM will be added) to convert into an unconstrained problem.
    /// - The objective function uses only binary decision variables.
    ///   - TODO: Binary encoding will be added.
    ///
    #[tracing::instrument(skip_all)]
    pub fn as_hubo_format(&self) -> Result<(BTreeMap<BinaryIds, f64>, f64)> {
        if self.sense() == Sense::Maximize {
            bail!("HUBO format is only for minimization problems.");
        }
        if !self.constraints().is_empty() {
            bail!("The instance still has constraints. Use penalty method or other way to translate into unconstrained problem first.");
        }
        let non_standard = self.required_capabilities();
        if !non_standard.is_empty() {
            bail!(
                "HUBO format does not support these constraint types: {non_standard:?}. Convert via penalty method or equivalent first."
            );
        }
        if !self
            .objective()
            .required_ids()
            .is_subset(&self.binary_ids())
        {
            bail!("The objective function uses non-binary decision variables.");
        }
        let mut constant = 0.0;
        let mut hubo: BTreeMap<BinaryIds, f64> = BTreeMap::new();
        for (ids, coefficient) in self.objective().iter() {
            let c = coefficient.into_inner();
            if c.abs() <= f64::EPSILON {
                continue;
            }
            if ids.is_empty() {
                constant += c;
            } else {
                let key = BinaryIds::from(ids);
                let value = hubo.entry(key.clone()).and_modify(|v| *v += c).or_insert(c);
                if value.abs() < f64::EPSILON {
                    hubo.remove(&key);
                }
            }
        }
        Ok((hubo, constant))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{coeff, linear, quadratic, DecisionVariable, Function, VariableID};
    use maplit::btreemap;
    use std::collections::BTreeMap;

    fn binary_vars(ids: impl IntoIterator<Item = u64>) -> BTreeMap<VariableID, DecisionVariable> {
        ids.into_iter()
            .map(|i| {
                let id = VariableID::from(i);
                (id, DecisionVariable::binary(id))
            })
            .collect()
    }

    #[test]
    fn qubo_from_quadratic_objective() {
        // min x1 + 2*x2 + 3*x1*x2 with binary x1, x2
        let objective = Function::from(linear!(1))
            + Function::from(coeff!(2.0) * linear!(2))
            + Function::from(coeff!(3.0) * quadratic!(1, 2));
        let instance = Instance::new(
            Sense::Minimize,
            objective,
            binary_vars([1, 2]),
            BTreeMap::new(),
        )
        .unwrap();

        let (quad, constant) = instance.as_qubo_format().unwrap();
        assert_eq!(constant, 0.0);
        assert_eq!(quad.get(&BinaryIdPair(1, 1)), Some(&1.0));
        assert_eq!(quad.get(&BinaryIdPair(2, 2)), Some(&2.0));
        assert_eq!(quad.get(&BinaryIdPair(1, 2)), Some(&3.0));
    }

    #[test]
    fn qubo_rejects_maximization() {
        let instance = Instance::new(
            Sense::Maximize,
            linear!(1).into(),
            binary_vars([1]),
            BTreeMap::new(),
        )
        .unwrap();
        let err = instance.as_qubo_format().unwrap_err();
        assert!(err.to_string().contains("minimization"));
    }

    #[test]
    fn qubo_rejects_instances_with_constraints() {
        let constraints = btreemap! {
            crate::ConstraintID::from(0) =>
                crate::Constraint::equal_to_zero(linear!(1).into()),
        };
        let instance = Instance::new(
            Sense::Minimize,
            linear!(1).into(),
            binary_vars([1]),
            constraints,
        )
        .unwrap();
        let err = instance.as_qubo_format().unwrap_err();
        assert!(err.to_string().contains("constraints"));
    }

    #[test]
    fn qubo_rejects_non_binary_decision_variables() {
        let mut dv = binary_vars([1]);
        let id = VariableID::from(2);
        dv.insert(id, DecisionVariable::integer(id));
        let instance = Instance::new(
            Sense::Minimize,
            (linear!(1) + linear!(2)).into(),
            dv,
            BTreeMap::new(),
        )
        .unwrap();
        let err = instance.as_qubo_format().unwrap_err();
        assert!(err.to_string().contains("non-binary"));
    }

    #[test]
    fn qubo_rejects_instances_with_one_hot_constraints() {
        use crate::{OneHotConstraint, OneHotConstraintID};
        use std::collections::BTreeSet;

        let mut instance = Instance::new(
            Sense::Minimize,
            linear!(1).into(),
            binary_vars([1, 2]),
            BTreeMap::new(),
        )
        .unwrap();
        let one_hot = OneHotConstraint::new(
            [VariableID::from(1), VariableID::from(2)]
                .into_iter()
                .collect::<BTreeSet<_>>(),
        );
        instance
            .one_hot_constraint_collection
            .active_mut()
            .insert(OneHotConstraintID::from(0), one_hot);

        let err = instance.as_qubo_format().unwrap_err();
        let msg = err.to_string() + " " + &err.root_cause().to_string();
        assert!(
            msg.contains("QUBO") || msg.contains("Unsupported"),
            "expected rejection message, got: {msg}"
        );
    }

    #[test]
    fn hubo_from_cubic_objective() {
        // min x1*x2*x3 + x1 with binary x1, x2, x3
        use crate::MonomialDyn;
        let cubic = crate::Polynomial::single_term(
            MonomialDyn::new(vec![
                VariableID::from(1),
                VariableID::from(2),
                VariableID::from(3),
            ]),
            coeff!(1.0),
        );
        let objective = Function::from(linear!(1)) + Function::from(cubic);
        let instance = Instance::new(
            Sense::Minimize,
            objective,
            binary_vars([1, 2, 3]),
            BTreeMap::new(),
        )
        .unwrap();

        let (hubo, constant) = instance.as_hubo_format().unwrap();
        assert_eq!(constant, 0.0);
        // Cubic term
        let cubic_key = BinaryIds::from(MonomialDyn::new(vec![
            VariableID::from(1),
            VariableID::from(2),
            VariableID::from(3),
        ]));
        assert_eq!(hubo.get(&cubic_key), Some(&1.0));
        // Linear term
        let linear_key = BinaryIds::from(MonomialDyn::new(vec![VariableID::from(1)]));
        assert_eq!(hubo.get(&linear_key), Some(&1.0));
    }
}
