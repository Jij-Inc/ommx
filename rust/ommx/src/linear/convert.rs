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

impl Parse for v1::Linear {
    type Output = Linear;
    type Context = ();

    fn parse(self, _: &Self::Context) -> Result<Self::Output, ParseError> {
        let mut out = Linear::default();
        for v1::linear::Term { id, coefficient } in self.terms {
            let coefficient = match coefficient.try_into() {
                Ok(coefficient) => coefficient,
                Err(CoefficientError::Zero) => continue,
                Err(e) => {
                    return Err(RawParseError::from(e)
                        .context("ommx.v1.linear.Term", "coefficient")
                        .into())
                }
            };
            out.add_term(id.into(), coefficient);
        }
        out.constant = self
            .constant
            .try_into()
            .map_err(|e| RawParseError::from(e).context("ommx.v1.Linear", "constant"))?;
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v1::linear::Term;

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
        insta::assert_debug_snapshot!(linear.parse(&()).unwrap(), @r###"
        Linear {
            terms: {
                VariableID(
                    1,
                ): Coefficient(
                    2.0,
                ),
                VariableID(
                    2,
                ): Coefficient(
                    3.0,
                ),
            },
            constant: Offset(
                4.0,
            ),
        }
        "###);

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
        insta::assert_debug_snapshot!(linear.parse(&()).unwrap(), @r###"
        Linear {
            terms: {
                VariableID(
                    2,
                ): Coefficient(
                    3.0,
                ),
            },
            constant: Offset(
                4.0,
            ),
        }
        "###);
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
        Coefficient must be finite
        "###);
    }
}
