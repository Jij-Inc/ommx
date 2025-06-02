use crate::{Function, VariableID};

mod assignments;
mod error;

pub use assignments::AcyclicAssignments;
pub use error::SubstitutionError;

/// Creates an [`AcyclicAssignments`] from assignment expressions.
///
/// This macro provides a convenient syntax for creating substitution assignments
/// using the syntax `assign! { var_id <- expression, ... }`. The macro validates
/// that the assignments are acyclic and returns an [`AcyclicAssignments`] object.
///
/// # Syntax
///
/// ```text
/// assign! {
///     var_id1 <- expression1,
///     var_id2 <- expression2,
///     ...
/// }
/// ```
///
/// Where:
/// - `var_id` is a literal integer representing the variable ID
/// - `expression` is any expression that can be converted to a [`Function`]
///
/// # Examples
///
/// Basic usage with linear expressions:
///
/// ```
/// use ommx::{assign, coeff, linear, Function};
///
/// // Create assignments: x1 <- x2 + 1, x2 <- x3 + 2
/// let assignments = assign! {
///     1 <- linear!(2) + coeff!(1.0),
///     2 <- linear!(3) + coeff!(2.0)
/// };
/// ```
///
/// Using with more complex expressions:
///
/// ```
/// use ommx::{assign, coeff, linear, Function};
///
/// // Create assignments with different expression types
/// let assignments = assign! {
///     1 <- coeff!(5.0),                           // Constant assignment
///     2 <- coeff!(2.0) * linear!(3) + coeff!(1.0), // Linear expression
///     4 <- linear!(5)                             // Simple variable assignment
/// };
/// ```
///
/// # Panics
///
/// This macro panics if:
/// - The assignments contain cycles (e.g., x1 <- x2, x2 <- x1)
/// - A variable is assigned to an expression containing itself (e.g., x1 <- x1 + 1)
///
/// # Note
///
/// For runtime creation of assignments where error handling is needed,
/// use [`AcyclicAssignments::new()`] directly:
///
/// ```
/// use ommx::{AcyclicAssignments, Function, VariableID, coeff, linear};
///
/// let assignments = vec![
///     (VariableID::from(1), Function::from(coeff!(5.0))),
///     (VariableID::from(2), Function::from(linear!(3) + coeff!(1.0))),
/// ];
///
/// match AcyclicAssignments::new(assignments) {
///     Ok(acyclic) => { /* use acyclic */ },
///     Err(err) => { /* handle error */ },
/// }
/// ```
#[macro_export]
macro_rules! assign {
    ( $( $var_id:literal <- $expr:expr ),* $(,)? ) => {
        {
            let assignments = vec![
                $(
                    ($crate::VariableID::from($var_id), $crate::Function::from($expr)),
                )*
            ];
            $crate::AcyclicAssignments::new(assignments).unwrap()
        }
    };
}

