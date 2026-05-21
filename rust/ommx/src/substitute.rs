use crate::{Evaluate, Function, VariableID};

mod assignments;
mod error;
mod macros;

pub use assignments::AcyclicAssignments;
pub use error::SubstitutionError;

/// A trait for substituting decision variables with other functions in mathematical expressions.
///
/// This trait enables the replacement of decision variables with arbitrary functions,
/// which is useful for optimization problems where some variables are dependent on others
/// or where you want to eliminate certain variables by expressing them in terms of others.
///
/// # Semantic Contract
///
/// Substitution is an algebraic rewrite. It replaces occurrences of the assigned
/// variable with the right-hand-side function and records the assignment for
/// dependent-variable reconstruction where the target type supports it. It does
/// not, by itself, convert the substituted variable's kind or bound into
/// additional constraints on the right-hand side.
///
/// For example, substituting a binary variable `x1` with `x2 + x3` does not
/// automatically add `0 <= x2 + x3 <= 1`. Likewise, substituting an integer
/// variable does not automatically enforce that the right-hand side is integral.
/// Callers that need an optimization-model-preserving transformation must ensure
/// those domain requirements themselves, either by constructing a
/// domain-preserving encoding (as in binary encodings) or by adding suitable
/// linking and bound constraints.
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
pub trait Substitute: Sized {
    type Output;

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
    fn substitute_acyclic(
        self,
        acyclic: &AcyclicAssignments,
    ) -> Result<Self::Output, SubstitutionError>;

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
    ) -> Result<Self::Output, SubstitutionError> {
        let acyclic = AcyclicAssignments::new(assignments)?;
        self.substitute_acyclic(&acyclic)
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
    ) -> Result<Self::Output, SubstitutionError>;
}

pub(crate) fn check_self_assignment(
    assigned: VariableID,
    f: &Function,
) -> Result<(), SubstitutionError> {
    if f.required_ids().contains(&assigned) {
        return Err(SubstitutionError::RecursiveAssignment { var_id: assigned });
    }
    Ok(())
}

/// In-place version of [`Substitute::substitute`].
///
/// This clones the current value, runs the consuming substitution API on the
/// clone, and commits the result back only on success. If substitution fails,
/// `substituted` is left unchanged. Use [`Substitute::substitute`] directly
/// when you want explicit control over cloning or ownership.
pub fn substitute<T>(
    substituted: &mut T,
    assignments: impl IntoIterator<Item = (VariableID, Function)>,
) -> Result<(), SubstitutionError>
where
    T: Substitute<Output = T> + Clone,
{
    let updated = substituted.clone().substitute(assignments)?;
    *substituted = updated;
    Ok(())
}

/// In-place version of [`Substitute::substitute_one`].
///
/// This clones the current value and commits the substituted value back only on
/// success. If substitution fails, `substituted` is left unchanged. Use
/// [`Substitute::substitute_one`] directly when you want explicit control over
/// cloning or ownership.
pub fn substitute_one<T>(
    substituted: &mut T,
    assigned: VariableID,
    linear: &Function,
) -> Result<(), SubstitutionError>
where
    T: Substitute<Output = T> + Clone,
{
    let updated = substituted.clone().substitute_one(assigned, linear)?;
    *substituted = updated;
    Ok(())
}

/// In-place version of [`Substitute::substitute_acyclic`].
///
/// This clones the current value and commits the substituted value back only on
/// success. If substitution fails, `substituted` is left unchanged. Use
/// [`Substitute::substitute_acyclic`] directly when you want explicit control
/// over cloning or ownership.
pub fn substitute_acyclic<T>(
    substituted: &mut T,
    acyclic: &AcyclicAssignments,
) -> Result<(), SubstitutionError>
where
    T: Substitute<Output = T> + Clone,
{
    let updated = substituted.clone().substitute_acyclic(acyclic)?;
    *substituted = updated;
    Ok(())
}

/// Default implementation of [`Substitute::substitute_acyclic`] using [`Substitute::substitute_one`].
pub(crate) fn substitute_acyclic_via_one<T, Output>(
    substituted: T,
    acyclic: &AcyclicAssignments,
) -> Result<Output, SubstitutionError>
where
    Output: From<T> + Substitute<Output = Output>,
{
    let mut out: Output = substituted.into();
    for (id, l) in acyclic.substitution_order_iter() {
        out = out.substitute_one(id, l)?;
    }
    Ok(out)
}

/// Default implementation of [`Substitute::substitute_one`] using [`Substitute::substitute_acyclic`].
pub(crate) fn substitute_one_via_acyclic<T: Substitute>(
    substituted: T,
    assigned: VariableID,
    f: &Function,
) -> Result<<T as Substitute>::Output, SubstitutionError> {
    let acyclic = AcyclicAssignments::new([(assigned, f.clone())])?;
    substituted.substitute_acyclic(&acyclic)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{coeff, linear};

    #[test]
    fn in_place_substitute_preserves_value_on_error() {
        let mut function = Function::from(linear!(1) + coeff!(2.0));
        let original = function.clone();

        let err = substitute(
            &mut function,
            [(
                VariableID::from(1),
                Function::from(linear!(1) + coeff!(1.0)),
            )],
        )
        .unwrap_err();

        assert!(matches!(
            err,
            SubstitutionError::RecursiveAssignment { var_id }
                if var_id == VariableID::from(1)
        ));
        assert!(function == original);
    }

    #[test]
    fn in_place_substitute_one_preserves_value_on_error() {
        let mut function = Function::from(linear!(1) + coeff!(2.0));
        let original = function.clone();

        let err = substitute_one(
            &mut function,
            VariableID::from(1),
            &Function::from(linear!(1) + coeff!(1.0)),
        )
        .unwrap_err();

        assert!(matches!(
            err,
            SubstitutionError::RecursiveAssignment { var_id }
                if var_id == VariableID::from(1)
        ));
        assert!(function == original);
    }
}
