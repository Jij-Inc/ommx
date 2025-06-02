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
        $crate::AcyclicAssignments::new([
            $(
                ($crate::VariableID::from($var_id), $crate::Function::from($expr)),
            )*
        ]).unwrap()
    };
}

#[cfg(test)]
mod tests {
    use crate::{coeff, linear};

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
    #[should_panic(expected = "RecursiveAssignment")]
    fn test_assign_macro_self_reference() {
        // This should panic due to self-reference: x1 <- x1 + 1
        let _assignments = assign! {
            1 <- linear!(1) + coeff!(1.0)
        };
    }
}
