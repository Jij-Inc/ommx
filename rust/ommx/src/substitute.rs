use crate::{decision_variable::VariableID, Function, Linear};
use fnv::FnvHashMap;

pub type Assignments = FnvHashMap<VariableID, Function>;

/// A trait for types that can have their variables substituted with functions.
///
/// This trait provides a common interface for performing substitution operations
/// on various mathematical expressions like `Function`, `Linear`, `Quadratic`,
/// `Polynomial`, and even higher-level structures like `Instance`.
pub trait Substitute: Clone {
    /// The type returned by the general `substitute` method.
    /// This allows for transformations where the resulting type might differ
    /// from the original (e.g., a `Linear` function becoming a `Quadratic`
    /// function after substitution, thus best represented as a `Function`).
    type Output;

    /// Substitutes variables in `self` with general functions specified in `assignments`.
    ///
    /// The substitution is performed "simultaneously" for all variables present
    /// as keys in the `assignments` map. If a variable in `self` is not
    /// present in `assignments`, it remains unchanged.
    ///
    /// # Arguments
    /// * `assignments`: A map from `VariableID` to the `Function` that should
    ///   replace it.
    ///
    /// # Returns
    /// A new object of type `Self::Output` representing the expression after
    /// substitution.
    fn substitute(&self, assignments: &Assignments) -> Self::Output;

    /// Substitutes variables in `self` exclusively with `Linear` functions.
    ///
    /// This specialized substitution guarantees that the degree of the expression
    /// will not increase. The structural type of `Self` is generally preserved
    /// (e.g., a `Linear` function remains `Linear`, a `Quadratic` remains `Quadratic`).
    /// If simplification occurs (e.g., a `Linear` function becomes a constant),
    /// the returned `Self` should represent this simplified form within its own type.
    ///
    /// # Arguments
    /// * `linear_assignments`: A map from `VariableID` to the `Linear` function
    ///   that should replace it.
    fn substitute_with_linears(&self, linear_assignments: &FnvHashMap<VariableID, Linear>) -> Self;
}
