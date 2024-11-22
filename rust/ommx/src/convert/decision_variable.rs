use crate::v1::{decision_variable::Kind, Bound, DecisionVariable};
use proptest::prelude::*;
use std::collections::HashMap;

impl Arbitrary for Bound {
    type Parameters = ();
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(_parameter: ()) -> Self::Strategy {
        let lower = prop_oneof![
            Just(f64::NEG_INFINITY),
            Just(0.0),
            Just(-1.0),
            Just(1.0),
            -10.0..10.0,
        ];
        let upper = prop_oneof![
            Just(f64::INFINITY),
            Just(0.0),
            Just(-1.0),
            Just(1.0),
            -10.0..10.0,
        ];
        (lower, upper)
            .prop_filter_map("Invalid bound", |(lower, upper)| {
                if lower <= upper {
                    Some(Bound { lower, upper })
                } else {
                    None
                }
            })
            .boxed()
    }
}

impl Arbitrary for DecisionVariable {
    type Parameters = u64;
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(max_id: Self::Parameters) -> Self::Strategy {
        (
            0..=max_id,
            Option::<Bound>::arbitrary(),
            Option::<String>::arbitrary(),
            prop_oneof![
                Just(Kind::Binary as i32),
                Just(Kind::Integer as i32),
                Just(Kind::Continuous as i32),
                Just(Kind::SemiInteger as i32),
                Just(Kind::SemiContinuous as i32),
            ],
            Vec::<i64>::arbitrary(),
            HashMap::<String, String>::arbitrary(),
            Option::<String>::arbitrary(),
        )
            .prop_map(
                |(id, bound, name, kind, subscripts, parameters, description)| DecisionVariable {
                    id,
                    bound,
                    name,
                    kind,
                    subscripts,
                    parameters,
                    description,
                },
            )
            .boxed()
    }
}
