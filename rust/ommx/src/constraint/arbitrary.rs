use super::*;
use crate::{Function, PolynomialParameters};
use proptest::prelude::*;

impl Arbitrary for Equality {
    type Parameters = ();
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(_params: Self::Parameters) -> Self::Strategy {
        prop_oneof![
            Just(Equality::EqualToZero),
            Just(Equality::LessThanOrEqualToZero),
        ]
        .boxed()
    }
}

impl Arbitrary for Constraint {
    type Parameters = PolynomialParameters;
    type Strategy = BoxedStrategy<Self>;
    fn arbitrary_with(params: Self::Parameters) -> Self::Strategy {
        (Function::arbitrary_with(params), Equality::arbitrary())
            .prop_map(|(function, equality)| Constraint {
                id: ConstraintID(0), // Should be replaced with a unique ID, but cannot be generated here
                function,
                equality,
                name: None,
                subscripts: Vec::new(),
                parameters: Default::default(),
                description: None,
            })
            .boxed()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ConstraintIDParameters {
    size: usize,
    max_id: ConstraintID,
}

impl ConstraintIDParameters {
    pub fn new(size: usize, max_id: ConstraintID) -> Result<Self> {
        if size > max_id.0 as usize + 1 {
            return Err(anyhow!(
                "size {} is greater than `max_id {} + 1`",
                size,
                max_id.0
            ));
        }
        Ok(Self { size, max_id })
    }
}

impl Default for ConstraintIDParameters {
    fn default() -> Self {
        Self {
            size: 5,
            max_id: ConstraintID(10),
        }
    }
}

pub fn arbitrary_constraints(
    id_parameters: ConstraintIDParameters,
    parameters: PolynomialParameters,
) -> impl Strategy<Value = FnvHashMap<ConstraintID, Constraint>> {
    let unique_ids_strategy = unique_integers(0, id_parameters.max_id.0, id_parameters.size);
    let constraints_strategy =
        proptest::collection::vec(Constraint::arbitrary_with(parameters), id_parameters.size);
    (unique_ids_strategy, constraints_strategy)
        .prop_map(|(ids, constraints)| {
            ids.into_iter()
                .map(ConstraintID::from)
                .zip(constraints)
                .map(|(id, mut constraint)| {
                    constraint.id = id;
                    (id, constraint)
                })
                .collect()
        })
        .boxed()
}
