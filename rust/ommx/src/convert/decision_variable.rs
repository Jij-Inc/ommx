use crate::v1::{
    decision_variable::Kind, Bound, DecisionVariable, Function, Linear, Polynomial, Quadratic,
};
use proptest::prelude::*;
use std::{
    collections::{BTreeSet, HashMap},
    ops::*,
};

impl From<&DecisionVariable> for Linear {
    fn from(dv: &DecisionVariable) -> Self {
        Linear::from(dv.id)
    }
}

macro_rules! impl_from_decision_variable {
    ($type:ty) => {
        impl From<&DecisionVariable> for $type {
            fn from(dv: &DecisionVariable) -> Self {
                Linear::from(dv).into()
            }
        }
    };
}
impl_from_decision_variable!(Quadratic);
impl_from_decision_variable!(Polynomial);
impl_from_decision_variable!(Function);

impl Add for &DecisionVariable {
    type Output = Linear;

    fn add(self, rhs: Self) -> Self::Output {
        Linear::from(self) + Linear::from(rhs)
    }
}

impl Mul for &DecisionVariable {
    type Output = Quadratic;

    fn mul(self, rhs: Self) -> Self::Output {
        Linear::from(self) * Linear::from(rhs)
    }
}

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
        let subscripts = prop_oneof![
            Just(Vec::<i64>::new()),
            proptest::collection::vec(-(max_id as i64)..=(max_id as i64), 1..=3),
        ];
        let parameters = prop_oneof![
            Just(HashMap::<String, String>::new()),
            proptest::collection::hash_map(String::arbitrary(), String::arbitrary(), 1..=3),
        ];
        (
            0..=max_id,
            Option::<Bound>::arbitrary(),
            Option::<String>::arbitrary(),
            Kind::arbitrary(),
            subscripts,
            parameters,
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
                    substituted_value: None,
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
