use super::*;
use crate::{v1, Message, Parse};
use anyhow::Result;

impl NamedFunction {
    pub fn to_bytes(&self) -> Vec<u8> {
        let v1_named_function = v1::NamedFunction::from(self.clone());
        v1_named_function.encode_to_vec()
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let inner = v1::NamedFunction::decode(bytes)?;
        Ok(Parse::parse(inner, &())?)
    }
}

impl EvaluatedNamedFunction {
    pub fn to_bytes(&self) -> Vec<u8> {
        let v1_evaluated_named_function = v1::EvaluatedNamedFunction::from(self.clone());
        v1_evaluated_named_function.encode_to_vec()
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let inner = v1::EvaluatedNamedFunction::decode(bytes)?;
        Ok(Parse::parse(inner, &())?)
    }
}

impl SampledNamedFunction {
    pub fn to_bytes(&self) -> Vec<u8> {
        let v1_sampled_named_function = v1::SampledNamedFunction::from(self.clone());
        v1_sampled_named_function.encode_to_vec()
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let inner = v1::SampledNamedFunction::decode(bytes)?;
        Ok(Parse::parse(inner, &())?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Coefficient, Function};

    #[test]
    fn test_named_function_bytes_roundtrip() {
        let nf = NamedFunction {
            id: NamedFunctionID::from(5),
            function: Function::Constant(Coefficient::try_from(3.14).unwrap()),
            name: Some("test_func".to_string()),
            subscripts: vec![1, 2, 3],
            parameters: {
                let mut params = fnv::FnvHashMap::default();
                params.insert("key".to_string(), "value".to_string());
                params
            },
            description: Some("roundtrip test".to_string()),
        };

        let bytes = nf.to_bytes();
        let restored = NamedFunction::from_bytes(&bytes).unwrap();

        assert_eq!(nf, restored);
    }

    #[test]
    fn test_evaluated_named_function_bytes_roundtrip() {
        let enf = EvaluatedNamedFunction {
            id: NamedFunctionID::from(10),
            evaluated_value: 99.5,
            name: Some("eval_func".to_string()),
            subscripts: vec![4, 5],
            parameters: Default::default(),
            description: Some("evaluated roundtrip".to_string()),
            used_decision_variable_ids: [crate::VariableID::from(1), crate::VariableID::from(2)]
                .into_iter()
                .collect(),
        };

        let bytes = enf.to_bytes();
        let restored = EvaluatedNamedFunction::from_bytes(&bytes).unwrap();

        assert_eq!(enf, restored);
    }
}
