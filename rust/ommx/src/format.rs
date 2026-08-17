use crate::{Function, Monomial, MonomialDyn, VariableID};
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

fn write_term(f: &mut fmt::Formatter, ids: MonomialDyn, coefficient: f64) -> fmt::Result {
    if ids.is_empty() {
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
    let mut ids = ids.iter().peekable();
    if let Some(id) = ids.next() {
        write!(f, "x{id}")?;
    }
    for id in ids {
        write!(f, "*x{id}")?;
    }
    Ok(())
}

fn write_term_to_string(
    ids: &MonomialDyn,
    coefficient: f64,
    symbols: &BTreeMap<VariableID, String>,
) -> crate::Result<String> {
    if ids.is_empty() {
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

    let mut ids = ids.iter().peekable();
    if let Some(id) = ids.next() {
        let symbol = symbols
            .get(id)
            .ok_or_else(|| crate::error!("Missing symbol for variable ID {id:?}"))?;
        out.push_str(symbol);
    }
    for id in ids {
        let symbol = symbols
            .get(id)
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
    match function {
        Function::Zero => 0,
        Function::Constant(_) => 1,
        Function::Linear(value) => value.num_terms(),
        Function::Quadratic(value) => value.num_terms(),
        Function::Polynomial(value) => value.num_terms(),
        Function::Unary(operation) => total_polynomial_terms(operation.operand()),
        Function::Nary(operation) => operation
            .operands()
            .iter()
            .map(total_polynomial_terms)
            .sum(),
        Function::Binary(operation) => {
            total_polynomial_terms(operation.lhs()) + total_polynomial_terms(operation.rhs())
        }
    }
}

fn render_function_with_symbols(
    function: &Function,
    symbols: &BTreeMap<VariableID, String>,
) -> crate::Result<String> {
    if function.is_polynomial() {
        return Ok(format_function_with_symbols(
            function,
            symbols,
            FunctionFormatOptions::default(),
        )?
        .text);
    }
    Ok(match function {
        Function::Unary(operation) => {
            let operand = render_function_with_symbols(operation.operand(), symbols)?;
            match operation.operator() {
                crate::UnaryOperator::Neg => format!("-({operand})"),
                crate::UnaryOperator::Abs => format!("abs({operand})"),
                crate::UnaryOperator::Signum => format!("signum({operand})"),
                crate::UnaryOperator::Powi(exponent) => {
                    format!("powi({operand}, {exponent})")
                }
            }
        }
        Function::Nary(operation) => {
            let operands = operation
                .operands()
                .iter()
                .map(|operand| render_function_with_symbols(operand, symbols))
                .collect::<crate::Result<Vec<_>>>()?;
            match operation.operator() {
                crate::NaryOperator::Add => operands
                    .into_iter()
                    .map(|operand| format!("({operand})"))
                    .collect::<Vec<_>>()
                    .join(" + "),
                crate::NaryOperator::Mul => operands
                    .into_iter()
                    .map(|operand| format!("({operand})"))
                    .collect::<Vec<_>>()
                    .join(" * "),
                crate::NaryOperator::Min => format!("min({})", operands.join(", ")),
                crate::NaryOperator::Max => format!("max({})", operands.join(", ")),
            }
        }
        Function::Binary(operation) => {
            let lhs = render_function_with_symbols(operation.lhs(), symbols)?;
            let rhs = render_function_with_symbols(operation.rhs(), symbols)?;
            match operation.operator() {
                crate::BinaryOperator::Div => format!("({lhs}) / ({rhs})"),
            }
        }
        _ => unreachable!("polynomial variants returned above"),
    })
}

pub(crate) fn format_function_with_symbols(
    function: &Function,
    symbols: &BTreeMap<VariableID, String>,
    opts: FunctionFormatOptions,
) -> crate::Result<FormattedFunction> {
    if !function.is_polynomial() {
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
    }

    let mut terms: Vec<_> = function
        .iter()
        .expect("polynomial functions expose terms")
        .map(|(monomial, coefficient)| (monomial, coefficient.into_inner()))
        .collect();
    if terms.is_empty() {
        return Ok(format_zero(opts));
    }
    terms.sort_unstable_by(|(a, _), (b, _)| {
        if a.len() != b.len() {
            b.len().cmp(&a.len())
        } else {
            a.cmp(b)
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
            format!(" - {}", write_term_to_string(&ids, -coefficient, symbols)?)
        } else if index > 0 {
            format!(" + {}", write_term_to_string(&ids, coefficient, symbols)?)
        } else {
            write_term_to_string(&ids, coefficient, symbols)?
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

pub fn format_polynomial(
    f: &mut fmt::Formatter,
    iter: impl Iterator<Item = (MonomialDyn, f64)>,
) -> fmt::Result {
    let mut terms: Vec<_> = iter.collect();
    if terms.is_empty() {
        write!(f, "0")?;
        return Ok(());
    }
    terms.sort_unstable_by(|(a, _), (b, _)| {
        if a.len() != b.len() {
            b.len().cmp(&a.len())
        } else {
            a.cmp(b)
        }
    });

    let mut iter = terms.into_iter();
    let (ids, coefficient) = iter.next().unwrap();
    write_term(f, ids, coefficient)?;

    for (ids, coefficient) in iter {
        if coefficient < 0.0 {
            write!(f, " - ")?;
            write_term(f, ids, -coefficient)?;
        } else {
            write!(f, " + ")?;
            write_term(f, ids, coefficient)?;
        }
    }
    Ok(())
}

impl<M: Monomial> fmt::Display for crate::PolynomialBase<M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.num_terms() == 0 {
            return write!(f, "0");
        }
        format_polynomial(
            f,
            self.iter()
                .map(|(monomial, coefficient)| (monomial.clone().into(), coefficient.into_inner())),
        )
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
            crate::Function::Unary(operation) => match operation.operator() {
                crate::UnaryOperator::Neg => write!(f, "-({})", operation.operand()),
                crate::UnaryOperator::Abs => write!(f, "abs({})", operation.operand()),
                crate::UnaryOperator::Signum => write!(f, "signum({})", operation.operand()),
                crate::UnaryOperator::Powi(exponent) => {
                    write!(f, "powi({}, {exponent})", operation.operand())
                }
            },
            crate::Function::Nary(operation) => {
                let mut operands = operation.operands().iter();
                let first = operands
                    .next()
                    .expect("nary operation always has at least two operands");
                match operation.operator() {
                    crate::NaryOperator::Min | crate::NaryOperator::Max => {
                        let name = match operation.operator() {
                            crate::NaryOperator::Min => "min",
                            crate::NaryOperator::Max => "max",
                            _ => unreachable!(),
                        };
                        write!(f, "{name}({first}")?;
                        for operand in operands {
                            write!(f, ", {operand}")?;
                        }
                        write!(f, ")")
                    }
                    crate::NaryOperator::Add | crate::NaryOperator::Mul => {
                        let separator = match operation.operator() {
                            crate::NaryOperator::Add => " + ",
                            crate::NaryOperator::Mul => " * ",
                            _ => unreachable!(),
                        };
                        write!(f, "({first})")?;
                        for operand in operands {
                            write!(f, "{separator}({operand})")?;
                        }
                        Ok(())
                    }
                }
            }
            crate::Function::Binary(operation) => match operation.operator() {
                crate::BinaryOperator::Div => {
                    write!(f, "({}) / ({})", operation.lhs(), operation.rhs())
                }
            },
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
            crate::Function::Unary(operation) => write!(
                f,
                "Unary({:?}, {:?})",
                operation.operator(),
                operation.operand()
            ),
            crate::Function::Nary(operation) => {
                write!(f, "Nary({:?}, [", operation.operator())?;
                for (index, operand) in operation.operands().iter().enumerate() {
                    if index > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{operand:?}")?;
                }
                write!(f, "])")
            }
            crate::Function::Binary(operation) => write!(
                f,
                "Binary({:?}, {:?}, {:?})",
                operation.operator(),
                operation.lhs(),
                operation.rhs()
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{coeff, linear, quadratic, Linear};

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
