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
