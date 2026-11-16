//! Connect each evaluated family to the result builder inside the Instance owner.

use super::*;

macro_rules! impl_collection_evaluation {
    ((); $( $method:ident: $family:ty {
        created: $created:ident, evaluated: $evaluated:ident, sampled: $sampled:ident,
        evaluated_builder: $evaluated_builder:ident, sampled_builder: $sampled_builder:ident,
        $($rest:tt)*
    } )*) => {
        pub fn evaluate(
            instance: &Instance,
            state: &v1::State,
            atol: ATol,
        ) -> Result<crate::SolutionBuilder> {
            Ok(crate::Solution::builder()
                $(.$evaluated_builder(instance.$created.evaluate(state, atol)?))*)
        }

        pub fn evaluate_samples(
            instance: &Instance,
            samples: &crate::Sampled<v1::State>,
            atol: ATol,
        ) -> Result<crate::SampleSetBuilder> {
            Ok(crate::SampleSet::builder()
                $(.$sampled_builder(instance.$created.evaluate_samples(samples, atol)?))*)
        }
    };
}

crate::constraint_type::families::with_constraint_families!(impl_collection_evaluation; ());
