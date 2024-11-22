use crate::v1::{decision_variable::Kind, Bound, DecisionVariable};
use proptest::prelude::*;
use std::collections::{BTreeSet, HashMap};

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

impl Arbitrary for Kind {
    type Parameters = ();
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(_parameter: ()) -> Self::Strategy {
        prop_oneof![
            Just(Kind::Binary),
            Just(Kind::Integer),
            Just(Kind::Continuous),
            Just(Kind::SemiInteger),
            Just(Kind::SemiContinuous),
        ]
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
            Kind::arbitrary(),
            Vec::<i64>::arbitrary(),
            HashMap::<String, String>::arbitrary(),
            Option::<String>::arbitrary(),
        )
            .prop_map(
                |(id, bound, name, kind, subscripts, parameters, description)| DecisionVariable {
                    id,
                    bound,
                    name,
                    kind: kind as i32,
                    subscripts,
                    parameters,
                    description,
                },
            )
            .boxed()
    }
}

pub fn arbitrary_decision_variables(ids: BTreeSet<u64>) -> BoxedStrategy<Vec<DecisionVariable>> {
    (
        proptest::collection::vec(DecisionVariable::arbitrary(), ids.len()),
        Just(ids),
    )
        .prop_map(|(mut dvs, used_ids)| {
            for (dv, id) in dvs.iter_mut().zip(used_ids.iter()) {
                dv.id = *id;
            }
            dvs
        })
        .boxed()
}
