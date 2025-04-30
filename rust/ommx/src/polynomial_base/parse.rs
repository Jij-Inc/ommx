use super::*;
use crate::{parse::*, v1, CoefficientError, VariableID};
use itertools::izip;

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

impl TryFrom<v1::linear::Term> for Option<Term> {
    type Error = ParseError;
    fn try_from(value: v1::linear::Term) -> Result<Self, Self::Error> {
        value.parse(&())
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
                out.add_term(term.id.into(), term.coefficient);
            }
        }
        match self.constant.try_into() {
            Ok(coefficient) => out.add_term(LinearMonomial::Constant, coefficient),
            Err(CoefficientError::Zero) => {}
            Err(e) => return Err(RawParseError::from(e).context(message, "constant")),
        }
        Ok(out)
    }
}

impl TryFrom<v1::Linear> for Linear {
    type Error = ParseError;
    fn try_from(value: v1::Linear) -> Result<Self, Self::Error> {
        value.parse(&())
    }
}

impl From<&Linear> for v1::Linear {
    fn from(value: &Linear) -> Self {
        let mut out = v1::Linear::default();
        for (id, coefficient) in &value.terms {
            match id {
                LinearMonomial::Constant => {
                    out.constant = coefficient.into_inner();
                }
                LinearMonomial::Variable(id) => {
                    out.terms.push(v1::linear::Term {
                        id: id.into_inner(),
                        coefficient: coefficient.into_inner(),
                    });
                }
            }
        }
        out
    }
}

#[derive(Debug, thiserror::Error)]
pub enum QuadraticParseError {
    #[error("Row length ({row}) does not match value length ({value})")]
    RowLengthMismatch { row: usize, value: usize },
    #[error("Column length ({column}) does not match value length ({value})")]
    ColumnLengthMismatch { column: usize, value: usize },
}

impl Parse for v1::Quadratic {
    type Output = Quadratic;
    type Context = ();

    fn parse(self, _: &Self::Context) -> Result<Self::Output, ParseError> {
        let message = "ommx.v1.Quadratic";
        let mut out = Quadratic::default();
        let num_terms = self.values.len();
        if self.columns.len() != num_terms {
            return Err(
                RawParseError::from(QuadraticParseError::ColumnLengthMismatch {
                    column: self.columns.len(),
                    value: num_terms,
                })
                .context(message, "columns"),
            );
        }
        if self.rows.len() != num_terms {
            return Err(RawParseError::from(QuadraticParseError::RowLengthMismatch {
                row: self.rows.len(),
                value: num_terms,
            })
            .context(message, "rows"));
        }
        for (column, row, value) in izip!(self.columns, self.rows, self.values) {
            let column = VariableID::from(column);
            let row = VariableID::from(row);
            match value.try_into() {
                Ok(coefficient) => out.add_term((column, row).into(), coefficient),
                Err(CoefficientError::Zero) => {}
                Err(e) => return Err(RawParseError::from(e).context(message, "values")),
            }
        }

        if let Some(linear) = self.linear {
            let linear = linear.parse_as(&(), message, "linear")?;
            out += &linear;
        }
        Ok(out)
    }
}

impl From<&Quadratic> for v1::Quadratic {
    fn from(value: &Quadratic) -> Self {
        let mut out = v1::Quadratic::default();
        for (id, coefficient) in &value.terms {
            match id {
                QuadraticMonomial::Constant => {
                    out.linear.get_or_insert_default().constant = coefficient.into_inner();
                }
                QuadraticMonomial::Linear(id) => {
                    out.linear
                        .get_or_insert_default()
                        .terms
                        .push(v1::linear::Term {
                            id: id.into_inner(),
                            coefficient: coefficient.into_inner(),
                        });
                }
                QuadraticMonomial::Pair(pair) => {
                    out.rows.push(pair.lower().into_inner());
                    out.columns.push(pair.upper().into_inner());
                    out.values.push(coefficient.into_inner());
                }
            }
        }
        out
    }
}

impl Parse for v1::Monomial {
    type Output = Option<(MonomialDyn, Coefficient)>;
    type Context = ();

    fn parse(self, _: &Self::Context) -> Result<Self::Output, ParseError> {
        let message = "ommx.v1.Monomial";
        let ids = MonomialDyn::new(self.ids);
        match self.coefficient.try_into() {
            Ok(coefficient) => Ok(Some((ids, coefficient))),
            Err(CoefficientError::Zero) => Ok(None),
            Err(e) => Err(RawParseError::from(e).context(message, "coefficient")),
        }
    }
}

