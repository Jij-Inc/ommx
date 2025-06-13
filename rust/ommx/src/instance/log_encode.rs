use super::Instance;
use crate::{
    polynomial_base::{Linear, LinearMonomial},
    DecisionVariable, Kind, VariableID, Coefficient,
};
use anyhow::{bail, ensure, Context, Result};
use num::Zero;

impl Instance {
    /// Encode an integer decision variable into binary decision variables.
    ///
    /// This method performs logarithmic encoding of an integer variable into binary variables.
    /// For an integer variable with range [lower, upper], it creates ceil(log₂(range + 1)) binary
    /// variables where each binary variable represents a power of 2, allowing representation of
    /// any integer value in the original range.
    ///
    /// Note that this method does not substitute the yielded binary representation into the 
    /// objective and constraints. Call [`Instance::substitute`] with the returned [`Linear`] 
    /// representation to complete the encoding process.
    ///
    /// # Mutability
    /// 
    /// This method adds new binary decision variables introduced for binary encoding to the instance.
    ///
    /// # Errors
    /// 
    /// Returns [`anyhow::Error`] in the following cases:
    /// 
    /// - The given decision variable ID is not found
    /// - The specified decision variable is not an integer type
    /// - The bound of the decision variable is not set or not finite
    /// - No feasible integer values exist within the bounds
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ommx::{Instance, DecisionVariable, VariableID, Kind, Bound, Sense, Function};
    /// use std::collections::BTreeMap;
    /// 
    /// let mut decision_variables = BTreeMap::new();
    /// let var_id = VariableID::from(1);
    /// let mut var = DecisionVariable::new(
    ///     var_id,
    ///     Kind::Integer,
    ///     Bound::new(0.0, 7.0),  // Range [0, 7] requires 3 binary variables
    ///     None,
    ///     ommx::ATol::default(),
    /// ).unwrap();
    /// decision_variables.insert(var_id, var);
    /// 
    /// let mut instance = Instance::new(
    ///     Sense::Minimize,
    ///     Function::zero(),
    ///     decision_variables,
    ///     BTreeMap::new(),
    ///     Default::default(),
    /// ).unwrap();
    /// 
    /// // Encode the integer variable as binary variables
    /// let linear_repr = instance.log_encode(var_id.into()).unwrap();
    /// 
    /// // The instance now contains additional binary variables for the encoding
    /// assert!(instance.decision_variables().len() > 1);
    /// ```
    pub fn log_encode(&mut self, decision_variable_id: u64) -> Result<Linear> {
        let variable_id = VariableID::from(decision_variable_id);
        
        // Find the decision variable
        let v = self
            .decision_variables
            .get(&variable_id)
            .with_context(|| format!("Decision variable ID {} not found", decision_variable_id))?;
        
        // Check if it's an integer variable
        if v.kind() != Kind::Integer {
            bail!(
                "The decision variable is not an integer type: ID={}",
                decision_variable_id
            );
        }

        let bound = v.bound();

        // Check bounds are finite
        if !bound.lower().is_finite() || !bound.upper().is_finite() {
            bail!(
                "Bound must be finite for log-encoding: ID={}, lower={}, upper={}",
                decision_variable_id,
                bound.lower(),
                bound.upper()
            );
        }

        // Bound of integer may be non-integer value, so floor/ceil to get valid integer range
        let upper = bound.upper().floor();
        let lower = bound.lower().ceil();
        let u_l = upper - lower;
        
        ensure!(
            u_l >= 0.0,
            "No feasible integer found in the bound: ID={}, lower={}, upper={}",
            decision_variable_id,
            bound.lower(),
            bound.upper()
        );

        // There is only one feasible integer, and no need to encode
        if u_l == 0.0 {
            if lower == 0.0 {
                return Ok(Linear::zero());
            } else {
                return Ok(Linear::zero() + Coefficient::try_from(lower).unwrap());
            }
        }

        // Log-encoding: calculate number of binary variables needed
        let n = (u_l + 1.0).log2().ceil() as usize;
        
        // Find the next available ID by getting the maximum existing ID and adding 1
        let id_base = self
            .decision_variables
            .keys()
            .max()
            .map(|id| VariableID::from(id.into_inner() + 1))
            .unwrap_or_else(|| VariableID::from(0));

        let mut terms = Vec::new();
        
        // Create binary variables for log encoding
        for i in 0..n {
            let new_id = VariableID::from(id_base.into_inner() + i as u64);
            
            // Calculate coefficient for this binary variable
            let coeff = if i == n - 1 {
                // Last binary variable gets special coefficient to handle exact range
                u_l - 2.0f64.powi(i as i32) + 1.0
            } else {
                // Other variables get power of 2 coefficients
                2.0f64.powi(i as i32)
            };
            
            terms.push((new_id, coeff));
            
            // Create the binary decision variable with appropriate metadata
            let mut binary_var = DecisionVariable::binary(new_id);
            binary_var.name = Some("ommx.log_encode".to_string());
            binary_var.subscripts = vec![decision_variable_id as i64, i as i64];
            
            // Add the new binary variable to the instance
            self.decision_variables.insert(new_id, binary_var);
        }
        
        // Create the linear expression representing the original variable
        let mut linear = Linear::zero();
        
        // Add each binary variable term
        for (id, coeff_val) in terms {
            let monomial = LinearMonomial::Variable(id);
            let coeff = Coefficient::try_from(coeff_val).unwrap();
            linear = linear + (monomial * coeff);
        }
        
        // Add the constant offset
        if lower != 0.0 {
            linear = linear + Coefficient::try_from(lower).unwrap();
        }
        
        Ok(linear)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Bound, ConstraintHints, Function, Sense};
    use std::collections::BTreeMap;

