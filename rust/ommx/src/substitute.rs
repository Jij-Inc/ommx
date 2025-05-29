use crate::{Function, VariableID};

mod assignments;
mod error;

pub use assignments::AcyclicAssignments;
pub use error::{RecursiveAssignmentError, SubstitutionError};

/// A trait for types that can have their variables substituted exclusively with `Function` functions.
///
/// This specialized substitution is often used when the degree of the expression
/// is expected not to increase, or to simplify expressions by replacing variables
/// with linear forms. The `Output` type allows for flexibility in the result,
/// for instance, a `Function` function might become a `Constant` (represented as `Function`)
/// after substitution.
pub trait Substitute: Clone + Sized + Into<Function> {
    /// Substitutes variables in `self` exclusively with `Function` functions using acyclic assignments.
    ///
    /// This is the primary method that implementers should provide. It takes
    /// an `AcyclicFunctionAssignments` which guarantees no circular dependencies.
    ///
    /// # Arguments
    /// * `acyclic_assignments`: An `AcyclicAssignments` containing the
    ///   linear functions to substitute, already validated to be acyclic.
    ///
    /// # Returns
    /// A new object of type `Self::Output` representing the expression after
    /// substitution with linear functions.
    fn substitute_acyclic(self, acyclic: &AcyclicAssignments) -> Function {
        let mut out: Function = self.into();
        for (id, l) in acyclic.sorted_iter() {
            out = out.substitute_one(id, l).unwrap(); // Checked when creating `AcyclicFunctionAssignments`
        }
        out
    }

    /// Substitutes variables in `self` exclusively with `Function` functions.
    ///
    /// This method has a default implementation that creates an `AcyclicFunctionAssignments`
    /// from the input iterator and calls `substitute_with_linears_acyclic`. If the
    /// assignments contain cycles, an error is returned.
    ///
    /// # Arguments
    /// * `linear_assignments`: An iterator of `(VariableID, Function)` pairs representing
    ///   the variables to replace and their corresponding linear functions.
    ///
    /// # Returns
    /// A `Result` containing either:
    /// - `Ok(Self::Output)`: The expression after substitution with linear functions
    /// - `Err(SubstitutionError)`: If the assignments contain circular dependencies
    fn substitute(
        self,
        assignments: impl IntoIterator<Item = (VariableID, Function)>,
    ) -> Result<Function, SubstitutionError> {
        let acyclic = AcyclicAssignments::new(assignments)?;
        Ok(self.substitute_acyclic(&acyclic))
    }

    fn substitute_one(
        self,
        assigned: VariableID,
        linear: &Function,
    ) -> Result<Function, SubstitutionError>;
}
