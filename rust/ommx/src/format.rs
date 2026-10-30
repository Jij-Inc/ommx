use crate::{
    function::operation::{
        instructions, AssociativeOperator, Atom, BinaryOperator, Instruction, MonomialRef,
        PolynomialRef, UnaryOperator,
    },
    Expression, Function, Monomial, VariableID,
};
use std::{collections::BTreeMap, fmt};

/// Options for formatting a [`Function`] with an instance-provided modeling context.
///
/// `max_terms` bounds the number of complete stored terms written. `max_chars`
/// bounds the returned text by Unicode scalar values, so truncation never
/// slices through the middle of a UTF-8 code point.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Default, crate::logical_memory::LogicalMemoryProfile,
)]
pub struct FunctionFormatOptions {
    pub max_terms: Option<usize>,
    pub max_chars: Option<usize>,
}

/// Result of context-aware function formatting.
///
/// `total_terms` is counted from the terms stored in the [`Function`] before
/// output truncation. `written_terms` counts complete terms written to `text`;
/// if the first term is clipped by `max_chars`, it is not counted as written.
#[derive(Debug, Clone, PartialEq, Eq, crate::logical_memory::LogicalMemoryProfile)]
pub struct FormattedFunction {
    pub text: String,
    pub total_terms: usize,
    pub written_terms: usize,
    pub omitted_terms: usize,
    pub truncated_by_chars: bool,
}

fn write_f64_with_precision(f: &mut fmt::Formatter, coefficient: f64) -> fmt::Result {
    if let Some(precision) = f.precision() {
        write!(f, "{coefficient:.precision$}")?;
    } else {
        write!(f, "{coefficient}")?;
    }
    Ok(())
}

fn write_term_ids(
    f: &mut fmt::Formatter,
    ids: impl Iterator<Item = VariableID>,
    coefficient: f64,
) -> fmt::Result {
    let mut ids = ids.peekable();
    if ids.peek().is_none() {
        write_f64_with_precision(f, coefficient)?;
        return Ok(());
    }
    if coefficient == -1.0 {
        write!(f, "-")?;
    } else if coefficient != 1.0 {
        write_f64_with_precision(f, coefficient)?;
    }
    if coefficient.abs() != 1.0 {
        write!(f, "*")?;
    }
    if let Some(id) = ids.next() {
        write!(f, "x{id}")?;
    }
    for id in ids {
        write!(f, "*x{id}")?;
    }
    Ok(())
}

fn write_term_to_string(
    monomial: MonomialRef<'_>,
    coefficient: f64,
    symbols: &BTreeMap<VariableID, String>,
) -> crate::Result<String> {
    if monomial.len() == 0 {
        return Ok(coefficient.to_string());
    }

    let mut out = String::new();
    if coefficient == -1.0 {
        out.push('-');
    } else if coefficient != 1.0 {
        out.push_str(&coefficient.to_string());
    }
    if coefficient.abs() != 1.0 {
        out.push('*');
    }

    let mut ids = monomial.ids().peekable();
    if let Some(id) = ids.next() {
        let symbol = symbols
            .get(&id)
            .ok_or_else(|| crate::error!("Missing symbol for variable ID {id:?}"))?;
        out.push_str(symbol);
    }
    for id in ids {
        let symbol = symbols
            .get(&id)
            .ok_or_else(|| crate::error!("Missing symbol for variable ID {id:?}"))?;
        out.push('*');
        out.push_str(symbol);
    }
    Ok(out)
}

fn char_prefix_byte_len(text: &str, max_chars: usize) -> usize {
    if max_chars == 0 {
        return 0;
    }
    text.char_indices()
        .nth(max_chars)
        .map_or(text.len(), |(index, _)| index)
}

fn format_zero(opts: FunctionFormatOptions) -> FormattedFunction {
    let truncated_by_chars = opts.max_chars == Some(0);
    let text = if truncated_by_chars {
        String::new()
    } else {
        "0".to_string()
    };
    FormattedFunction {
        text,
        total_terms: 0,
        written_terms: 0,
        omitted_terms: 0,
        truncated_by_chars,
    }
}

