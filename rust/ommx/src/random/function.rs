use crate::{
    random::{
        arbitrary_coefficient_nonzero, multi_choose, LinearParameters, PolynomialParameters,
        QuadraticParameters,
    },
    v1::{Function, Linear, Polynomial, Quadratic},
};
use num::Zero;
use proptest::{prelude::*, strategy::Union};

pub type FunctionParameters = PolynomialParameters;

impl Arbitrary for Function {
    type Parameters = FunctionParameters;
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(p: Self::Parameters) -> Self::Strategy {
        p.validate().unwrap();
        let mut strategies = Vec::new();
        if p.num_terms == 0 {
            strategies.push(Just(Function::zero()).boxed());
        }
        if p.num_terms == 1 {
            strategies.push(
                arbitrary_coefficient_nonzero()
                    .prop_map(|c| Function::from(c))
                    .boxed(),
            );
        }
        let mut threshold = multi_choose(p.max_id + 1, 1) as usize;
        if p.num_terms <= threshold {
            strategies.push(
                Linear::arbitrary_with(LinearParameters {
                    num_terms: p.num_terms,
                    max_id: p.max_id,
                })
                .prop_map(Function::from)
                .boxed(),
            )
        }
        threshold += multi_choose(p.max_id + 1, 2) as usize;
        if p.num_terms <= threshold {
            strategies.push(
                Quadratic::arbitrary_with(QuadraticParameters {
                    num_terms: p.num_terms,
                    max_id: p.max_id,
                })
                .prop_map(Function::from)
                .boxed(),
            )
        }
        strategies.push(
            Polynomial::arbitrary_with(p)
                .prop_map(Function::from)
                .boxed(),
        );
        Union::new(strategies).boxed()
    }

    fn arbitrary() -> Self::Strategy {
        Self::Parameters::default()
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
        fn test_arbitrary_function(f in Function::arbitrary_with(FunctionParameters { num_terms: 5, max_degree: 3, max_id: 10 })) {
            let mut count = 0;
            for (ids, _) in f.into_iter() {
                prop_assert!(ids.len() <= 3);
                for &id in ids.iter() {
                    prop_assert!(id <= 10);
                }
                count += 1;
            }
            prop_assert_eq!(count, 5);
        }
    }
}
