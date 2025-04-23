use super::*;
use crate::{parse::*, v1, CoefficientError};

impl Linear {
    pub fn add_term(&mut self, id: VariableID, coefficient: Coefficient) {
        use std::collections::hash_map::Entry;
        match self.terms.entry(id) {
            Entry::Occupied(mut entry) => {
                *entry.get_mut() += coefficient;
            }
            Entry::Vacant(entry) => {
                entry.insert(coefficient);
            }
        }
    }
}

impl From<Linear> for v1::Linear {
    fn from(linear: Linear) -> Self {
        let mut new = Self::default();
        for (id, coefficient) in linear.terms {
            new.terms.push(v1::linear::Term {
                id: id.into(),
                coefficient: coefficient.into(),
            });
        }
        new.constant = linear.constant.into();
        new
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Term {
    id: VariableID,
    coefficient: Coefficient,
}

impl Parse for v1::linear::Term {
    type Output = Option<Term>;
    type Context = ();
    fn parse(self, _: &Self::Context) -> Result<Self::Output, ParseError> {
        let id = VariableID::from(self.id);
        match self.coefficient.try_into() {
            Ok(coefficient) => Ok(Some(Term { id, coefficient })),
            Err(CoefficientError::Zero) => Ok(None),
            Err(e) => Err(RawParseError::from(e).context("ommx.v1.linear.Term", "coefficient")),
        }
    }
}

impl Parse for v1::Linear {
    type Output = Linear;
    type Context = ();

    fn parse(self, _: &Self::Context) -> Result<Self::Output, ParseError> {
        let message = "ommx.v1.Linear";
        let mut out = Linear::default();
        for term in self.terms {
            let term = term.parse_as(&(), message, "terms")?;
            if let Some(term) = term {
                out.add_term(term.id, term.coefficient);
            }
        }
        out.constant = self
            .constant
            .try_into()
            .map_err(|e| RawParseError::from(e).context(message, "constant"))?;
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v1::linear::Term;
    use maplit::*;

    #[test]
    fn test_parse_linear() {
        // Valid case
        let linear = v1::Linear {
            terms: vec![
                Term {
                    id: 1,
                    coefficient: 2.0,
                },
                Term {
                    id: 2,
                    coefficient: 3.0,
                },
            ],
            constant: 4.0,
        };
        assert_eq!(
            linear.parse(&()).unwrap(),
            Linear {
                terms: hashmap! {
                    1.into() => 2.0.try_into().unwrap(),
                    2.into() => 3.0.try_into().unwrap(),
                },
                constant: 4.0.try_into().unwrap(),
            }
        );

        // Valid case with zero coefficient
        let linear = v1::Linear {
            terms: vec![
                Term {
                    id: 1,
                    coefficient: 0.0,
                },
                Term {
                    id: 2,
                    coefficient: 3.0,
                },
            ],
            constant: 4.0,
        };
        assert_eq!(
            linear.parse(&()).unwrap(),
            Linear {
                terms: hashmap! {
                    2.into() => 3.0.try_into().unwrap(),
                },
                constant: 4.0.try_into().unwrap(),
            }
        )
    }

    #[test]
    fn test_parse_linear_error() {
        // Invalid case: coefficient has infinity
        let linear = v1::Linear {
            terms: vec![
                Term {
                    id: 1,
                    coefficient: f64::INFINITY,
                },
                Term {
                    id: 2,
                    coefficient: 3.0,
                },
            ],
            constant: 4.0,
        };
        insta::assert_snapshot!(linear.parse(&()).unwrap_err(), @r###"
        Traceback for OMMX Message parse error:
        └─ommx.v1.Linear[terms]
          └─ommx.v1.linear.Term[coefficient]
        Coefficient must be finite
        "###);

        // Invalid case: constant has infinity
        let linear = v1::Linear {
            terms: vec![
                Term {
                    id: 1,
                    coefficient: 2.0,
                },
                Term {
                    id: 2,
                    coefficient: 3.0,
                },
            ],
            constant: f64::INFINITY,
        };
        insta::assert_snapshot!(linear.parse(&()).unwrap_err(), @r###"
        Traceback for OMMX Message parse error:
        └─ommx.v1.Linear[constant]
        Offset must be finite
        "###);
    }
}
