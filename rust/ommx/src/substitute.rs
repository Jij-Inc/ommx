use crate::{
    decision_variable::VariableID, Coefficient, Evaluate, Function, Linear, Polynomial, Quadratic,
};
use fnv::{FnvHashMap, FnvHashSet};

/// Error indicating that a recursive assignment was attempted.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("Recursive assignment detected: variable {var_id} cannot be assigned to a function that depends on itself")]
pub struct RecursiveAssignmentError {
    pub var_id: VariableID,
}

/// Type-safe container for variable assignments that prevents recursive definitions.
///
/// This type prevents assignments like `x1 = x1 + x2` by checking that the function
/// being assigned to a variable does not require that variable's ID.
#[derive(Debug, Clone, PartialEq)]
pub struct Assignments {
    inner: FnvHashMap<VariableID, Function>,
}

impl Assignments {
    /// Creates a new empty assignments container.
    pub fn new() -> Self {
        Self {
            inner: FnvHashMap::default(),
        }
    }

    /// Attempts to insert a new assignment, ensuring no recursive dependencies.
    ///
    /// Returns an error if the function requires the variable being assigned to.
    pub fn insert(
        &mut self,
        var_id: VariableID,
        function: Function,
    ) -> Result<(), RecursiveAssignmentError> {
        if function.required_ids().contains(&var_id) {
            return Err(RecursiveAssignmentError { var_id });
        }
        self.inner.insert(var_id, function);
        Ok(())
    }

    /// Gets a reference to the function assigned to the given variable.
    pub fn get(&self, var_id: &VariableID) -> Option<&Function> {
        self.inner.get(var_id)
    }

    /// Returns an iterator over all variable-function pairs.
    pub fn iter(&self) -> impl Iterator<Item = (&VariableID, &Function)> {
        self.inner.iter()
    }

    /// Returns the number of assignments.
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Returns true if there are no assignments.
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Removes the assignment for the given variable.
    pub fn remove(&mut self, var_id: &VariableID) -> Option<Function> {
        self.inner.remove(var_id)
    }

    /// Returns true if the variable has an assignment.
    pub fn contains_key(&self, var_id: &VariableID) -> bool {
        self.inner.contains_key(var_id)
    }
}

impl Default for Assignments {
    fn default() -> Self {
        Self::new()
    }
}

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

/// Holds classified assignment data, borrowing from an original `Assignments` map.
///
/// This structure is used internally by the `Substitute` trait's default
/// `substitute` method to categorize assignments before performing the actual
/// substitution logic. This allows for potentially more efficient, staged
/// substitution.
#[derive(Debug)]
pub struct ClassifiedAssignments<'a> {
    pub(crate) zeros: FnvHashSet<VariableID>,
    pub(crate) constants: FnvHashMap<VariableID, Coefficient>,
    pub(crate) linears: FnvHashMap<VariableID, &'a Linear>,
    pub(crate) quadratics: FnvHashMap<VariableID, &'a Quadratic>,
    pub(crate) polynomials: FnvHashMap<VariableID, &'a Polynomial>,
}

impl<'a> ClassifiedAssignments<'a> {
    /// Creates a `ClassifiedAssignments` instance by categorizing functions from an `Assignments`.
    ///
    /// This method is an alternative to the `From<&'a Assignments>` implementation.
    pub fn classify(assignments: &'a Assignments) -> Self {
        Self::from(assignments)
    }
}

impl<'a> From<&'a Assignments> for ClassifiedAssignments<'a> {
    /// Converts an `&Assignments` into `ClassifiedAssignments` by categorizing its functions.
    fn from(assignments: &'a Assignments) -> Self {
        let mut classified = ClassifiedAssignments {
            zeros: FnvHashSet::default(),
            constants: FnvHashMap::default(),
            linears: FnvHashMap::default(),
            quadratics: FnvHashMap::default(),
            polynomials: FnvHashMap::default(),
        };

        for (var_id, func_to_assign) in assignments.iter() {
            match func_to_assign {
                Function::Zero => {
                    classified.zeros.insert(*var_id);
                }
                Function::Constant(c) => {
                    classified.constants.insert(*var_id, *c);
                }
                Function::Linear(l) => {
                    classified.linears.insert(*var_id, l);
                }
                Function::Quadratic(q) => {
                    classified.quadratics.insert(*var_id, q);
                }
                Function::Polynomial(p) => {
                    classified.polynomials.insert(*var_id, p);
                }
            }
        }
        classified
    }
}

/// A trait for types that can have their variables substituted with functions.
///
/// This trait provides a common interface for performing substitution operations
/// on various mathematical expressions within the `ommx` crate, such as
/// `Function` (and its variants `Linear`, `Quadratic`, `Polynomial`),
/// and potentially higher-level structures like `Instance`.
///
/// The primary method to implement is `substitute_classified`, which takes
/// pre-categorized assignment data for potentially optimized processing.
/// The `substitute` method, which takes a general `Assignments` map,
/// has a default implementation that uses `substitute_classified`.
pub trait Substitute {
    /// The type returned by the general `substitute` method.
    /// This allows for transformations where the resulting type might differ
    /// from the original. For example:
    /// - If `Self` is `Linear` and a `Quadratic` function is substituted into it,
    ///   the `Output` will likely be `Function` (specifically, `Function::Quadratic`).
    /// - If `Self` is `Function`, the `Output` will also be `Function`.
    /// - If `Self` is `Instance`, the `Output` will be `Instance`.
    /// function after substitution, thus best represented as a `Function`).
    type Output;

