use crate::{Linear, VariableID};

mod error;
mod linear_assignments;

pub use error::RecursiveAssignmentError;
pub use linear_assignments::AcyclicLinearAssignments;

/// A trait for types that can have their variables substituted exclusively with `Linear` functions.
///
/// This specialized substitution is often used when the degree of the expression
/// is expected not to increase, or to simplify expressions by replacing variables
/// with linear forms. The `Output` type allows for flexibility in the result,
/// for instance, a `Linear` function might become a `Constant` (represented as `Function`)
/// after substitution.
pub trait SubstituteWithLinears: Clone + Sized {
    /// The type returned by the `substitute_with_linears` method.
    type Output: From<Self> + SubstituteWithLinears<Output = Self::Output>;

    /// Substitutes variables in `self` exclusively with `Linear` functions using acyclic assignments.
    ///
    /// This is the primary method that implementers should provide. It takes
    /// an `AcyclicLinearAssignments` which guarantees no circular dependencies.
    ///
    /// # Arguments
    /// * `acyclic_assignments`: An `AcyclicLinearAssignments` containing the
    ///   linear functions to substitute, already validated to be acyclic.
    ///
    /// # Returns
    /// A new object of type `Self::Output` representing the expression after
    /// substitution with linear functions.
    fn substitute_with_linears_acyclic(
        self,
        acyclic_assignments: &AcyclicLinearAssignments,
    ) -> Self::Output {
        let mut out: Self::Output = self.into();
        for (id, l) in acyclic_assignments.sorted_iter() {
            out = out.substitute_with_linear(id, l).unwrap(); // Checked when creating `AcyclicLinearAssignments`
        }
        out
    }

    /// Substitutes variables in `self` exclusively with `Linear` functions.
    ///
    /// This method has a default implementation that creates an `AcyclicLinearAssignments`
    /// from the input iterator and calls `substitute_with_linears_acyclic`. If the
    /// assignments contain cycles, an error is returned.
    ///
    /// # Arguments
    /// * `linear_assignments`: An iterator of `(VariableID, Linear)` pairs representing
    ///   the variables to replace and their corresponding linear functions.
    ///
    /// # Returns
    /// A `Result` containing either:
    /// - `Ok(Self::Output)`: The expression after substitution with linear functions
    /// - `Err(RecursiveAssignmentError)`: If the assignments contain circular dependencies
    fn substitute_with_linears(
        self,
        linear_assignments: impl IntoIterator<Item = (VariableID, Linear)>,
    ) -> Result<Self::Output, RecursiveAssignmentError> {
        let acyclic_assignments = AcyclicLinearAssignments::new(linear_assignments)?;
        Ok(self.substitute_with_linears_acyclic(&acyclic_assignments))
    }

    fn substitute_with_linear(
        self,
        assigned: VariableID,
        linear: &Linear,
    ) -> Result<Self::Output, RecursiveAssignmentError>;
}
