//! Decode bridge representations through the Rust SDK's domain parsers.

use ommx::Parse as _;

fn decode<M: ommx::Message + Default>(bytes: &[u8], root: &'static str) -> ommx::Result<M> {
    Ok(
        M::decode(bytes)
            .map_err(|error| ommx::RawParseError::from(error).context(root, "bytes"))?,
    )
}

pub fn function(bytes: &[u8]) -> ommx::Result<ommx::Function> {
    ommx::Function::from_bytes(bytes)
}

pub fn constraint_v2(
    constraint: &[u8],
    context: &[u8],
) -> ommx::Result<(ommx::Constraint, ommx::ConstraintContext)> {
    Ok((
        decode::<ommx::v2::RegularConstraint>(constraint, "ommx.v2.RegularConstraint")?
            .parse(&())?,
        decode::<ommx::v2::ConstraintContext>(context, "ommx.v2.ConstraintContext")?.parse(&())?,
    ))
}

pub fn constraint_v1(bytes: &[u8]) -> ommx::Result<(ommx::Constraint, ommx::ConstraintContext)> {
    let (_, constraint, context) =
        decode::<ommx::v1::Constraint>(bytes, "ommx.v1.Constraint")?.parse(&())?;
    Ok((constraint, context))
}

pub fn decision_variable_v2(
    id: u64,
    variable: &[u8],
    label: &[u8],
) -> ommx::Result<(
    ommx::VariableID,
    ommx::DecisionVariable,
    ommx::ModelingLabel,
)> {
    let id = ommx::VariableID::from(id);
    Ok((
        id,
        decode::<ommx::v2::DecisionVariable>(variable, "ommx.v2.DecisionVariable")?.parse(&id)?,
        decode::<ommx::v2::ModelingLabel>(label, "ommx.v2.ModelingLabel")?.into(),
    ))
}

pub fn decision_variable_v1(
    bytes: &[u8],
) -> ommx::Result<(
    ommx::VariableID,
    ommx::DecisionVariable,
    ommx::ModelingLabel,
)> {
    let parsed =
        decode::<ommx::v1::DecisionVariable>(bytes, "ommx.v1.DecisionVariable")?.parse(&())?;
    if parsed.fixed_value.is_some() {
        return Err(ommx::Error::msg(
            "a detached DecisionVariable cannot own a fixed value; transfer its Instance instead",
        ));
    }
    Ok((parsed.id, parsed.variable, parsed.label))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ommx::Message as _;

    #[test]
    fn constraint_receiver_accepts_deep_flat_expression() {
        let function = (0..4096).fold(ommx::Function::from(ommx::linear!(1)), |function, level| {
            if level % 2 == 0 {
                function.abs()
            } else {
                function.signum()
            }
        });
        let expected = ommx::Constraint::equal_to_zero(function);
        let constraint = ommx::v2::RegularConstraint::from(expected.clone()).encode_to_vec();
        let context = ommx::v2::ConstraintContext::default().encode_to_vec();
        let (actual, _) = constraint_v2(&constraint, &context).unwrap();
        assert_eq!(actual, expected);
    }
}
