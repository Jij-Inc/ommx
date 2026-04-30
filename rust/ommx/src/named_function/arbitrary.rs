use std::collections::BTreeMap;

use super::*;
use crate::{random::unique_integers, Function, PolynomialParameters};
use anyhow::{anyhow, Result};
use proptest::prelude::*;

impl Arbitrary for NamedFunction {
    type Parameters = PolynomialParameters;
    type Strategy = BoxedStrategy<Self>;
    fn arbitrary_with(params: Self::Parameters) -> Self::Strategy {
        Function::arbitrary_with(params)
            .prop_map(|function| NamedFunction {
                id: NamedFunctionID(0), // Should be replaced with a unique ID, but cannot be generated here
                function,
            })
            .boxed()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NamedFunctionIDParameters {
    size: usize,
    max_id: NamedFunctionID,
}

impl NamedFunctionIDParameters {
    pub fn new(size: usize, max_id: NamedFunctionID) -> Result<Self> {
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

impl Default for NamedFunctionIDParameters {
    fn default() -> Self {
        Self {
            size: 5,
            max_id: NamedFunctionID(10),
        }
    }
}

pub fn arbitrary_named_functions(
    id_parameters: NamedFunctionIDParameters,
    parameters: PolynomialParameters,
) -> impl Strategy<Value = BTreeMap<NamedFunctionID, NamedFunction>> {
    let unique_ids_strategy = unique_integers(0, id_parameters.max_id.0, id_parameters.size);
    let named_functions_strategy = proptest::collection::vec(
        NamedFunction::arbitrary_with(parameters),
        id_parameters.size,
    );
    (unique_ids_strategy, named_functions_strategy)
        .prop_map(|(ids, named_functions)| {
            ids.into_iter()
                .map(NamedFunctionID::from)
                .zip(named_functions)
                .map(|(id, mut named_function)| {
                    named_function.id = id;
                    (id, named_function)
                })
                .collect()
        })
        .boxed()
}
