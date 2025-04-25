use super::*;
use proptest::prelude::*;

impl<M: Monomial> Arbitrary for Polynomial<M> {
    type Parameters = M::Parameters;
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(_p: Self::Parameters) -> Self::Strategy {
        todo!()
    }

    fn arbitrary() -> Self::Strategy {
        todo!()
    }
}
