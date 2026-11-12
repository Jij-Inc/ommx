use crate::{ATol, VariableID, VariableIDSet};
use std::collections::BTreeMap;

pub fn diagnose(
    variables: &VariableIDSet,
    mut value_of: impl FnMut(VariableID) -> f64,
    atol: ATol,
) -> BTreeMap<VariableID, f64> {
    let retained = variables
        .iter()
        .map(|&id| (id, value_of(id)))
        .max_by(|(_, a), (_, b)| a.abs().total_cmp(&b.abs()))
        .map(|(id, _)| id);
    variables
        .iter()
        .filter_map(|&id| {
            let value = value_of(id);
            (Some(id) != retained && !atol.approx_is_zero(value)).then_some((id, value))
        })
        .collect()
}
