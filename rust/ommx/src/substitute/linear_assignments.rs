use crate::{decision_variable::VariableID, Evaluate, Linear};
use fnv::FnvHashMap;

use super::error::RecursiveAssignmentError;

/// Type-safe container for linear variable assignments that prevents recursive definitions.
///
/// This type prevents assignments like `x1 = x1 + x2` by checking that the linear function
/// being assigned to a variable does not require that variable's ID.
#[derive(Debug, Clone, PartialEq)]
pub struct LinearAssignments {
    inner: FnvHashMap<VariableID, Linear>,
}

impl LinearAssignments {
    /// Creates a new empty linear assignments container.
    pub fn new() -> Self {
        Self {
            inner: FnvHashMap::default(),
        }
    }

    /// Attempts to insert a new linear assignment, ensuring no recursive dependencies.
    ///
    /// Returns an error if the linear function requires the variable being assigned to.
    pub fn insert(
        &mut self,
        var_id: VariableID,
        linear: Linear,
    ) -> Result<(), RecursiveAssignmentError> {
        if linear.required_ids().contains(&var_id) {
            return Err(RecursiveAssignmentError { var_id });
        }
        self.inner.insert(var_id, linear);
        Ok(())
    }

    /// Gets a reference to the linear function assigned to the given variable.
    pub fn get(&self, var_id: &VariableID) -> Option<&Linear> {
        self.inner.get(var_id)
    }

    /// Returns an iterator over all variable-linear function pairs.
    pub fn iter(&self) -> impl Iterator<Item = (&VariableID, &Linear)> {
        self.inner.iter()
    }

    /// Returns the number of linear assignments.
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Returns true if there are no linear assignments.
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Removes the linear assignment for the given variable.
    pub fn remove(&mut self, var_id: &VariableID) -> Option<Linear> {
        self.inner.remove(var_id)
    }

    /// Returns true if the variable has a linear assignment.
    pub fn contains_key(&self, var_id: &VariableID) -> bool {
        self.inner.contains_key(var_id)
    }
}

impl Default for LinearAssignments {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Coefficient, LinearMonomial, PolynomialBase};

    #[test]
    fn test_linear_assignments_prevents_recursive_assignment() {
        let mut assignments = LinearAssignments::new();

        // Create a linear function x1 + 2
        let x1_id = VariableID::from(1);
        let linear_func = PolynomialBase::single_term(
            LinearMonomial::Variable(x1_id),
            Coefficient::try_from(1.0).unwrap(),
        ) + PolynomialBase::from(Coefficient::try_from(2.0).unwrap());

        // Try to assign x1 = x1 + 2 (should fail)
        let result = assignments.insert(x1_id, linear_func);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().var_id, x1_id);
    }

    #[test]
    fn test_linear_assignments_allows_non_recursive_assignment() {
        let mut assignments = LinearAssignments::new();

        // Create a linear function x2 + 3
        let x1_id = VariableID::from(1);
        let x2_id = VariableID::from(2);
        let linear_func = PolynomialBase::single_term(
            LinearMonomial::Variable(x2_id),
            Coefficient::try_from(1.0).unwrap(),
        ) + PolynomialBase::from(Coefficient::try_from(3.0).unwrap());

        // Assign x1 = x2 + 3 (should succeed)
        let result = assignments.insert(x1_id, linear_func);
        assert!(result.is_ok());
        assert_eq!(assignments.len(), 1);
        assert!(assignments.contains_key(&x1_id));
    }

    #[test]
    fn test_linear_assignments_basic_operations() {
        let mut assignments = LinearAssignments::new();
        let x1_id = VariableID::from(1);
        let x2_id = VariableID::from(2);
        let linear_func = PolynomialBase::single_term(
            LinearMonomial::Variable(x2_id),
            Coefficient::try_from(1.0).unwrap(),
        );

        // Test insertion and retrieval
        assignments.insert(x1_id, linear_func.clone()).unwrap();
        assert_eq!(assignments.get(&x1_id), Some(&linear_func));

        // Test removal
        let removed = assignments.remove(&x1_id);
        assert_eq!(removed, Some(linear_func));
        assert!(assignments.is_empty());
    }
}