fn total_polynomial_terms(function: &Function) -> usize {
    if let Some(polynomial) = PolynomialRef::from_function(function) {
        return polynomial.num_terms();
    }
    match function {
        Function::Expression(expression) => instructions(expression)
            .iter()
            .filter_map(|instruction| match instruction {
                Instruction::Push(atom) => Some(PolynomialRef::from_atom(atom).num_terms()),
                Instruction::Unary(_) | Instruction::Associative(_) | Instruction::Binary(_) => {
                    None
                }
            })
            .sum(),
        Function::Zero
        | Function::Constant(_)
        | Function::Linear(_)
        | Function::Quadratic(_)
        | Function::Polynomial(_) => unreachable!("polynomial variants returned above"),
    }
}

fn render_expression(
    expression: &Expression,
    mut render_atom: impl FnMut(&Atom) -> crate::Result<String>,
) -> crate::Result<String> {
    let mut stack = Vec::new();
    for instruction in instructions(expression) {
        match instruction {
            Instruction::Push(atom) => stack.push(render_atom(atom)?),
            Instruction::Unary(operator) => {
                let operand = stack
                    .pop()
                    .expect("validated expression has a unary operand");
                stack.push(match operator {
                    UnaryOperator::Neg => format!("-({operand})"),
                    UnaryOperator::Abs => format!("abs({operand})"),
                    UnaryOperator::Signum => format!("signum({operand})"),
                    UnaryOperator::Powi(exponent) => {
                        format!("powi({operand}, {exponent})")
                    }
                });
            }
            Instruction::Associative(operator) => {
                let rhs = stack
                    .pop()
                    .expect("validated expression has a right operand");
                let lhs = stack
                    .pop()
                    .expect("validated expression has a left operand");
                stack.push(match operator {
                    AssociativeOperator::Add => format!("({lhs}) + ({rhs})"),
                    AssociativeOperator::Mul => format!("({lhs}) * ({rhs})"),
                    AssociativeOperator::Min => format!("min({lhs}, {rhs})"),
                    AssociativeOperator::Max => format!("max({lhs}, {rhs})"),
                });
            }
            Instruction::Binary(operator) => {
                let rhs = stack
                    .pop()
                    .expect("validated expression has a right operand");
                let lhs = stack
                    .pop()
                    .expect("validated expression has a left operand");
                stack.push(match operator {
                    BinaryOperator::Div => format!("({lhs}) / ({rhs})"),
                });
            }
        }
    }
    Ok(stack
        .pop()
        .expect("validated expression leaves one rendered value"))
}

fn render_function_with_symbols(
    function: &Function,
    symbols: &BTreeMap<VariableID, String>,
) -> crate::Result<String> {
    if let Some(polynomial) = PolynomialRef::from_function(function) {
        return Ok(format_polynomial_ref_with_symbols(
            polynomial,
            symbols,
            FunctionFormatOptions::default(),
        )?
        .text);
    }
    let Function::Expression(expression) = function else {
        unreachable!("polynomial variants returned above")
    };
    render_expression(expression, |atom| {
        Ok(format_polynomial_ref_with_symbols(
            PolynomialRef::from_atom(atom),
            symbols,
            FunctionFormatOptions::default(),
        )?
        .text)
    })
}

