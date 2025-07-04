use super::*;
use crate::{v1, Message, Parse};
use anyhow::Result;

impl DecisionVariable {
    pub fn to_bytes(&self) -> Vec<u8> {
        let v1_decision_variable = v1::DecisionVariable::from(self.clone());
        v1_decision_variable.encode_to_vec()
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let inner = v1::DecisionVariable::decode(bytes)?;
        Ok(Parse::parse(inner, &())?)
    }
}

impl EvaluatedDecisionVariable {
    pub fn to_bytes(&self) -> Vec<u8> {
        let v1_decision_variable = v1::DecisionVariable::from(self.clone());
        v1_decision_variable.encode_to_vec()
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let inner = v1::DecisionVariable::decode(bytes)?;
        let parsed_dv: DecisionVariable = Parse::parse(inner, &())?;
        // Convert DecisionVariable to EvaluatedDecisionVariable
        // We need the value from substituted_value field
        let value = parsed_dv.substituted_value()
            .ok_or_else(|| anyhow::anyhow!("Missing value for EvaluatedDecisionVariable"))?;
        Ok(EvaluatedDecisionVariable::new(parsed_dv, value, crate::ATol::default())?)
    }
}

impl SampledDecisionVariable {
    pub fn to_bytes(&self) -> Vec<u8> {
        let v1_sampled_decision_variable = v1::SampledDecisionVariable::from(self.clone());
        v1_sampled_decision_variable.encode_to_vec()
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let inner = v1::SampledDecisionVariable::decode(bytes)?;
        Ok(Parse::parse(inner, &())?)
    }
}