    /// Substitutes variables in `self` using pre-classified assignment data.
    ///
    /// This is the primary method that implementers should provide, containing
    /// the core substitution logic that leverages the categorized assignments
    /// (e.g., handling zeros, constants, linears, and higher-order functions
    /// in distinct, optimized stages).
    ///
    /// # Arguments
    /// * `classified_assignments`: A reference to `ClassifiedAssignments`
    ///   containing categorized functions to substitute.
    ///
    /// # Returns
    /// A new object of type `Self::Output` representing the expression after
    /// substitution.
    fn substitute_classified(&self, classified_assignments: &ClassifiedAssignments)
        -> Self::Output;

    /// Substitutes variables in `self` with general functions specified in an `Assignments` map.
    ///
    /// The substitution is performed "simultaneously" for all variables present
    /// as keys in the `assignments` map. If a variable in `self` is not
    /// present in `assignments`, it remains unchanged.
    ///
    /// This method has a default implementation that first converts the input
    /// `assignments` into `ClassifiedAssignments` and then calls
    /// `substitute_classified`. Implementers typically only need to provide
    /// `substitute_classified`.
    ///
    fn substitute(&self, assignments: &Assignments) -> Self::Output {
        let classified = ClassifiedAssignments::from(assignments);
        self.substitute_classified(&classified)
    }
}

/// A trait for types that can have their variables substituted exclusively with `Linear` functions.
///
/// This specialized substitution is often used when the degree of the expression
/// is expected not to increase, or to simplify expressions by replacing variables
/// with linear forms. The `Output` type allows for flexibility in the result,
/// for instance, a `Linear` function might become a `Constant` (represented as `Function`)
/// after substitution.
pub trait SubstituteWithLinears {
    /// The type returned by the `substitute_with_linears` method.
    type Output;

    /// Substitutes variables in `self` exclusively with `Linear` functions.
    ///
    /// # Arguments
    /// * `linear_assignments`: A map from `VariableID` to the `Linear` function
    ///   that should replace it.
    ///
    /// # Returns
    /// A new object of type `Self::Output` representing the expression after
    /// substitution with linear functions.
    fn substitute_with_linears(&self, linear_assignments: &LinearAssignments) -> Self::Output;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{LinearMonomial, PolynomialBase};

    #[test]
    fn test_assignments_prevents_recursive_assignment() {
        let mut assignments = Assignments::new();

        // Create a linear function x1 + 2
        let x1_id = VariableID::from(1);
        let linear_func = PolynomialBase::single_term(
            LinearMonomial::Variable(x1_id),
            Coefficient::try_from(1.0).unwrap(),
        ) + PolynomialBase::from(Coefficient::try_from(2.0).unwrap());

        // Try to assign x1 = x1 + 2 (should fail)
        let result = assignments.insert(x1_id, Function::Linear(linear_func));
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().var_id, x1_id);
    }

    #[test]
    fn test_assignments_allows_non_recursive_assignment() {
        let mut assignments = Assignments::new();

        // Create a linear function x2 + 3
        let x1_id = VariableID::from(1);
        let x2_id = VariableID::from(2);
        let linear_func = PolynomialBase::single_term(
            LinearMonomial::Variable(x2_id),
            Coefficient::try_from(1.0).unwrap(),
        ) + PolynomialBase::from(Coefficient::try_from(3.0).unwrap());

        // Assign x1 = x2 + 3 (should succeed)
        let result = assignments.insert(x1_id, Function::Linear(linear_func));
        assert!(result.is_ok());
        assert_eq!(assignments.len(), 1);
        assert!(assignments.contains_key(&x1_id));
    }

    #[test]
    fn test_assignments_allows_constant_assignment() {
        let mut assignments = Assignments::new();

        let x1_id = VariableID::from(1);
        let constant_func = Function::Constant(Coefficient::try_from(5.0).unwrap());

        // Assign x1 = 5 (should succeed)
        let result = assignments.insert(x1_id, constant_func);
        assert!(result.is_ok());
        assert_eq!(assignments.len(), 1);
    }

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
    fn test_assignments_basic_operations() {
        let mut assignments = Assignments::new();
        let x1_id = VariableID::from(1);
        let constant_func = Function::Constant(Coefficient::try_from(5.0).unwrap());

        // Test insertion and retrieval
        assignments.insert(x1_id, constant_func.clone()).unwrap();
        assert_eq!(assignments.get(&x1_id), Some(&constant_func));

        // Test removal
        let removed = assignments.remove(&x1_id);
        assert_eq!(removed, Some(constant_func));
        assert!(assignments.is_empty());
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

    #[test]
    fn test_classified_assignments_with_new_type() {
        let mut assignments = Assignments::new();

        let x1_id = VariableID::from(1);
        let x2_id = VariableID::from(2);
        let x3_id = VariableID::from(3);

        // Add various types of functions
        assignments.insert(x1_id, Function::Zero).unwrap();
        assignments
            .insert(
                x2_id,
                Function::Constant(Coefficient::try_from(5.0).unwrap()),
            )
            .unwrap();

        let linear_func = PolynomialBase::single_term(
            LinearMonomial::Variable(x1_id),
            Coefficient::try_from(2.0).unwrap(),
        );
        assignments
            .insert(x3_id, Function::Linear(linear_func))
            .unwrap();

        let classified = ClassifiedAssignments::from(&assignments);

        assert!(classified.zeros.contains(&x1_id));
        assert!(classified.constants.contains_key(&x2_id));
        assert!(classified.linears.contains_key(&x3_id));
        assert!(classified.quadratics.is_empty());
        assert!(classified.polynomials.is_empty());
    }
}