fn format_polynomial_ref_with_symbols(
    polynomial: PolynomialRef<'_>,
    symbols: &BTreeMap<VariableID, String>,
    opts: FunctionFormatOptions,
) -> crate::Result<FormattedFunction> {
    let mut terms = Vec::with_capacity(polynomial.num_terms());
    polynomial.for_each_term(|monomial, coefficient| {
        terms.push((monomial, coefficient.into_inner()));
    });
    if terms.is_empty() {
        return Ok(format_zero(opts));
    }
    terms.sort_unstable_by(|(a, _), (b, _)| {
        if a.len() != b.len() {
            b.len().cmp(&a.len())
        } else {
            a.ids().cmp(b.ids())
        }
    });

    let total_terms = terms.len();
    let mut text = String::new();
    let mut written_chars = 0;
    let mut written_terms = 0;
    let mut truncated_by_chars = false;
    for (index, (ids, coefficient)) in terms.into_iter().enumerate() {
        if opts
            .max_terms
            .is_some_and(|max_terms| written_terms >= max_terms)
        {
            break;
        }

        let term = if coefficient < 0.0 && index > 0 {
            format!(" - {}", write_term_to_string(ids, -coefficient, symbols)?)
        } else if index > 0 {
            format!(" + {}", write_term_to_string(ids, coefficient, symbols)?)
        } else {
            write_term_to_string(ids, coefficient, symbols)?
        };

        if let Some(max_chars) = opts.max_chars {
            let term_chars = term.chars().count();
            if term_chars <= max_chars.saturating_sub(written_chars) {
                text.push_str(&term);
                written_chars += term_chars;
                written_terms += 1;
            } else {
                truncated_by_chars = true;
                if text.is_empty() && max_chars > 0 {
                    let prefix_len = char_prefix_byte_len(&term, max_chars);
                    text.push_str(&term[..prefix_len]);
                }
                break;
            }
        } else {
            text.push_str(&term);
            written_terms += 1;
        }
    }

    Ok(FormattedFunction {
        text,
        total_terms,
        written_terms,
        omitted_terms: total_terms.saturating_sub(written_terms),
        truncated_by_chars,
    })
}

pub(crate) fn format_function_with_symbols(
    function: &Function,
    symbols: &BTreeMap<VariableID, String>,
    opts: FunctionFormatOptions,
) -> crate::Result<FormattedFunction> {
    let Some(polynomial) = PolynomialRef::from_function(function) else {
        let total_terms = total_polynomial_terms(function);
        // Resolve every referenced symbol before applying display limits. This
        // preserves the validation contract of the polynomial formatter: an
        // unknown variable ID must not be hidden by truncation.
        let rendered = render_function_with_symbols(function, symbols)?;
        let max_terms_truncated = opts
            .max_terms
            .is_some_and(|max_terms| max_terms < total_terms);
        let mut text = if max_terms_truncated {
            "…".to_string()
        } else {
            rendered
        };
        let mut truncated_by_chars = false;
        if let Some(max_chars) = opts.max_chars {
            if text.chars().count() > max_chars {
                text.truncate(char_prefix_byte_len(&text, max_chars));
                truncated_by_chars = true;
            }
        }
        let written_terms = if max_terms_truncated || truncated_by_chars {
            0
        } else {
            total_terms
        };
        return Ok(FormattedFunction {
            text,
            total_terms,
            written_terms,
            omitted_terms: total_terms.saturating_sub(written_terms),
            truncated_by_chars,
        });
    };

    format_polynomial_ref_with_symbols(polynomial, symbols, opts)
}

fn format_polynomial_borrowed<M: Monomial>(
    f: &mut fmt::Formatter,
    polynomial: &crate::PolynomialBase<M>,
) -> fmt::Result {
    let mut terms: Vec<_> = polynomial.iter().collect();
    if terms.is_empty() {
        return write!(f, "0");
    }
    terms.sort_unstable_by(|(a, _), (b, _)| {
        b.degree()
            .cmp(&a.degree())
            .then_with(|| a.ids().cmp(b.ids()))
    });

    let mut terms = terms.into_iter();
    let (monomial, coefficient) = terms.next().unwrap();
    write_term_ids(f, monomial.ids(), coefficient.into_inner())?;

    for (monomial, coefficient) in terms {
        let coefficient = coefficient.into_inner();
        if coefficient < 0.0 {
            write!(f, " - ")?;
            write_term_ids(f, monomial.ids(), -coefficient)?;
        } else {
            write!(f, " + ")?;
            write_term_ids(f, monomial.ids(), coefficient)?;
        }
    }
    Ok(())
}

impl<M: Monomial> fmt::Display for crate::PolynomialBase<M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        format_polynomial_borrowed(f, self)
    }
}

impl<M: Monomial> fmt::Debug for crate::PolynomialBase<M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self}")
    }
}

