mod evaluation;
mod parse;
mod storage;

use super::*;
use crate::{
    coeff, linear, v1, ATol, Constraint, ConstraintContext, ConstraintID, Equality, Evaluate,
    EvaluatedConstraint, Function, ModelingLabel, Parse, ParseError, SampleID, SampledConstraint,
    VariableIDSet,
};

fn inactive_metadata() -> InactiveConstraintMetadata {
    InactiveConstraintMetadata {
        reason: "test".to_string(),
        parameters: Default::default(),
        skip_feasibility_check: false,
    }
}