/// A trait for substituting decision variables with other functions in mathematical expressions.
///
/// This trait enables the replacement of decision variables with arbitrary functions,
/// which is useful for optimization problems where some variables are dependent on others
/// or where you want to eliminate certain variables by expressing them in terms of others.
///
/// # Example
///
/// Basic substitution of a variable in a linear function:
///
/// ```
/// use ommx::{Function, LinearMonomial, coeff, linear, VariableID, Substitute};
///
/// // Create f(x1, x2) = 2*x1 + 3*x2 + 1
/// let f = Function::from(coeff!(2.0) * linear!(1) + coeff!(3.0) * linear!(2) + coeff!(1.0));
///
/// // Substitute x1 = x3 + 5
/// let substitution = Function::from(
///     linear!(3) + coeff!(5.0)
/// );
///
/// let result = f.substitute_one(VariableID::from(1), &substitution).unwrap();
/// // Result: 2*(x3 + 5) + 3*x2 + 1 = 2*x3 + 3*x2 + 11
/// ```
///
/// # Error Handling
///
/// The substitution operations can fail in two main scenarios:
///
/// ## Self-Reference Error
///
/// When attempting to substitute a variable with an expression that contains the variable itself:
///
/// ```
/// use ommx::{Function, LinearMonomial, coeff, linear, VariableID, Substitute, SubstitutionError};
///
/// // Try to substitute x1 = x1 + 2 (illegal self-reference)
/// let x1 = Function::from(linear!(1));
/// let self_ref = Function::from(linear!(1) + coeff!(2.0));
///
/// let result = x1.substitute_one(VariableID::from(1), &self_ref);
/// assert!(matches!(result, Err(SubstitutionError::RecursiveAssignment { var_id }) if var_id == VariableID::from(1)));
/// ```
///
/// ## Cyclic Dependencies Error
///
/// When attempting to substitute variables with cyclic dependencies:
///
/// ```
/// use ommx::{Function, LinearMonomial, coeff, linear, VariableID, Substitute, SubstitutionError};
///
/// // Try to create cyclic substitution: x1 = x2 + 1, x2 = x1 + 2
/// let assignments = vec![
///     (VariableID::from(1), Function::from(linear!(2) + coeff!(1.0))),
///     (VariableID::from(2), Function::from(linear!(1) + coeff!(2.0))),
/// ];
///
/// let f = Function::from(linear!(1));
/// let result = f.substitute(assignments);
/// assert!(matches!(result, Err(SubstitutionError::CyclicAssignmentDetected)));
/// ```
///
/// # Complex Example with Multiple Substitutions
///
/// ```
/// use ommx::{Function, LinearMonomial, coeff, linear, VariableID, Substitute};
///
/// // Create f(x1, x2, x3) = x1 + 2*x2 + 3*x3
/// let f = Function::from(
///     linear!(1)
///     + coeff!(2.0) * linear!(2)
///     + coeff!(3.0) * linear!(3)
/// );
///
/// // Create substitutions: x1 = x4 + 1, x2 = 2*x4 + x5
/// let assignments = vec![
///     (
///         VariableID::from(1),
///         Function::from(linear!(4) + coeff!(1.0))
///     ),
///     (
///         VariableID::from(2),
///         Function::from(coeff!(2.0) * linear!(4) + linear!(5))
///     ),
/// ];
///
/// let result = f.substitute(assignments).unwrap();
/// // Result: (x4 + 1) + 2*(2*x4 + x5) + 3*x3 = 5*x4 + 2*x5 + 3*x3 + 1
/// ```
pub trait Substitute: Clone + Sized + Into<Function> {
    /// Performs substitution using pre-validated acyclic assignments.
    ///
    /// This method is more efficient than [`substitute`](Self::substitute) when you already
    /// have validated assignments that are guaranteed to be acyclic. The assignments are
    /// applied in topological order to ensure correctness.
    ///
    /// # Arguments
    ///
    /// * `acyclic` - Pre-validated assignments that are guaranteed to be free of cycles
    ///
    /// # Returns
    ///
    /// The resulting function after applying all substitutions
    ///
    /// # Example
    ///
    /// ```
    /// use ommx::{Function, LinearMonomial, coeff, linear, VariableID, Substitute, AcyclicAssignments};
    ///
    /// // Create f(x1, x2) = x1 + x2
    /// let f = Function::from(
    ///     linear!(1)
    ///     + linear!(2)
    /// );
    ///
    /// // Create acyclic assignments: x1 = x3 + 1, x2 = x4 + 2
    /// let assignments = vec![
    ///     (
    ///         VariableID::from(1),
    ///         Function::from(linear!(3) + coeff!(1.0))
    ///     ),
    ///     (
    ///         VariableID::from(2),
    ///         Function::from(linear!(4) + coeff!(2.0))
    ///     ),
    /// ];
    ///
    /// let acyclic = AcyclicAssignments::new(assignments).unwrap();
    /// let result = f.substitute_acyclic(&acyclic);
    /// // Result: (x3 + 1) + (x4 + 2) = x3 + x4 + 3
    /// ```
    fn substitute_acyclic(self, acyclic: &AcyclicAssignments) -> Function {
        let mut out: Function = self.into();
        for (id, l) in acyclic.sorted_iter() {
            out = out.substitute_one(id, l).unwrap(); // Checked when creating `AcyclicFunctionAssignments`
        }
        out
    }

