use crate::random::random_lp;
use proptest::prelude::*;
use rand::SeedableRng;

#[derive(Debug, Clone)]
pub enum InstanceParameter {
    LP {
        num_constraints: usize,
        num_variables: usize,
    },
    // FIXME: Add more instance types
}

impl Default for InstanceParameter {
    fn default() -> Self {
        InstanceParameter::LP {
            num_constraints: 5,
            num_variables: 7,
        }
    }
}

impl Arbitrary for crate::v1::Instance {
    type Parameters = InstanceParameter;
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(parameter: InstanceParameter) -> Self::Strategy {
        // The instance yielded from strategy must depends only on the parameter deterministically.
        // Thus we should not use `thread_rng` here.
        let mut rng = rand_xoshiro::Xoshiro256StarStar::seed_from_u64(0);
        match parameter {
            InstanceParameter::LP {
                num_constraints,
                num_variables,
            } => Just(random_lp(&mut rng, num_variables, num_constraints)).boxed(),
        }
    }
}