impl fmt::Display for crate::Function {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            crate::Function::Zero => write!(f, "0"),
            crate::Function::Constant(c) => write!(f, "{}", c.into_inner()),
            crate::Function::Linear(linear) => write!(f, "{linear}"),
            crate::Function::Quadratic(quadratic) => write!(f, "{quadratic}"),
            crate::Function::Polynomial(polynomial) => write!(f, "{polynomial}"),
            crate::Function::Expression(expression) => {
                let rendered = render_expression(expression, |atom| {
                    Ok(match atom {
                        Atom::Zero => "0".to_string(),
                        Atom::Constant(value) => value.into_inner().to_string(),
                        Atom::Linear(value) => value.to_string(),
                        Atom::Quadratic(value) => value.to_string(),
                        Atom::Polynomial(value) => value.to_string(),
                    })
                })
                .map_err(|_| fmt::Error)?;
                write!(f, "{rendered}")
            }
        }
    }
}

impl fmt::Debug for crate::Function {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            crate::Function::Zero => write!(f, "Zero"),
            crate::Function::Constant(c) => write!(f, "Constant({})", c.into_inner()),
            crate::Function::Linear(linear) => write!(f, "Linear({linear})"),
            crate::Function::Quadratic(quadratic) => write!(f, "Quadratic({quadratic})"),
            crate::Function::Polynomial(polynomial) => write!(f, "Polynomial({polynomial})"),
            crate::Function::Expression(expression) => {
                write!(f, "Expression({:?})", instructions(expression))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{coeff, linear, monomial, quadratic, Linear, Polynomial};

    #[test]
    fn test_polynomial_base_display_empty() {
        let poly: Linear = Linear::default();
        assert_eq!(format!("{poly}"), "0");
    }

    #[test]
    fn test_polynomial_base_display_single_term() {
        let poly = (coeff!(3.0) * linear!(1)).unwrap();
        assert_eq!(format!("{poly}"), "3*x1");
    }

    #[test]
    fn test_polynomial_base_display_constant() {
        let poly = Linear::from(coeff!(5.0));
        assert_eq!(format!("{poly}"), "5");
    }

    #[test]
    fn test_polynomial_base_display_multiple_terms() {
        let poly =
            ((coeff!(2.0) * linear!(1)).unwrap() - (coeff!(3.0) * linear!(2)).unwrap()).unwrap();
        let poly = (poly + coeff!(1.0)).unwrap();

        let result = format!("{poly}");
        // Terms should be sorted by degree (highest first), then lexicographically
        assert_eq!(result, "2*x1 - 3*x2 + 1");
    }

    #[test]
    fn test_polynomial_base_display_quadratic() {
        let poly = ((coeff!(4.0) * quadratic!(1, 2)).unwrap()
            - (coeff!(2.0) * quadratic!(1)).unwrap())
        .unwrap();
        let poly = (poly + coeff!(3.0)).unwrap();

        let result = format!("{poly}");
        // Quadratic term should come first (highest degree), then linear, then constant
        assert_eq!(result, "4*x1*x2 - 2*x1 + 3");
    }

    #[test]
    fn test_polynomial_base_display_coefficient_one() {
        let poly: Linear = linear!(1).into();
        assert_eq!(format!("{poly}"), "x1");
    }

    #[test]
    fn test_polynomial_base_display_coefficient_negative_one() {
        let poly = (coeff!(-1.0) * linear!(1)).unwrap();
        assert_eq!(format!("{poly}"), "-x1");
    }

    #[test]
    fn composite_function_display_snapshot() {
        let x = Function::from(linear!(1));
        let y = Function::from(linear!(2));

        insta::assert_snapshot!(-x.clone().abs(), @"-(abs(x1))");
        insta::assert_snapshot!(x.clone().abs(), @"abs(x1)");
        insta::assert_snapshot!(x.clone().signum(), @"signum(x1)");
        insta::assert_snapshot!(x.clone().powi(-2), @"powi(x1, -2)");
        insta::assert_snapshot!(
            (x.clone().abs() + y.clone()).unwrap(),
            @"(abs(x1)) + (x2)"
        );
        insta::assert_snapshot!(
            (x.clone().abs() * y.clone()).unwrap(),
            @"(abs(x1)) * (x2)"
        );
        insta::assert_snapshot!(x.clone().min(y.clone()), @"min(x1, x2)");
        insta::assert_snapshot!(x.clone().max(y.clone()), @"max(x1, x2)");
        insta::assert_snapshot!((x / y).unwrap(), @"(x1) / (x2)");
    }

    #[test]
    fn composite_function_grouping_snapshot() {
        let x = Function::from(linear!(1)).abs();
        let y = Function::from(linear!(2)).signum();
        let z = Function::from(linear!(3)).powi(2);

        let left_grouped = ((x.clone() + y.clone()).unwrap() + z.clone()).unwrap();
        let right_grouped = (x + (y + z).unwrap()).unwrap();

        insta::assert_snapshot!(
            left_grouped,
            @"((abs(x1)) + (signum(x2))) + (powi(x3, 2))"
        );
        insta::assert_snapshot!(
            right_grouped,
            @"(abs(x1)) + ((signum(x2)) + (powi(x3, 2)))"
        );
        insta::assert_debug_snapshot!(
            left_grouped,
            @"Expression([Push(Linear(x1)), Unary(Abs), Push(Linear(x2)), Unary(Signum), Associative(Add), Push(Linear(x3)), Unary(Powi(2)), Associative(Add)])"
        );
    }

    #[test]
    fn composite_context_and_truncation_snapshot() {
        let x = Function::from(linear!(1));
        let y = Function::from(linear!(2));
        let z = Function::from(linear!(3));
        let function = (x.abs().min(y.signum()) / z.powi(-1)).unwrap();
        let symbols = BTreeMap::from([
            (VariableID::from(1), "alpha".to_string()),
            (VariableID::from(2), "beta".to_string()),
            (VariableID::from(3), "gamma".to_string()),
        ]);

        insta::assert_debug_snapshot!(
            format_function_with_symbols(&function, &symbols, FunctionFormatOptions::default())
                .unwrap(),
            @r###"
        FormattedFunction {
            text: "(min(abs(alpha), signum(beta))) / (powi(gamma, -1))",
            total_terms: 3,
            written_terms: 3,
            omitted_terms: 0,
            truncated_by_chars: false,
        }
        "###
        );

        insta::assert_debug_snapshot!(
            format_function_with_symbols(
                &function,
                &symbols,
                FunctionFormatOptions {
                    max_terms: Some(2),
                    max_chars: None,
                },
            )
            .unwrap(),
            @r###"
        FormattedFunction {
            text: "…",
            total_terms: 3,
            written_terms: 0,
            omitted_terms: 3,
            truncated_by_chars: false,
        }
        "###
        );

        insta::assert_debug_snapshot!(
            format_function_with_symbols(
                &function,
                &symbols,
                FunctionFormatOptions {
                    max_terms: None,
                    max_chars: Some(12),
                },
            )
            .unwrap(),
            @r###"
        FormattedFunction {
            text: "(min(abs(alp",
            total_terms: 3,
            written_terms: 0,
            omitted_terms: 3,
            truncated_by_chars: true,
        }
        "###
        );
    }

    #[test]
    fn composite_context_formats_borrowed_dynamic_monomials() {
        let polynomial = Polynomial::single_term(monomial!(4, 3, 2, 1), coeff!(2.0));
        let function = Function::Polynomial(polynomial).abs();
        let symbols = BTreeMap::from([
            (VariableID::from(1), "alpha".to_string()),
            (VariableID::from(2), "beta".to_string()),
            (VariableID::from(3), "gamma".to_string()),
            (VariableID::from(4), "delta".to_string()),
        ]);

        assert_eq!(
            format_function_with_symbols(&function, &symbols, FunctionFormatOptions::default())
                .unwrap()
                .text,
            "abs(2*alpha*beta*gamma*delta)"
        );
    }

    #[test]
    fn composite_format_validates_symbols_before_truncation() {
        let function = Function::from(linear!(1)).abs();
        let error = format_function_with_symbols(
            &function,
            &BTreeMap::new(),
            FunctionFormatOptions {
                max_terms: Some(0),
                max_chars: Some(0),
            },
        )
        .unwrap_err();
        assert!(error.to_string().contains("Missing symbol for variable ID"));
    }
}