impl Parse for v1::Polynomial {
    type Output = Polynomial;
    type Context = ();

    fn parse(self, _: &Self::Context) -> Result<Self::Output, ParseError> {
        let mut out = Polynomial::default();
        for term in self.terms {
            if let Some((monomial, coefficient)) =
                term.parse_as(&(), "ommx.v1.Polynomial", "terms")?
            {
                out.add_term(monomial, coefficient);
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v1::linear::Term;
    use maplit::*;
    use proptest::prelude::*;

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
                    LinearMonomial::Constant => 4.0.try_into().unwrap()
                },
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
                    LinearMonomial::Constant => 4.0.try_into().unwrap(),
                },
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
        Coefficient must be finite
        "###);
    }

    proptest! {
        /// Linear -> v1::Linear -> Linear roundtrip test
        #[test]
        fn test_linear_roundtrip(linear in Linear::arbitrary()) {
            let v1_linear: v1::Linear = (&linear).into();
            let parsed = v1_linear.parse(&()).unwrap();
            prop_assert_eq!(linear, parsed);
        }
    }

    #[test]
    fn test_parse_quadratic() {
        // Valid case
        let quadratic = v1::Quadratic {
            rows: vec![1, 2, 3],
            columns: vec![4, 5, 6],
            values: vec![7.0, 8.0, 9.0],
            linear: None,
        };
        assert_eq!(
            quadratic.parse(&()).unwrap(),
            Quadratic {
                terms: hashmap! {
                    (1.into(), 4.into()).into() => 7.0.try_into().unwrap(),
                    (2.into(), 5.into()).into() => 8.0.try_into().unwrap(),
                    (3.into(), 6.into()).into() => 9.0.try_into().unwrap(),
                },
            }
        );

        // Invalid case: row length mismatch
        let quadratic = v1::Quadratic {
            rows: vec![1, 2],
            columns: vec![4, 5, 6],
            values: vec![7.0, 8.0, 9.0],
            linear: None,
        };
        insta::assert_snapshot!(quadratic.parse(&()).unwrap_err(), @r###"
        Traceback for OMMX Message parse error:
        └─ommx.v1.Quadratic[rows]
        Row length (2) does not match value length (3)
        "###);

        // Invalid case: column length mismatch
        let quadratic = v1::Quadratic {
            rows: vec![1, 2, 3],
            columns: vec![4, 5],
            values: vec![7.0, 8.0, 9.0],
            linear: None,
        };
        insta::assert_snapshot!(quadratic.parse(&()).unwrap_err(), @r###"
        Traceback for OMMX Message parse error:
        └─ommx.v1.Quadratic[columns]
        Column length (2) does not match value length (3)
        "###);
    }

    proptest! {
        /// Quadratic -> v1::Quadratic -> Quadratic roundtrip test
        #[test]
        fn test_quadratic_roundtrip(quadratic in Quadratic::arbitrary()) {
            let v1_quadratic: v1::Quadratic = (&quadratic).into();
            let parsed = v1_quadratic.parse(&()).unwrap();
            prop_assert_eq!(quadratic, parsed);
        }
    }

    #[test]
    fn test_parse_polynomial() {
        // Valid case
        let polynomial = v1::Polynomial {
            terms: vec![
                v1::Monomial {
                    ids: vec![1, 2],
                    coefficient: 3.0,
                },
                v1::Monomial {
                    ids: vec![3, 4],
                    coefficient: 5.0,
                },
            ],
        };
        assert_eq!(
            polynomial.parse(&()).unwrap(),
            Polynomial {
                terms: hashmap! {
                    MonomialDyn::new(vec![1, 2]) => 3.0.try_into().unwrap(),
                    MonomialDyn::new(vec![3, 4]) => 5.0.try_into().unwrap(),
                },
            }
        );

        // Invalid case: coefficient has infinity
        let polynomial = v1::Polynomial {
            terms: vec![
                v1::Monomial {
                    ids: vec![1, 2],
                    coefficient: f64::INFINITY,
                },
                v1::Monomial {
                    ids: vec![3, 4],
                    coefficient: 5.0,
                },
            ],
        };
        insta::assert_snapshot!(polynomial.parse(&()).unwrap_err(), @r###"
        Traceback for OMMX Message parse error:
        └─ommx.v1.Polynomial[terms]
          └─ommx.v1.Monomial[coefficient]
        Coefficient must be finite
        "###);
    }
}