    /// Performs substitution with cycle detection and validation.
    ///
    /// This method validates the assignments for cycles and self-references before
    /// applying them. If any cycles are detected, it returns an error.
    ///
    /// # Arguments
    ///
    /// * `assignments` - An iterable of (variable_id, function) pairs to substitute
    ///
    /// # Returns
    ///
    /// * `Ok(Function)` - The resulting function after applying all substitutions
    /// * `Err(SubstitutionError)` - If cycles or self-references are detected
    ///
    /// # Errors
    ///
    /// * [`SubstitutionError::CyclicAssignmentDetected`] - When cycles are found in assignments
    /// * [`SubstitutionError::RecursiveAssignment`] - When a variable references itself
    ///
    /// # Example
    ///
    /// ```
    /// use ommx::{Function, LinearMonomial, coeff, linear, VariableID, Substitute};
    ///
    /// let f = Function::from(linear!(1));
    ///
    /// // Valid substitution
    /// let assignments = vec![
    ///     (VariableID::from(1), Function::from(coeff!(5.0)))
    /// ];
    /// let result = f.substitute(assignments).unwrap();
    /// ```
    fn substitute(
        self,
        assignments: impl IntoIterator<Item = (VariableID, Function)>,
    ) -> Result<Function, SubstitutionError> {
        let acyclic = AcyclicAssignments::new(assignments)?;
        Ok(self.substitute_acyclic(&acyclic))
    }

    /// Substitutes a single variable with a function.
    ///
    /// This is the core method that must be implemented by types that implement this trait.
    /// It performs the actual substitution logic for replacing one variable with a function.
    ///
    /// # Arguments
    ///
    /// * `assigned` - The variable ID to be replaced
    /// * `linear` - The function to substitute for the variable
    ///
    /// # Returns
    ///
    /// * `Ok(Function)` - The resulting function after substitution
    /// * `Err(SubstitutionError::RecursiveAssignment)` - If the function contains the assigned variable
    ///
    /// # Example
    ///
    /// ```
    /// use ommx::{Function, coeff, linear, VariableID, Substitute};
    ///
    /// // f(x1) = 2*x1 + 3
    /// let f = Function::from(coeff!(2.0) * linear!(1) + coeff!(3.0));
    ///
    /// // Substitute x1 = x2 + 1
    /// let substitution = Function::from(linear!(2) + coeff!(1.0));
    ///
    /// let result = f.substitute_one(VariableID::from(1), &substitution).unwrap();
    /// // Result: 2*(x2 + 1) + 3 = 2*x2 + 5
    /// ```
    fn substitute_one(
        self,
        assigned: VariableID,
        linear: &Function,
    ) -> Result<Function, SubstitutionError>;
}

#[cfg(test)]
mod tests {
    use crate::{coeff, linear, Coefficient, Function, LinearMonomial, VariableID};

    #[test]
    fn test_assign_macro_basic() {
        // Test basic assignment: x1 <- x2 + 1, x2 <- x3 + 2
        let assignments = assign! {
            1 <- linear!(2) + coeff!(1.0),
            2 <- linear!(3) + coeff!(2.0)
        };

        // Verify the assignments were created correctly
        assert_eq!(assignments.len(), 2);

        // Check first assignment: x1 <- x2 + 1
        let assignment_1 = assignments.get(&VariableID::from(1)).unwrap();
        if let Function::Linear(l) = assignment_1 {
            // Should have two terms: x2 and constant
            assert_eq!(l.num_terms(), 2);

            // Check that we have the expected terms
            let has_x2 = l.iter().any(|(m, c)| {
                matches!(m, LinearMonomial::Variable(id) if *id == VariableID::from(2))
                    && *c == Coefficient::try_from(1.0).unwrap()
            });
            let has_constant = l.iter().any(|(m, c)| {
                matches!(m, LinearMonomial::Constant) && *c == Coefficient::try_from(1.0).unwrap()
            });
            assert!(has_x2, "Expected x2 term with coefficient 1.0");
            assert!(has_constant, "Expected constant term with coefficient 1.0");
        } else {
            panic!("Expected Linear function");
        }

        // Check second assignment: x2 <- x3 + 2
        let assignment_2 = assignments.get(&VariableID::from(2)).unwrap();
        if let Function::Linear(l) = assignment_2 {
            assert_eq!(l.num_terms(), 2);

            let has_x3 = l.iter().any(|(m, c)| {
                matches!(m, LinearMonomial::Variable(id) if *id == VariableID::from(3))
                    && *c == Coefficient::try_from(1.0).unwrap()
            });
            let has_constant = l.iter().any(|(m, c)| {
                matches!(m, LinearMonomial::Constant) && *c == Coefficient::try_from(2.0).unwrap()
            });
            assert!(has_x3, "Expected x3 term with coefficient 1.0");
            assert!(has_constant, "Expected constant term with coefficient 2.0");
        } else {
            panic!("Expected Linear function");
        }
    }

