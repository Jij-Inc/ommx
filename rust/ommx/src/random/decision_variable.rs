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

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DecisionVariableParameters {
    pub id: u64,
    pub kind: Kind,
}

impl Arbitrary for DecisionVariable {
    type Parameters = DecisionVariableParameters;
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(DecisionVariableParameters { id, kind }: Self::Parameters) -> Self::Strategy {
        let subscripts = prop_oneof![
            Just(Vec::<i64>::new()),
            proptest::collection::vec(-10_i64..=10, 1..=3),
        ];
        let parameters = prop_oneof![
            Just(HashMap::<String, String>::new()),
            proptest::collection::hash_map(".{0,3}", ".{0,3}", 1..=3),
        ];
        (
            Just(id),
            Option::<Bound>::arbitrary(),
            proptest::option::of(".{0,3}"),
            Just(kind),
            subscripts,
            parameters,
            proptest::option::of(".{0,3}"),
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
                    substituted_value: None,
                },
            )
            .boxed()
    }

    fn arbitrary() -> Self::Strategy {
        (Just(0), Kind::arbitrary())
            .prop_flat_map(|(id, kind)| {
                Self::arbitrary_with(DecisionVariableParameters { id, kind })
            })
            .boxed()
    }
}

pub fn arbitrary_decision_variables(
    ids: BTreeSet<u64>,
    kind_strategy: impl Strategy<Value = Kind> + 'static,
) -> BoxedStrategy<Vec<DecisionVariable>> {
    (
        proptest::collection::vec(
            (Just(0), kind_strategy).prop_flat_map(|(id, kind)| {
                DecisionVariable::arbitrary_with(DecisionVariableParameters { id, kind })
            }),
            ids.len(),
        ),
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
