use crate::{
    random::{arbitrary_coefficient, LinearParameters, PolynomialParameters, QuadraticParameters},
    v1::{Function, Linear, Polynomial, Quadratic},
};
use proptest::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunctionParameters {
    pub num_terms: usize,
    pub max_degree: u32,
    pub max_id: u64,
}

impl Default for FunctionParameters {
    fn default() -> Self {
        Self {
            num_terms: 5,
            max_degree: 2,
            max_id: 10,
        }
    }
}

impl Arbitrary for Function {
    type Parameters = FunctionParameters;
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(
        FunctionParameters {
            num_terms,
            max_degree,
            max_id,
        }: Self::Parameters,
    ) -> Self::Strategy {
        let linear = if max_degree >= 1 {
            Linear::arbitrary_with(LinearParameters { num_terms, max_id })
        } else {
            arbitrary_coefficient().prop_map(Linear::from).boxed()
        };
        let quad = if max_degree >= 2 {
            Quadratic::arbitrary_with(QuadraticParameters { num_terms, max_id })
        } else {
            linear.clone().prop_map(Quadratic::from).boxed()
        };
        prop_oneof![
            arbitrary_coefficient().prop_map(Function::from),
            linear.prop_map(Function::from),
            quad.prop_map(Function::from),
            Polynomial::arbitrary_with(PolynomialParameters {
                num_terms,
                max_degree,
                max_id
            })
            .prop_map(Function::from),
        ]
        .boxed()
    }

    fn arbitrary() -> Self::Strategy {
        let FunctionParameters {
            num_terms,
            max_degree,
            max_id,
        } = Self::Parameters::default();
        (0..=num_terms, 0..=max_degree, 0..=max_id)
            .prop_flat_map(|(num_terms, max_degree, max_id)| {
                Self::arbitrary_with(FunctionParameters {
                    num_terms,
                    max_degree,
                    max_id,
                })
            })
            .boxed()
    }
}