    #[test]
    fn test_assign_macro_constant() {
        // Test constant assignment: x1 <- 5.0
        let assignments = assign! {
            1 <- coeff!(5.0)
        };

        assert_eq!(assignments.len(), 1);
        let assignment = assignments.get(&VariableID::from(1)).unwrap();
        if let Function::Constant(c) = assignment {
            assert_eq!(*c, Coefficient::try_from(5.0).unwrap());
        } else {
            panic!("Expected Constant function");
        }
    }

    #[test]
    fn test_assign_macro_complex_expression() {
        // Test complex expression: x1 <- 2.0 * x2 + 3.0 * x3 + 1.0
        let assignments = assign! {
            1 <- coeff!(2.0) * linear!(2) + coeff!(3.0) * linear!(3) + coeff!(1.0)
        };

        assert_eq!(assignments.len(), 1);
        let assignment = assignments.get(&VariableID::from(1)).unwrap();
        if let Function::Linear(l) = assignment {
            assert_eq!(l.num_terms(), 3); // x2, x3, and constant

            // Check that we have the expected terms
            let has_x2 = l.iter().any(|(m, c)| {
                matches!(m, LinearMonomial::Variable(id) if *id == VariableID::from(2))
                    && *c == Coefficient::try_from(2.0).unwrap()
            });
            let has_x3 = l.iter().any(|(m, c)| {
                matches!(m, LinearMonomial::Variable(id) if *id == VariableID::from(3))
                    && *c == Coefficient::try_from(3.0).unwrap()
            });
            let has_constant = l.iter().any(|(m, c)| {
                matches!(m, LinearMonomial::Constant) && *c == Coefficient::try_from(1.0).unwrap()
            });
            assert!(has_x2, "Expected x2 term with coefficient 2.0");
            assert!(has_x3, "Expected x3 term with coefficient 3.0");
            assert!(has_constant, "Expected constant term with coefficient 1.0");
        } else {
            panic!("Expected Linear function");
        }
    }

    #[test]
    fn test_assign_macro_empty() {
        // Test empty assignment
        let assignments = assign! {};
        assert_eq!(assignments.len(), 0);
    }

    #[test]
    fn test_assign_macro_trailing_comma() {
        // Test with trailing comma
        let assignments = assign! {
            1 <- coeff!(5.0),
        };

        assert_eq!(assignments.len(), 1);
        let assignment = assignments.get(&VariableID::from(1)).unwrap();
        if let Function::Constant(c) = assignment {
            assert_eq!(*c, Coefficient::try_from(5.0).unwrap());
        } else {
            panic!("Expected Constant function");
        }
    }

    #[test]
    #[should_panic(expected = "CyclicAssignmentDetected")]
    fn test_assign_macro_cycle_detection() {
        // This should panic due to cycle: x1 <- x2, x2 <- x1
        let _assignments = assign! {
            1 <- linear!(2),
            2 <- linear!(1)
        };
    }

    #[test]
    #[should_panic(expected = "CyclicAssignmentDetected")]
    fn test_assign_macro_self_reference() {
        // This should panic due to self-reference: x1 <- x1 + 1
        let _assignments = assign! {
            1 <- linear!(1) + coeff!(1.0)
        };
    }
}
