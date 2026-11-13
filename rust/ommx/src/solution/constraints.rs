use crate::{
    constraint_type::{dispatch::Query, DiagnosticRow},
    Constraint, ConstraintRole, ConstraintType, EvaluatedCollection, EvaluatedConstraintBehavior,
    IndicatorConstraint, OneHotConstraint, Solution, Sos1Constraint,
};
use std::collections::BTreeMap;

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

    // Select rows from this owner; the accessor cannot capture another result's table.
    fn family_violation_diagnostics<C: ConstraintType>(
        &self,
        select: fn(&Self) -> &EvaluatedCollection<C>,
        min_role: ConstraintRole,
    ) -> BTreeMap<C::ID, C::ViolationDiagnostics>
    where
        C::Evaluated: DiagnosticRow<Diagnostics = C::ViolationDiagnostics>,
    {
        select(self)
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
    ($method:ident, $family:ty, $accessor:ident) => {
        fn $method(self) -> Self::Output<$family> {
            self.host
                .family_violation_diagnostics::<$family>(Solution::$accessor, self.min_role)
        }
    };
}

impl Query for DiagnosticsQuery<'_> {
    type Output<C: ConstraintType> = BTreeMap<C::ID, C::ViolationDiagnostics>;

    query_family!(regular, Constraint, evaluated_constraints);
    query_family!(
        indicator,
        IndicatorConstraint,
        evaluated_indicator_constraints
    );
    query_family!(one_hot, OneHotConstraint, evaluated_one_hot_constraints);
    query_family!(sos1, Sos1Constraint, evaluated_sos1_constraints);
}

#[cfg(test)]
mod tests;
