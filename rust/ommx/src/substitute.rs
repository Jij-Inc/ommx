use crate::{decision_variable::VariableID, Coefficient, Function, Linear, Polynomial, Quadratic};
use fnv::{FnvHashMap, FnvHashSet};

pub type Assignments = FnvHashMap<VariableID, Function>;
pub type LinearAssignments = FnvHashMap<VariableID, Linear>;

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
    /// Creates a `ClassifiedAssignments` instance by categorizing functions from an `Assignments` map.
    ///
    /// This method is an alternative to the `From<&'a Assignments>` implementation.
    pub fn classify(assignments: &'a Assignments) -> Self {
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

impl<'a> From<&'a Assignments> for ClassifiedAssignments<'a> {
    /// Converts an `&Assignments` map into `ClassifiedAssignments` by categorizing its functions.
    fn from(assignments: &'a Assignments) -> Self {
        ClassifiedAssignments::classify(assignments)
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
pub trait Substitute: Clone {
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
pub trait SubstituteWithLinears: Clone {
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
