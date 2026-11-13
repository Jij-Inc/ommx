use crate::{
    constraint_type::{dispatch::Query, DiagnosticRow},
    Constraint, ConstraintRole, ConstraintType, EvaluatedCollection, EvaluatedConstraintBehavior,
    IndicatorConstraint, OneHotConstraint, Solution, Sos1Constraint,
};
use std::collections::BTreeMap;

// Storage access is private to this result owner. The public query dispatches
// over SDK families without requiring callers to name a host trait.
trait EvaluatedConstraintHost<C: ConstraintType> {
    fn evaluated_constraints(&self) -> &EvaluatedCollection<C>;
}

macro_rules! impl_host {
    ($family:ty, $accessor:ident) => {
        impl EvaluatedConstraintHost<$family> for Solution {
            fn evaluated_constraints(&self) -> &EvaluatedCollection<$family> {
                self.$accessor()
            }
        }
    };
}
impl_host!(Constraint, evaluated_constraints);
impl_host!(IndicatorConstraint, evaluated_indicator_constraints);
impl_host!(OneHotConstraint, evaluated_one_hot_constraints);
impl_host!(Sos1Constraint, evaluated_sos1_constraints);

impl Solution {
    /// Return diagnostics for this family's violated constraints at the saved tolerance.
    ///
    /// Rows at or above `min_role` are selected; use `EvaluationOnly` to include
    /// all retained rows. Feasible rows are absent. Regular and Indicator rows
    /// have dedicated empty diagnostic structs, but still appear when violated.
    /// Each family's associated type defines its details, so new families need no shared enum.
    ///
    /// Only violated rows generate details, using the stored, canonicalized
    /// variable values. Scalar violations stay stored; diagnostics are not cached.
    /// An empty map means every selected constraint in this family is feasible,
    /// not that variable domains or other families are feasible.
    ///
    /// ```
    /// # use ommx::{Solution, Constraint, OneHotConstraint, ConstraintRole};
    /// # fn inspect(solution: &Solution) {
    /// let regular = solution.violation_diagnostics::<Constraint>(ConstraintRole::EvaluationOnly);
    /// let one_hot = solution.violation_diagnostics::<OneHotConstraint>(ConstraintRole::EvaluationOnly);
    /// # }
    /// ```
    pub fn violation_diagnostics<C: ConstraintType>(
        &self,
        min_role: ConstraintRole,
    ) -> BTreeMap<C::ID, C::ViolationDiagnostics> {
        C::dispatch(DiagnosticsQuery {
            host: self,
            min_role,
        })
    }

    fn family_violation_diagnostics<C: ConstraintType>(
        &self,
        min_role: ConstraintRole,
    ) -> BTreeMap<C::ID, C::ViolationDiagnostics>
    where
        Self: EvaluatedConstraintHost<C>,
        C::Evaluated: DiagnosticRow<Diagnostics = C::ViolationDiagnostics>,
    {
        <Self as EvaluatedConstraintHost<C>>::evaluated_constraints(self)
            .selected(min_role)
            .filter(|(_, row)| !row.is_feasible(self.feasibility_atol()))
            .map(|(&id, row)| {
                (
                    id,
                    row.diagnose(
                        |id| {
                            *self
                                .decision_variable_table()
                                .get(&id)
                                .expect("validated variable IDs")
                                .value()
                        },
                        self.feasibility_atol(),
                    ),
                )
            })
            .collect()
    }
}

struct DiagnosticsQuery<'a> {
    host: &'a Solution,
    min_role: ConstraintRole,
}

macro_rules! query_family {
    ($method:ident, $family:ty) => {
        fn $method(self) -> Self::Output<$family> {
            self.host
                .family_violation_diagnostics::<$family>(self.min_role)
        }
    };
}

impl Query for DiagnosticsQuery<'_> {
    type Output<C: ConstraintType> = BTreeMap<C::ID, C::ViolationDiagnostics>;

    query_family!(regular, Constraint);
    query_family!(indicator, IndicatorConstraint);
    query_family!(one_hot, OneHotConstraint);
    query_family!(sos1, Sos1Constraint);
}

#[cfg(test)]
mod tests;
