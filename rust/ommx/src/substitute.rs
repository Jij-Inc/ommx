use crate::{decision_variable::VariableID, Coefficient, Function, Linear, Polynomial, Quadratic};
use fnv::{FnvHashMap, FnvHashSet};

pub type Assignments = FnvHashMap<VariableID, Function>;

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
/// on various mathematical expressions like `Function`, `Linear`, `Quadratic`,
/// `Polynomial`, and even higher-level structures like `Instance`.
pub trait Substitute: Clone {
    /// The type returned by the general `substitute` method.
    /// This allows for transformations where the resulting type might differ
    /// from the original (e.g., a `Linear` function becoming a `Quadratic`
    /// function after substitution, thus best represented as a `Function`).
    type Output;

    /// Substitutes variables in `self` using pre-classified assignment data.
    ///
    /// This is the primary method that implementers should provide, containing
    /// the core substitution logic that leverages the categorized assignments
    /// for potentially staged and optimized processing.
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
    /// This method has a default implementation that creates `ClassifiedAssignments`
    /// from the input `assignments` and then calls `substitute_classified`.
    fn substitute(&self, assignments: &Assignments) -> Self::Output {
        let classified = ClassifiedAssignments::from(assignments);
        self.substitute_classified(&classified)
    }

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
