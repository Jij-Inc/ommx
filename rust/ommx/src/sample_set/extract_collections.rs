//! Project sampled collections while retaining their membership and sidecars.

use super::*;

fn extract<C: ConstraintType>(
    collection: &SampledCollection<C>,
    sample_id: SampleID,
) -> Option<EvaluatedCollection<C>> {
    let rows = collection
        .iter()
        .map(|(&id, row)| Some((id, row.get(sample_id)?)))
        .collect::<Option<BTreeMap<_, _>>>()?;
    Some(
        EvaluatedCollection::with_context(
            rows,
            collection.inactive_metadata().clone(),
            collection.context().clone(),
        )
        .expect("SampleSet sidecars must reference constraints present in this sample"),
    )
}

macro_rules! define_extraction {
    ((); $( $method:ident: $family:ty {
        created: $created:ident, evaluated: $evaluated:ident, sampled: $sampled:ident,
        evaluated_builder: $evaluated_builder:ident, sampled_builder: $sampled_builder:ident,
        $($rest:tt)*
    } )*) => {
        pub fn extract_collections(
            samples: &SampleSet,
            sample_id: SampleID,
        ) -> Option<crate::SolutionBuilder> {
            Some(Solution::builder()
                $(.$evaluated_builder(extract(&samples.$sampled, sample_id)?))*)
        }
    };
}

crate::constraint_type::families::with_constraint_families!(define_extraction; ());