    #[test]
    fn test_log_encode_basic() {
        let mut decision_variables = BTreeMap::new();
        let var_id = VariableID::from(1);
        
        // Create integer variable with range [0, 7] (8 values, needs 3 binary variables)
        let var = DecisionVariable::new(
            var_id,
            Kind::Integer,
            Bound::new(0.0, 7.0).unwrap(),
            None,
            crate::ATol::default(),
        ).unwrap();
        decision_variables.insert(var_id, var);

        let mut instance = Instance::new(
            Sense::Minimize,
            Function::Zero,
            decision_variables,
            BTreeMap::new(),
            ConstraintHints::default(),
        ).unwrap();

        // Should have 1 variable initially
        assert_eq!(instance.decision_variables().len(), 1);

        let linear_repr = instance.log_encode(1).unwrap();

        // Should now have 4 variables (1 original + 3 binary)
        assert_eq!(instance.decision_variables().len(), 4);

        // Check that binary variables were created correctly
        for id in 2..=4 {
            let var = instance.decision_variables().get(&VariableID::from(id)).unwrap();
            assert_eq!(var.kind(), Kind::Binary);
            assert_eq!(var.name, Some("ommx.log_encode".to_string()));
            assert_eq!(var.subscripts, vec![1, (id - 2) as i64]);
        }
    }

    #[test]
    fn test_log_encode_single_value() {
        let mut decision_variables = BTreeMap::new();
        let var_id = VariableID::from(1);
        
        // Create integer variable with range [5, 5] (single value)
        let var = DecisionVariable::new(
            var_id,
            Kind::Integer,
            Bound::new(5.0, 5.0).unwrap(),
            None,
            crate::ATol::default(),
        ).unwrap();
        decision_variables.insert(var_id, var);

        let mut instance = Instance::new(
            Sense::Minimize,
            Function::Zero,
            decision_variables,
            BTreeMap::new(),
            ConstraintHints::default(),
        ).unwrap();

        let linear_repr = instance.log_encode(1).unwrap();

        // Should still have only 1 variable (no binary variables added)
        assert_eq!(instance.decision_variables().len(), 1);

        // Linear representation should be just the constant value 5
        assert_eq!(linear_repr.constant_term(), 5.0);
        assert_eq!(linear_repr.num_terms(), 0);
    }

    #[test]
    fn test_log_encode_non_integer_bounds() {
        let mut decision_variables = BTreeMap::new();
        let var_id = VariableID::from(1);
        
        // Create integer variable with non-integer bounds [1.3, 7.8]
        // Should be treated as [2, 7] after ceil/floor
        let var = DecisionVariable::new(
            var_id,
            Kind::Integer,
            Bound::new(1.3, 7.8).unwrap(),
            None,
            crate::ATol::default(),
        ).unwrap();
        decision_variables.insert(var_id, var);

        let mut instance = Instance::new(
            Sense::Minimize,
            Function::Zero,
            decision_variables,
            BTreeMap::new(),
            ConstraintHints::default(),
        ).unwrap();

        let linear_repr = instance.log_encode(1).unwrap();

        // Range [2, 7] has 6 values (2,3,4,5,6,7), needs 3 binary variables
        assert_eq!(instance.decision_variables().len(), 4); // 1 + 3
    }

    #[test]
    fn test_log_encode_variable_not_found() {
        let mut instance = Instance::new(
            Sense::Minimize,
            Function::Zero,
            BTreeMap::new(),
            BTreeMap::new(),
            ConstraintHints::default(),
        ).unwrap();

        let result = instance.log_encode(999);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Decision variable ID 999 not found"));
    }

    #[test]
    fn test_log_encode_non_integer_variable() {
        let mut decision_variables = BTreeMap::new();
        let var_id = VariableID::from(1);
        
        // Create continuous variable instead of integer
        let var = DecisionVariable::continuous(var_id);
        decision_variables.insert(var_id, var);

        let mut instance = Instance::new(
            Sense::Minimize,
            Function::Zero,
            decision_variables,
            BTreeMap::new(),
            ConstraintHints::default(),
        ).unwrap();

        let result = instance.log_encode(1);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not an integer type"));
    }

    #[test]
    fn test_log_encode_infeasible_bounds() {
        let mut decision_variables = BTreeMap::new();
        let var_id = VariableID::from(1);
        
        // Create integer variable with infeasible bounds [5.8, 4.2]
        // After ceil/floor becomes [6, 4] which is infeasible
        let var = DecisionVariable::new(
            var_id,
            Kind::Integer,
            Bound::new(5.8, 4.2).unwrap(),
            None,
            crate::ATol::default(),
        ).unwrap();
        decision_variables.insert(var_id, var);

        let mut instance = Instance::new(
            Sense::Minimize,
            Function::Zero,
            decision_variables,
            BTreeMap::new(),
            ConstraintHints::default(),
        ).unwrap();

        let result = instance.log_encode(1);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("No feasible integer found"));
    }
}