use crate::SampleID;
use crate::{
    constraint_type::{dispatch::Query, DiagnosticRow},
    ConstraintRole, ConstraintType, SampleSet, SampledCollection, SampledConstraintBehavior,
};
use std::collections::BTreeMap;

impl SampleSet {
    /// Return one family's violated-constraint diagnostics for a single sample.
    ///
    /// Like [`crate::Solution::violation_diagnostics`], uses the saved tolerance and
    /// excludes feasible rows. Only the requested sample's violated rows generate
    /// details; this does not extract a whole Solution or cache the result.
    /// `EvaluationOnly` includes all retained rows, including skipped checks.
    ///
    /// # Errors
    /// An unknown sample produces [`crate::SampleSetError::UnknownSampleID`],
    /// including when the selected family has no rows.
    pub fn violation_diagnostics<C: ConstraintType>(
        &self,
        sample_id: SampleID,
        min_role: ConstraintRole,
    ) -> crate::Result<BTreeMap<C::ID, C::ViolationDiagnostics>> {
        crate::ensure!(
            self.objectives().get(sample_id).is_some(),
            crate::SampleSetError::UnknownSampleID { id: sample_id }
        );
        Ok(C::dispatch(DiagnosticsQuery {
            host: self,
            sample_id,
            min_role,
        }))
    }

    // Select rows from this owner; the accessor cannot capture another result's table.
    fn family_violation_diagnostics<C: ConstraintType>(
        &self,
        select: fn(&Self) -> &SampledCollection<C>,
        sample_id: SampleID,
        min_role: ConstraintRole,
    ) -> BTreeMap<C::ID, C::ViolationDiagnostics>
    where
        C::Sampled: DiagnosticRow<Diagnostics = C::ViolationDiagnostics>,
    {
        select(self)
            .selected(min_role)
            .filter(|(_, row)| {
                !row.is_feasible_for(sample_id, self.feasibility_atol())
                    .expect("validated sample IDs")
            })
            .map(|(&id, row)| {
                (
                    id,
                    row.diagnose(
                        |id| {
                            *self
                                .decision_variable_table()
                                .get(&id)
                                .expect("validated variable IDs")
                                .samples()
                                .get(sample_id)
                                .expect("validated sample IDs")
                        },
                        self.feasibility_atol(),
                    ),
                )
            })
            .collect()
    }
}

struct DiagnosticsQuery<'a> {
    host: &'a SampleSet,
    min_role: ConstraintRole,
    sample_id: SampleID,
}

macro_rules! impl_diagnostics_query {
    ((); $( $method:ident: $family:ty {
        created: $created:ident, evaluated: $evaluated:ident, sampled: $sampled:ident, $($rest:tt)*
    } )*) => {
        impl Query for DiagnosticsQuery<'_> {
            type Output<C: ConstraintType> = BTreeMap<C::ID, C::ViolationDiagnostics>;

            $(fn $method(self) -> Self::Output<$family> {
                self.host.family_violation_diagnostics::<$family>(
                    SampleSet::$sampled, self.sample_id, self.min_role,
                )
            })*
        }
    };
}

crate::constraint_type::families::with_constraint_families!(impl_diagnostics_query; ());
