use super::*;
use crate::random::{unique_integers, FunctionParameters};
use num::Zero;
use proptest::prelude::*;

impl Arbitrary for Linear {
    type Parameters = FunctionParameters;
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(p: Self::Parameters) -> Self::Strategy {
        p.validate().unwrap();
        assert!(
            p.can_be_linear(),
            "FunctionParameters ({p:?}) cannot be realized as a Linear",
        );
        if p.num_terms == 0 {
            return Just(Linear::zero()).boxed();
        }
        if p.max_degree == 0 {
            return Offset::arbitrary()
                .prop_map(|constant| constant.into())
                .boxed();
        }

        let constant = if p.num_terms == 1 + p.possible_max_linear_terms() {
            Coefficient::arbitrary()
                .prop_map(|constant| constant.into())
                .boxed()
        } else {
            Offset::arbitrary().boxed()
        };
        constant
            .prop_flat_map(move |constant| {
                let num_linear = if constant.abs() > f64::EPSILON {
                    p.num_terms - 1
                } else {
                    p.num_terms
                };
                let ids = unique_integers(0, p.max_id, num_linear);
                let coefficients = proptest::collection::vec(Coefficient::arbitrary(), num_linear);
                (ids, coefficients).prop_map(move |(ids, coefficients)| {
                    Linear::new(
                        ids.into_iter()
                            .map(VariableID::from)
                            .zip(coefficients)
                            .collect(),
                        constant,
                    )
                })
            })
            .boxed()
    }

    fn arbitrary() -> Self::Strategy {
        Self::Parameters {
            max_degree: 1,
            ..Default::default()
        }
        .smaller()
        .prop_flat_map(Self::arbitrary_with)
        .boxed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    proptest! {
        #[test]
        fn test_arbitrary_linear(l in Linear::arbitrary_with(FunctionParameters { num_terms: 5, max_degree: 1, max_id: 10 })) {
            let mut count = 0;
            for (ids, _) in l.into_iter() {
                for &id in ids.iter() {
                    prop_assert!(id <= 10.into());
                }
                count += 1;
            }
            prop_assert_eq!(count, 5);
        }

        #[test]
        fn test_max_num_terms(l in Linear::arbitrary_with(FunctionParameters { num_terms: 12, max_degree: 1, max_id: 10 })) {
            prop_assert_eq!(l.into_iter().count(), 12);
        }
    }
}
