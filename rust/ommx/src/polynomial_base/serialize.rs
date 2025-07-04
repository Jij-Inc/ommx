use super::*;
use crate::{v1, Message, Parse};
use anyhow::Result;

impl Linear {
    pub fn to_bytes(&self) -> Vec<u8> {
        let v1_linear = v1::Linear::from(self.clone());
        v1_linear.encode_to_vec()
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let inner = v1::Linear::decode(bytes)?;
        Ok(Parse::parse(inner, &())?)
    }
}

impl Quadratic {
    pub fn to_bytes(&self) -> Vec<u8> {
        let v1_quadratic = v1::Quadratic::from(self.clone());
        v1_quadratic.encode_to_vec()
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let inner = v1::Quadratic::decode(bytes)?;
        Ok(Parse::parse(inner, &())?)
    }
}

impl Polynomial {
    pub fn to_bytes(&self) -> Vec<u8> {
        let v1_polynomial = v1::Polynomial::from(self.clone());
        v1_polynomial.encode_to_vec()
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let inner = v1::Polynomial::decode(bytes)?;
        Ok(Parse::parse(inner, &())?)
    }
}
