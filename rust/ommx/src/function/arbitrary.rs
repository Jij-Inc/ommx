use super::*;
use crate::{
    Coefficient, Linear, LinearParameters, Polynomial, PolynomialParameters, Quadratic,
    QuadraticParameters,
};
use num::Zero;
use proptest::{prelude::*, strategy::Union};

impl Arbitrary for Function {
    type Parameters = PolynomialParameters;
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(p: Self::Parameters) -> Self::Strategy {
        if p.num_terms() == 0 {
            return prop_oneof![
                Just(Function::Zero),
                Just(Function::Linear(Linear::zero())),
                Just(Function::Quadratic(Quadratic::zero())),
                Just(Function::Polynomial(Polynomial::zero())),
            ]
            .boxed();
        }
        if p.max_degree() == 0 {
            debug_assert_eq!(p.num_terms(), 1);
            return Coefficient::arbitrary()
                .prop_map(Function::Constant)
                .boxed();
        }
        let polynomial = Polynomial::arbitrary_with(p);
        let linear = LinearParameters::new(p.num_terms(), p.max_id())
            .ok()
            .map(Linear::arbitrary_with);
        let quad = if p.max_degree() == 1 {
            linear.clone().map(|l| l.prop_map(Quadratic::from).boxed())
        } else {
            QuadraticParameters::new(p.num_terms(), p.max_id())
                .ok()
                .map(|p| Quadratic::arbitrary_with(p).boxed())
        };
        let mut candidates = vec![polynomial.prop_map(Function::Polynomial).boxed()];
        if let Some(l) = linear {
            candidates.push(l.prop_map(Function::Linear).boxed());
        }
        if let Some(q) = quad {
            candidates.push(q.prop_map(Function::Quadratic).boxed());
        }
        Union::new(candidates).boxed()
    }
}
