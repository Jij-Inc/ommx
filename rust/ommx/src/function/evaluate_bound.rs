use super::operation::{
    instructions, AssociativeOperator, Atom, BinaryOperator, Instruction, UnaryOperator,
};
use super::*;
use crate::{ATol, Bound, Bounds};
use num::Zero;

fn round_down(value: f64) -> f64 {
    if value.is_finite() && !(value == 0.0 && value.is_sign_positive()) {
        value.next_down()
    } else {
        value
    }
}

fn round_up(value: f64) -> f64 {
    if value.is_finite() && !(value == 0.0 && value.is_sign_negative()) {
        value.next_up()
    } else {
        value
    }
}

fn ensure_representable_endpoint_result(
    operation: &'static str,
    operands_are_finite: bool,
    result: f64,
) -> crate::Result<f64> {
    if operands_are_finite && !result.is_finite() {
        Err(FunctionEvaluationError::NonFiniteResult { operation }.into())
    } else {
        Ok(result)
    }
}

fn add_enclosure(lhs: f64, rhs: f64) -> crate::Result<(f64, f64)> {
    let rounded = ensure_representable_endpoint_result(
        "interval addition",
        lhs.is_finite() && rhs.is_finite(),
        lhs + rhs,
    )?;
    if !rounded.is_finite() {
        return Ok((rounded, rounded));
    }

    // Knuth's TwoSum recovers the exact rounding residual of finite addition.
    // Exact endpoint operations therefore remain points, while inexact ones
    // are expanded only in the direction indicated by the residual.
    let rhs_approximation = rounded - lhs;
    let lhs_approximation = rounded - rhs_approximation;
    let rhs_error = rhs - rhs_approximation;
    let lhs_error = lhs - lhs_approximation;
    let error = lhs_error + rhs_error;
    if error < 0.0 {
        Ok((rounded.next_down(), rounded))
    } else if error > 0.0 {
        Ok((rounded, rounded.next_up()))
    } else {
        Ok((rounded, rounded))
    }
}

fn mul_enclosure(lhs: f64, rhs: f64) -> crate::Result<(f64, f64)> {
    let rounded = ensure_representable_endpoint_result(
        "interval multiplication",
        lhs.is_finite() && rhs.is_finite(),
        lhs * rhs,
    )?;
    if !rounded.is_finite() {
        return Ok((rounded, rounded));
    }
    if rounded == 0.0 {
        return Ok(if lhs == 0.0 || rhs == 0.0 {
            (rounded, rounded)
        } else {
            // A non-zero exact product underflowed below the smallest
            // subnormal. Preserve both possible directions around zero.
            (rounded.next_down(), rounded.next_up())
        });
    }
    if rounded.is_subnormal() {
        // The fused residual can itself underflow in the subnormal range.
        return Ok((rounded.next_down(), rounded.next_up()));
    }

    // The fused multiply-add recovers the exact rounding residual when the
    // rounded product is normal.
    let error = lhs.mul_add(rhs, -rounded);
    if error < 0.0 {
        Ok((rounded.next_down(), rounded))
    } else if error > 0.0 {
        Ok((rounded, rounded.next_up()))
    } else if rounded.abs() < f64::from_bits(54_u64 << 52) {
        // A binary64 product has at most 106 significant bits. Below 2^-969,
        // its exact residual can fall below the smallest subnormal even when
        // the rounded product is normal, so an FMA result of zero does not
        // prove exactness.
        Ok((rounded.next_down(), rounded.next_up()))
    } else {
        Ok((rounded, rounded))
    }
}

fn add_bounds(lhs: Bound, rhs: Bound) -> crate::Result<Bound> {
    // Addition by an exact zero interval is an identity, so no rounding is
    // needed. This also avoids widening every polynomial when terms are added
    // to the zero accumulator.
    if lhs == Bound::zero() {
        return Ok(rhs);
    }
    if rhs == Bound::zero() {
        return Ok(lhs);
    }
    let (lower, _) = add_enclosure(lhs.lower(), rhs.lower())?;
    let (_, upper) = add_enclosure(lhs.upper(), rhs.upper())?;
    if lower.is_nan() || upper.is_nan() {
        Ok(Bound::default())
    } else {
        Ok(Bound::new(lower, upper)?)
    }
}

fn mul_bounds(lhs: Bound, rhs: Bound) -> crate::Result<Bound> {
    if lhs == Bound::zero() || rhs == Bound::zero() {
        return Ok(Bound::zero());
    }
    let one = Bound::new(1.0, 1.0).expect("one is a valid bound");
    if lhs == one {
        return Ok(rhs);
    }
    if rhs == one {
        return Ok(lhs);
    }
    let products = [
        mul_enclosure(lhs.lower(), rhs.lower())?,
        mul_enclosure(lhs.lower(), rhs.upper())?,
        mul_enclosure(lhs.upper(), rhs.lower())?,
        mul_enclosure(lhs.upper(), rhs.upper())?,
    ];
    if products
        .iter()
        .any(|(lower, upper)| lower.is_nan() || upper.is_nan())
    {
        return Ok(Bound::default());
    }
    let lower = products
        .iter()
        .map(|(lower, _)| *lower)
        .fold(f64::INFINITY, f64::min);
    let upper = products
        .iter()
        .map(|(_, upper)| *upper)
        .fold(f64::NEG_INFINITY, f64::max);
    Ok(Bound::new(lower, upper)?)
}

fn div_enclosure(
    lhs: f64,
    rhs: f64,
    zero_denominator_is_unattainable: bool,
) -> crate::Result<(f64, f64)> {
    let rounded = ensure_representable_endpoint_result(
        "interval division",
        lhs.is_finite() && rhs.is_finite() && !(rhs == 0.0 && zero_denominator_is_unattainable),
        lhs / rhs,
    )?;
    if rounded.is_nan() || rounded.is_infinite() {
        return Ok((rounded, rounded));
    }
    Ok((round_down(rounded), round_up(rounded)))
}

fn div_bounds(
    lhs: Bound,
    rhs: Bound,
    zero_denominator_is_unattainable: bool,
) -> crate::Result<Bound> {
    let quotients = [
        div_enclosure(lhs.lower(), rhs.lower(), zero_denominator_is_unattainable)?,
        div_enclosure(lhs.lower(), rhs.upper(), zero_denominator_is_unattainable)?,
        div_enclosure(lhs.upper(), rhs.lower(), zero_denominator_is_unattainable)?,
        div_enclosure(lhs.upper(), rhs.upper(), zero_denominator_is_unattainable)?,
    ];
    if quotients
        .iter()
        .any(|(lower, upper)| lower.is_nan() || upper.is_nan())
    {
        return Ok(Bound::default());
    }
    let lower = quotients
        .iter()
        .map(|(lower, _)| *lower)
        .fold(f64::INFINITY, f64::min);
    let upper = quotients
        .iter()
        .map(|(_, upper)| *upper)
        .fold(f64::NEG_INFINITY, f64::max);
    Ok(Bound::new(lower, upper)?)
}

fn reciprocal_bound(bound: Bound) -> crate::Result<Bound> {
    // The caller has already rejected a base that can actually evaluate to
    // zero. A zero endpoint here can therefore only be the unattained limit of
    // a reciprocal over an unbounded finite input interval.
    let (lower, _) = div_enclosure(1.0, bound.upper(), true)?;
    let (_, upper) = div_enclosure(1.0, bound.lower(), true)?;
    Ok(Bound::new(lower, upper)?)
}

fn point_power_enclosure(value: f64, exponent: u64) -> crate::Result<(f64, f64)> {
    if value.is_infinite() {
        // `Bound` intentionally cannot represent a point at either infinity.
        // Keep the endpoint enclosure as a pair until both base endpoints are
        // combined into the (valid) range of the whole power operation.
        let powered = if value.is_sign_positive() || exponent.is_multiple_of(2) {
            f64::INFINITY
        } else {
            f64::NEG_INFINITY
        };
        return Ok((powered, powered));
    }

    let mut result = Bound::new(1.0, 1.0)?;
    let mut factor = Bound::new(value, value)?;
    let mut remaining = exponent;
    while remaining > 0 {
        if remaining & 1 == 1 {
            result = mul_bounds(result, factor)?;
        }
        remaining >>= 1;
        if remaining > 0 {
            factor = mul_bounds(factor, factor)?;
        }
    }
    Ok((result.lower(), result.upper()))
}

fn left_fold_point_power_enclosure(value: f64, exponent: u64) -> crate::Result<(f64, f64)> {
    if value.is_infinite() {
        let powered = if value.is_sign_positive() || exponent.is_multiple_of(2) {
            f64::INFINITY
        } else {
            f64::NEG_INFINITY
        };
        return Ok((powered, powered));
    }

    let factor = Bound::new(value, value)?;
    let mut result = Bound::new(1.0, 1.0)?;
    for _ in 0..exponent {
        result = mul_bounds(result, factor)?;
    }
    Ok((result.lower(), result.upper()))
}

fn left_fold_positive_integer_power_bound(base: Bound, exponent: u64) -> crate::Result<Bound> {
    if exponent == 0 {
        return Ok(Bound::new(1.0, 1.0).unwrap());
    }
    if exponent == 1 {
        return Ok(base);
    }

    let lower_endpoint = left_fold_point_power_enclosure(base.lower(), exponent)?;
    let upper_endpoint = left_fold_point_power_enclosure(base.upper(), exponent)?;
    if exponent.is_multiple_of(2) && base.lower() <= 0.0 && base.upper() >= 0.0 {
        Ok(Bound::new(0.0, lower_endpoint.1.max(upper_endpoint.1))?)
    } else {
        Ok(Bound::new(
            lower_endpoint.0.min(upper_endpoint.0),
            lower_endpoint.1.max(upper_endpoint.1),
        )?)
    }
}

fn positive_integer_power_bound(base: Bound, exponent: u64) -> crate::Result<Bound> {
    if exponent == 0 {
        return Ok(Bound::new(1.0, 1.0).unwrap());
    }
    if exponent == 1 {
        return Ok(base);
    }

    let lower_endpoint = point_power_enclosure(base.lower(), exponent)?;
    let upper_endpoint = point_power_enclosure(base.upper(), exponent)?;
    if exponent.is_multiple_of(2) && base.lower() <= 0.0 && base.upper() >= 0.0 {
        // Zero is the exact minimum of an even power across zero. Only the
        // larger-magnitude endpoint can attain the maximum.
        Ok(Bound::new(0.0, lower_endpoint.1.max(upper_endpoint.1))?)
    } else {
        Ok(Bound::new(
            lower_endpoint.0.min(upper_endpoint.0),
            lower_endpoint.1.max(upper_endpoint.1),
        )?)
    }
}

fn integer_power_bound(
    base: Bound,
    exponent: i32,
    base_may_evaluate_to_zero: bool,
) -> crate::Result<Bound> {
    if exponent == 0 {
        return Ok(Bound::new(1.0, 1.0).unwrap());
    }
    if exponent < 0 && base_may_evaluate_to_zero {
        anyhow::bail!(
            "cannot bound a negative power when the base may be classified as zero at the caller tolerance"
        )
    }

    let scalar_endpoints = [
        ensure_representable_endpoint_result(
            "integer power interval endpoint",
            base.lower().is_finite()
                && !(exponent < 0 && base.lower() == 0.0 && !base_may_evaluate_to_zero),
            base.lower().powi(exponent),
        )?,
        ensure_representable_endpoint_result(
            "integer power interval endpoint",
            base.upper().is_finite()
                && !(exponent < 0 && base.upper() == 0.0 && !base_may_evaluate_to_zero),
            base.upper().powi(exponent),
        )?,
    ];
    if scalar_endpoints.iter().any(|value| value.is_nan()) {
        anyhow::bail!("integer power is undefined at an endpoint of the base interval")
    }

    let powered_base = if exponent < 0 {
        // Invert first. Computing the positive power before the reciprocal can
        // overflow even when the mathematical negative power is finite. The
        // scalar `powi` endpoint values are included separately below because
        // its implementation is allowed to take that overflowing route.
        reciprocal_bound(base)?
    } else {
        base
    };
    let mathematical =
        positive_integer_power_bound(powered_base, u64::from(exponent.unsigned_abs()))?;
    let lower = scalar_endpoints
        .iter()
        .copied()
        .fold(mathematical.lower(), f64::min);
    let upper = scalar_endpoints
        .iter()
        .copied()
        .fold(mathematical.upper(), f64::max);
    Ok(Bound::new(lower, upper)?)
}

fn bound_contains_zero(bound: Bound) -> bool {
    bound.lower() <= 0.0 && bound.upper() >= 0.0
}

fn bound_may_be_classified_as_zero(bound: Bound, atol: ATol) -> bool {
    // This is interval intersection with the inclusive approximate-zero
    // domain, not a scalar approximate-equality comparison.
    bound.lower() <= *atol && bound.upper() >= -*atol
}

fn minimum_absolute_value(bound: Bound) -> f64 {
    if bound_contains_zero(bound) {
        0.0
    } else {
        bound.lower().abs().min(bound.upper().abs())
    }
}

fn maximum_finite_absolute_value(bound: Bound) -> f64 {
    let finite_abs = |value: f64| {
        if value.is_infinite() {
            f64::MAX
        } else {
            value.abs()
        }
    };
    finite_abs(bound.lower()).max(finite_abs(bound.upper()))
}

#[derive(Clone, Copy)]
struct BoundAnalysis {
    bound: Bound,
    /// Whether a finite binary64 evaluation can be classified as zero by the
    /// caller-provided tolerance. This can be stricter than checking whether
    /// the interval enclosure intersects `[-atol, atol]` because infinite
    /// endpoints represent unbounded finite values rather than attainable
    /// infinities.
    may_evaluate_to_zero: bool,
}

fn evaluate_polynomial_terms_bound<'a, M: crate::Monomial>(
    terms: impl Iterator<Item = (&'a M, &'a Coefficient)>,
    bounds: &Bounds,
) -> crate::Result<Bound> {
    let mut bound = Bound::zero();
    for (monomial, coefficient) in terms {
        let value = coefficient.into_inner();
        let mut ids = monomial.ids().peekable();
        if ids.peek().is_none() {
            bound = add_bounds(bound, Bound::new(value, value)?)?;
            continue;
        }
        let mut cur = Bound::new(1.0, 1.0)?;
        // Match PolynomialBase::evaluate's left-to-right repeated-ID fold.
        // The per-variable power still preserves correlation (for example,
        // an even power across zero has lower bound zero), unlike naively
        // multiplying the same interval by itself.
        while let Some(id) = ids.next() {
            let mut exponent = 1_usize;
            while ids.peek().is_some_and(|next| *next == id) {
                ids.next();
                exponent += 1;
            }
            let variable_bound = bounds.get(&id).cloned().unwrap_or_default();
            let exponent = u64::try_from(exponent).map_err(|_| {
                anyhow::anyhow!("monomial exponent is outside the supported integer range")
            })?;
            cur = mul_bounds(
                cur,
                left_fold_positive_integer_power_bound(variable_bound, exponent)?,
            )?;
        }
        let coefficient = Bound::new(value, value)?;
        bound = add_bounds(bound, mul_bounds(coefficient, cur)?)?;
    }
    Ok(bound)
}

fn analyze_polynomial_bound(bound: Bound, atol: ATol) -> BoundAnalysis {
    BoundAnalysis {
        bound,
        may_evaluate_to_zero: bound_may_be_classified_as_zero(bound, atol),
    }
}

fn analyze_atom(atom: &Atom, bounds: &Bounds, atol: ATol) -> crate::Result<BoundAnalysis> {
    match atom {
        Atom::Zero => Ok(BoundAnalysis {
            bound: Bound::zero(),
            may_evaluate_to_zero: true,
        }),
        Atom::Constant(value) => {
            let value = value.into_inner();
            Ok(BoundAnalysis {
                bound: Bound::new(value, value)?,
                may_evaluate_to_zero: atol.approx_is_zero(value),
            })
        }
        Atom::Linear(function) => Ok(analyze_polynomial_bound(
            evaluate_polynomial_terms_bound(function.iter(), bounds)?,
            atol,
        )),
        Atom::Quadratic(function) => Ok(analyze_polynomial_bound(
            evaluate_polynomial_terms_bound(function.iter(), bounds)?,
            atol,
        )),
        Atom::Polynomial(function) => Ok(analyze_polynomial_bound(
            evaluate_polynomial_terms_bound(function.iter(), bounds)?,
            atol,
        )),
    }
}

fn analyze_unary_bound(
    operator: UnaryOperator,
    operand: BoundAnalysis,
    atol: ATol,
) -> crate::Result<BoundAnalysis> {
    let mut signum_zero_possible = false;
    let mut signum_nonzero_possible = false;
    let bound = match operator {
        UnaryOperator::Neg => Bound::new(-operand.bound.upper(), -operand.bound.lower())?,
        UnaryOperator::Abs if operand.bound.lower() >= 0.0 => operand.bound,
        UnaryOperator::Abs if operand.bound.upper() <= 0.0 => {
            Bound::new(-operand.bound.upper(), -operand.bound.lower())?
        }
        UnaryOperator::Abs => Bound::new(
            0.0,
            operand.bound.lower().abs().max(operand.bound.upper().abs()),
        )?,
        UnaryOperator::Signum => {
            // These strict endpoint checks are the complement of the inclusive
            // `[-atol, atol]` zero classification.
            let negative_possible = operand.bound.lower() < -*atol;
            signum_zero_possible = operand.may_evaluate_to_zero;
            let positive_possible = operand.bound.upper() > *atol;
            signum_nonzero_possible = negative_possible || positive_possible;

            let lower = if negative_possible {
                -1.0
            } else if signum_zero_possible {
                0.0
            } else {
                1.0
            };
            let upper = if positive_possible {
                1.0
            } else if signum_zero_possible {
                0.0
            } else {
                -1.0
            };
            Bound::new(lower, upper)?
        }
        UnaryOperator::Powi(exponent) => {
            integer_power_bound(operand.bound, exponent, operand.may_evaluate_to_zero)?
        }
    };

    let may_evaluate_to_zero = match operator {
        UnaryOperator::Powi(0) => atol.approx_is_zero(1.0),
        UnaryOperator::Powi(1) => operand.may_evaluate_to_zero,
        UnaryOperator::Powi(exponent) => {
            let value = if exponent < 0 {
                maximum_finite_absolute_value(operand.bound)
            } else {
                minimum_absolute_value(operand.bound)
            };
            atol.approx_is_zero(value.powi(exponent))
        }
        UnaryOperator::Neg | UnaryOperator::Abs => operand.may_evaluate_to_zero,
        UnaryOperator::Signum => {
            signum_zero_possible || (signum_nonzero_possible && atol.approx_is_zero(1.0))
        }
    };
    Ok(BoundAnalysis {
        bound,
        may_evaluate_to_zero,
    })
}

fn analyze_associative_bound(
    operator: AssociativeOperator,
    lhs: BoundAnalysis,
    rhs: BoundAnalysis,
    atol: ATol,
) -> crate::Result<BoundAnalysis> {
    let bound = match operator {
        AssociativeOperator::Add => add_bounds(lhs.bound, rhs.bound)?,
        AssociativeOperator::Mul => mul_bounds(lhs.bound, rhs.bound)?,
        AssociativeOperator::Min => Bound::new(
            lhs.bound.lower().min(rhs.bound.lower()),
            lhs.bound.upper().min(rhs.bound.upper()),
        )?,
        AssociativeOperator::Max => Bound::new(
            lhs.bound.lower().max(rhs.bound.lower()),
            lhs.bound.upper().max(rhs.bound.upper()),
        )?,
    };
    let interval_may_evaluate_to_zero = bound_may_be_classified_as_zero(bound, atol);
    let may_evaluate_to_zero = match operator {
        // Same-sign addition cannot reduce either operand's absolute value.
        AssociativeOperator::Add
            if (lhs.bound.lower() >= 0.0 && rhs.bound.lower() >= 0.0)
                || (lhs.bound.upper() <= 0.0 && rhs.bound.upper() <= 0.0) =>
        {
            lhs.may_evaluate_to_zero && rhs.may_evaluate_to_zero
        }
        // Multiplication by a factor whose magnitude is always at least one
        // cannot erase an operand's proof that it stays outside the caller's
        // zero tolerance.
        AssociativeOperator::Mul
            if (!lhs.may_evaluate_to_zero && minimum_absolute_value(rhs.bound) >= 1.0)
                || (!rhs.may_evaluate_to_zero && minimum_absolute_value(lhs.bound) >= 1.0) =>
        {
            false
        }
        // Min and max return one of their operands, so they can be classified
        // as zero only if at least one operand can be.
        AssociativeOperator::Min | AssociativeOperator::Max => {
            lhs.may_evaluate_to_zero || rhs.may_evaluate_to_zero
        }
        _ => interval_may_evaluate_to_zero,
    };
    Ok(BoundAnalysis {
        bound,
        may_evaluate_to_zero,
    })
}

fn analyze_binary_bound(
    operator: BinaryOperator,
    lhs: BoundAnalysis,
    rhs: BoundAnalysis,
    atol: ATol,
) -> crate::Result<BoundAnalysis> {
    match operator {
        BinaryOperator::Div => {
            if rhs.may_evaluate_to_zero {
                anyhow::bail!(
                    "cannot bound division when the denominator may be classified as zero at the caller tolerance"
                )
            }
            // A zero endpoint can arise as the unattained limit of a previous
            // reciprocal over an unbounded finite input. `may_evaluate_to_zero`
            // distinguishes that sentinel from an attainable denominator.
            let bound = div_bounds(lhs.bound, rhs.bound, !rhs.may_evaluate_to_zero)?;
            let may_evaluate_to_zero =
                if !lhs.may_evaluate_to_zero && maximum_finite_absolute_value(rhs.bound) <= 1.0 {
                    // Division by a nonzero value whose magnitude is at most one
                    // cannot erase the numerator's proof that it stays outside
                    // the caller's zero tolerance.
                    false
                } else {
                    lhs.may_evaluate_to_zero
                        || atol.approx_is_zero(
                            minimum_absolute_value(lhs.bound)
                                / maximum_finite_absolute_value(rhs.bound),
                        )
                };
            Ok(BoundAnalysis {
                bound,
                may_evaluate_to_zero,
            })
        }
    }
}

fn analyze_expression_bound(
    expression: &Expression,
    bounds: &Bounds,
    atol: ATol,
) -> crate::Result<BoundAnalysis> {
    let mut values = Vec::new();
    for instruction in instructions(expression) {
        match instruction {
            Instruction::Push(atom) => values.push(analyze_atom(atom, bounds, atol)?),
            Instruction::Unary(operator) => {
                let operand = values
                    .pop()
                    .expect("validated unary instruction has an operand");
                values.push(analyze_unary_bound(*operator, operand, atol)?);
            }
            Instruction::Associative(operator) => {
                let rhs = values
                    .pop()
                    .expect("validated associative instruction has a right operand");
                let lhs = values
                    .pop()
                    .expect("validated associative instruction has a left operand");
                values.push(analyze_associative_bound(*operator, lhs, rhs, atol)?);
            }
            Instruction::Binary(operator) => {
                let rhs = values
                    .pop()
                    .expect("validated binary instruction has a right operand");
                let lhs = values
                    .pop()
                    .expect("validated binary instruction has a left operand");
                values.push(analyze_binary_bound(*operator, lhs, rhs, atol)?);
            }
        }
    }
    Ok(values
        .pop()
        .expect("validated expression leaves one result"))
}

impl Function {
    #[cfg_attr(doc, katexit::katexit)]
    /// Compute an interval bound of this function given variable bounds.
    ///
    /// Missing IDs in `bounds` are treated as `Bound::default()` (unbounded).
    /// The same `atol` used for point evaluation must be supplied here: values
    /// whose absolute value is less than or equal to `atol` are classified as
    /// zero by [`Function::signum`], division, and negative integer powers.
    ///
    /// # Tightness
    ///
    /// Polynomial leaves are evaluated **term by term** (monomial-wise), while
    /// composed expression nodes combine operand bounds with interval
    /// arithmetic. The result is a **sound over-approximation** of the true
    /// range $[\inf f, \sup f]$ but is **not guaranteed to be tight**, because
    /// it ignores dependencies between terms or operands that share variables.
    ///
    /// For example, $f = x^2 - x$ with $x \in [0, 1]$ has true range
    /// $[-1/4, 0]$ (minimum at $x = 1/2$), but term-wise evaluation yields
    /// $[0, 1] + (-[0, 1]) = [-1, 1]$.
    ///
    /// # Errors
    ///
    /// Returns an error when an interval may evaluate into the `[-atol, atol]`
    /// zero domain of a division or negative integer power, or when finite
    /// interval endpoints cannot be represented.
    pub fn evaluate_bound(&self, bounds: &Bounds, atol: ATol) -> crate::Result<Bound> {
        let analysis = match self {
            Function::Zero => BoundAnalysis {
                bound: Bound::zero(),
                may_evaluate_to_zero: true,
            },
            Function::Constant(value) => {
                let value = value.into_inner();
                BoundAnalysis {
                    bound: Bound::new(value, value)?,
                    may_evaluate_to_zero: atol.approx_is_zero(value),
                }
            }
            Function::Linear(function) => analyze_polynomial_bound(
                evaluate_polynomial_terms_bound(function.iter(), bounds)?,
                atol,
            ),
            Function::Quadratic(function) => analyze_polynomial_bound(
                evaluate_polynomial_terms_bound(function.iter(), bounds)?,
                atol,
            ),
            Function::Polynomial(function) => analyze_polynomial_bound(
                evaluate_polynomial_terms_bound(function.iter(), bounds)?,
                atol,
            ),
            Function::Expression(expression) => analyze_expression_bound(expression, bounds, atol)?,
        };
        Ok(analysis.bound)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        coeff, linear, quadratic, ATol, Bound, Evaluate, MonomialDyn, Polynomial, VariableID,
    };

    fn assert_contains(bound: Bound, value: f64) {
        assert!(
            bound.lower() <= value && value <= bound.upper(),
            "{bound} does not contain {value:?}",
        );
    }

    #[test]
    fn bound_of_constant() {
        let f = Function::Constant(coeff!(3.5));
        assert_eq!(
            f.evaluate_bound(&Bounds::new(), ATol::default()).unwrap(),
            Bound::new(3.5, 3.5).unwrap()
        );
    }

    #[test]
    fn bound_of_linear() {
        // f = 2*x1 + 3 with x1 in [0, 2]
        let f = Function::from(((coeff!(2.0) * linear!(1)).unwrap() + coeff!(3.0)).unwrap());
        let mut bounds = Bounds::new();
        bounds.insert(VariableID::from(1), Bound::new(0.0, 2.0).unwrap());
        let bound = f.evaluate_bound(&bounds, ATol::default()).unwrap();
        assert_contains(bound, 3.0);
        assert_contains(bound, 7.0);
    }

    #[test]
    fn bound_of_quadratic_with_squared_term() {
        // f = x1*x1 with x1 in [-2, 3].
        //
        // `Function::evaluate_bound` collapses each repeated-ID run and bounds
        // its integer power. For an even
        // exponent across zero this yields [0, max(|-2|^2, 3^2)] = [0, 9], not
        // a naive interval-square [-6, 9].
        let f = Function::from(quadratic!(1, 1));
        let mut bounds = Bounds::new();
        bounds.insert(VariableID::from(1), Bound::new(-2.0, 3.0).unwrap());
        let bound = f.evaluate_bound(&bounds, ATol::default()).unwrap();
        assert_contains(bound, 0.0);
        assert_contains(bound, 9.0);
    }

    #[test]
    fn bound_missing_id_is_unbounded() {
        let f = Function::from(linear!(1));
        assert_eq!(
            f.evaluate_bound(&Bounds::new(), ATol::default()).unwrap(),
            Bound::default()
        );
    }

    #[test]
    fn bound_of_missing_id_integer_powers_uses_unbounded_interval() {
        let id = VariableID::from(1);
        let square = Function::from(quadratic!(id, id));
        let cube = Function::Polynomial(Polynomial::single_term(
            MonomialDyn::new(vec![id; 3]),
            coeff!(1.0),
        ));

        assert_eq!(
            square
                .evaluate_bound(&Bounds::new(), ATol::default())
                .unwrap(),
            Bound::positive()
        );
        assert_eq!(
            cube.evaluate_bound(&Bounds::new(), ATol::default())
                .unwrap(),
            Bound::default()
        );
    }

    #[test]
    fn negative_integer_power_handles_unbounded_endpoints_and_zero_domain() {
        let id = VariableID::from(1);
        let x = Function::from(linear!(id));
        let inverse_square = x.clone().powi(-2);
        let inverse_cube = x.powi(-3);

        let positive_half_line = Bounds::from([(id, Bound::new(1.0, f64::INFINITY).unwrap())]);
        let positive_bound = inverse_square
            .evaluate_bound(&positive_half_line, ATol::default())
            .unwrap();
        assert_contains(positive_bound, 0.0);
        assert_contains(positive_bound, 1.0);

        let negative_half_line = Bounds::from([(id, Bound::new(f64::NEG_INFINITY, -1.0).unwrap())]);
        let negative_bound = inverse_cube
            .evaluate_bound(&negative_half_line, ATol::default())
            .unwrap();
        assert_contains(negative_bound, -1.0);
        assert_contains(negative_bound, 0.0);

        assert!(inverse_square
            .evaluate_bound(&Bounds::new(), ATol::default())
            .is_err());
    }

    #[test]
    fn bound_is_sound_over_approximation_not_tight() {
        // f = x^2 - x with x in [0, 1].
        //
        // Term-wise evaluation: [0,1]^2 + (-[0,1]) = [0,1] + [-1,0] = [-1, 1].
        // True range: f'(x) = 2x - 1 = 0 at x = 1/2, so min = -1/4 and max = 0.
        //
        // Pins the documented tightness caveat: the returned bound contains
        // the true range but is strictly wider here.
        let f = (Function::from(quadratic!(1, 1))
            + Function::from((-coeff!(1.0) * linear!(1)).unwrap()))
        .unwrap();
        let mut bounds = Bounds::new();
        bounds.insert(VariableID::from(1), Bound::new(0.0, 1.0).unwrap());
        let bound = f.evaluate_bound(&bounds, ATol::default()).unwrap();
        assert_contains(bound, -1.0);
        assert_contains(bound, 1.0);
    }

    #[test]
    fn bound_of_composite_operations() {
        let x = Function::from(linear!(1));
        let mut bounds = Bounds::new();
        bounds.insert(VariableID::from(1), Bound::new(-2.0, 3.0).unwrap());

        let absolute = x
            .clone()
            .abs()
            .evaluate_bound(&bounds, ATol::default())
            .unwrap();
        assert_contains(absolute, 0.0);
        assert_contains(absolute, 3.0);

        let minimum = x
            .clone()
            .min(Function::try_from(1.0).unwrap())
            .evaluate_bound(&bounds, ATol::default())
            .unwrap();
        assert_contains(minimum, -2.0);
        assert_contains(minimum, 1.0);

        let square = x.powi(2).evaluate_bound(&bounds, ATol::default()).unwrap();
        assert_contains(square, 0.0);
        assert_contains(square, 9.0);
    }

    #[test]
    fn bound_rejects_intervals_that_cross_an_undefined_domain() {
        let x = Function::from(linear!(1));
        let reciprocal = (Function::one() / x.clone()).unwrap();
        let mut bounds = Bounds::new();
        bounds.insert(VariableID::from(1), Bound::new(-1.0, 1.0).unwrap());
        assert!(reciprocal.evaluate_bound(&bounds, ATol::default()).is_err());

        let inverse = x.powi(-1);
        assert!(inverse.evaluate_bound(&bounds, ATol::default()).is_err());
    }

    #[test]
    fn point_bound_uses_the_same_atol_as_signum_evaluation() {
        let id = VariableID::from(1);
        let atol = ATol::new(1e-6).unwrap();
        let value = 1e-8;
        let function = (Function::from(linear!(id)).signum() - coeff!(0.5)).unwrap();
        let state = crate::v1::State::from_iter([(1, value)]);
        let bounds = Bounds::from([(id, Bound::new(value, value).unwrap())]);

        let evaluated = function.evaluate(&state, atol).unwrap();
        assert_eq!(evaluated, -0.5);
        assert_contains(function.evaluate_bound(&bounds, atol).unwrap(), evaluated);
    }

    #[test]
    fn point_bound_rejects_zero_sensitive_domains_at_the_atol_boundary() {
        let id = VariableID::from(1);
        let atol = ATol::new(1e-6).unwrap();
        let x = Function::from(linear!(id));
        let reciprocal = (Function::one() / x.clone()).unwrap();
        let inverse = x.powi(-1);

        for value in [*atol, -*atol] {
            let state = crate::v1::State::from_iter([(1, value)]);
            let bounds = Bounds::from([(id, Bound::new(value, value).unwrap())]);

            assert!(reciprocal.evaluate(&state, atol).is_err());
            assert!(reciprocal.evaluate_bound(&bounds, atol).is_err());
            assert!(inverse.evaluate(&state, atol).is_err());
            assert!(inverse.evaluate_bound(&bounds, atol).is_err());
        }

        for value in [2.0 * *atol, -2.0 * *atol] {
            let state = crate::v1::State::from_iter([(1, value)]);
            let bounds = Bounds::from([(id, Bound::new(value, value).unwrap())]);

            let reciprocal_value = reciprocal.evaluate(&state, atol).unwrap();
            assert_contains(
                reciprocal.evaluate_bound(&bounds, atol).unwrap(),
                reciprocal_value,
            );
            let inverse_value = inverse.evaluate(&state, atol).unwrap();
            assert_contains(
                inverse.evaluate_bound(&bounds, atol).unwrap(),
                inverse_value,
            );
        }
    }

    #[test]
    fn nested_zero_sensitive_domain_uses_the_caller_atol() {
        let id = VariableID::from(1);
        let atol = ATol::new(1e-6).unwrap();
        let value = 1e8;
        let x = Function::from(linear!(id));
        let reciprocal = (Function::one() / x).unwrap();
        let inverse_reciprocal = reciprocal.powi(-1);
        let state = crate::v1::State::from_iter([(1, value)]);
        let bounds = Bounds::from([(id, Bound::new(value, value).unwrap())]);

        assert!(inverse_reciprocal.evaluate(&state, atol).is_err());
        assert!(inverse_reciprocal.evaluate_bound(&bounds, atol).is_err());
    }

    #[test]
    fn polynomial_bound_preserves_exponents_larger_than_u8() {
        let id = VariableID::from(1);
        let monomial = MonomialDyn::new(vec![id; 256]);
        let function = Function::Polynomial(Polynomial::single_term(monomial, coeff!(1.0)));
        let bounds = Bounds::from([(id, Bound::new(2.0, 2.0).unwrap())]);
        let expected = 2.0_f64.powi(256);

        assert_contains(
            function.evaluate_bound(&bounds, ATol::default()).unwrap(),
            expected,
        );
    }

    #[test]
    fn division_bound_contains_scalar_evaluation_despite_reciprocal_rounding() {
        let x = Function::from(linear!(1));
        let y = Function::from(linear!(2));
        let function = (x / y).unwrap();
        let state = crate::v1::State::from_iter([(1, 0.1), (2, 10.1)]);
        let bounds = Bounds::from([
            (VariableID::from(1), Bound::new(0.1, 0.1).unwrap()),
            (VariableID::from(2), Bound::new(10.1, 10.1).unwrap()),
        ]);
        let atol = ATol::default();

        let value = function.evaluate(&state, atol).unwrap();
        assert_contains(function.evaluate_bound(&bounds, atol).unwrap(), value);
    }

    #[test]
    fn polynomial_power_bound_contains_left_to_right_scalar_evaluation() {
        let id = VariableID::from(1);
        for (value, exponent) in [(0.1, 4), (2.1, 923), (0.906_234_158_714_408_3, 10_000)] {
            let monomial = MonomialDyn::new(vec![id; exponent]);
            let function = Function::Polynomial(Polynomial::single_term(monomial, coeff!(1.0)));
            let state = crate::v1::State::from_iter([(1, value)]);
            let bounds = Bounds::from([(id, Bound::new(value, value).unwrap())]);
            let atol = ATol::default();

            let evaluated = function.evaluate(&state, atol).unwrap();
            assert_contains(function.evaluate_bound(&bounds, atol).unwrap(), evaluated);
        }
    }

    #[test]
    fn integer_power_bound_contains_powi_scalar_evaluation() {
        let id = VariableID::from(1);
        let function = Function::from(linear!(id)).powi(4);
        let state = crate::v1::State::from_iter([(1, 0.1)]);
        let bounds = Bounds::from([(id, Bound::new(0.1, 0.1).unwrap())]);
        let atol = ATol::default();

        let evaluated = function.evaluate(&state, atol).unwrap();
        assert_contains(function.evaluate_bound(&bounds, atol).unwrap(), evaluated);
    }

    #[test]
    fn integer_power_handles_i32_min_without_overflowing_the_magnitude() {
        let id = VariableID::from(1);
        let function = Function::from(linear!(id)).powi(i32::MIN);
        let state = crate::v1::State::from_iter([(1, -1.0)]);
        let bounds = Bounds::from([(id, Bound::new(-1.0, -1.0).unwrap())]);
        let atol = ATol::default();

        let evaluated = function.evaluate(&state, atol).unwrap();
        assert_eq!(evaluated, 1.0);
        assert_contains(function.evaluate_bound(&bounds, atol).unwrap(), evaluated);
    }

    #[test]
    fn negative_integer_power_inverts_before_exponentiation() {
        let id = VariableID::from(1);
        for (base, exponent) in [(1e200, -2), (2.0, -1024)] {
            let function = Function::from(linear!(id)).powi(exponent);
            let state = crate::v1::State::from_iter([(1, base)]);
            let bounds = Bounds::from([(id, Bound::new(base, base).unwrap())]);
            let atol = ATol::default();
            let evaluated = function.evaluate(&state, atol).unwrap();

            assert_contains(function.evaluate_bound(&bounds, atol).unwrap(), evaluated);
        }
    }

    #[test]
    fn division_avoids_an_overflowing_reciprocal_intermediate() {
        let nonzero = f64::from_bits(2);
        let atol = ATol::new(f64::from_bits(1)).unwrap();
        let lhs = Function::from(linear!(1));
        let rhs = Function::from(linear!(2));
        let function = (lhs / rhs).unwrap();
        let state = crate::v1::State::from_iter([(1, nonzero), (2, nonzero)]);
        let bounds = Bounds::from([
            (VariableID::from(1), Bound::new(nonzero, nonzero).unwrap()),
            (VariableID::from(2), Bound::new(nonzero, nonzero).unwrap()),
        ]);
        let evaluated = function.evaluate(&state, atol).unwrap();

        assert_eq!(evaluated, 1.0);
        assert_contains(function.evaluate_bound(&bounds, atol).unwrap(), evaluated);
    }

    #[test]
    fn nested_operations_do_not_invent_a_zero_domain() {
        let id = VariableID::from(1);
        let x = Function::from(linear!(id));
        let bounds = Bounds::from([(id, Bound::new(1.0, f64::INFINITY).unwrap())]);
        let atol = ATol::new(f64::from_bits(1)).unwrap();

        let reciprocal = (Function::one() / x.clone()).unwrap();
        let inverse_reciprocal = reciprocal.clone().powi(-1);
        assert!(inverse_reciprocal.evaluate_bound(&bounds, atol).is_ok());

        let zero_power = reciprocal.clone().powi(0);
        assert_eq!(
            zero_power.evaluate_bound(&bounds, atol).unwrap(),
            Bound::new(1.0, 1.0).unwrap()
        );

        let reciprocal_of_reciprocal = (Function::one() / reciprocal).unwrap();
        assert!(reciprocal_of_reciprocal
            .evaluate_bound(&bounds, atol)
            .is_ok());

        let zero_to_one = Bounds::from([(id, Bound::new(0.0, 1.0).unwrap())]);
        let undefined_reciprocal = x.powi(-1);
        assert!(undefined_reciprocal
            .evaluate_bound(&zero_to_one, atol)
            .is_err());
    }

    #[test]
    fn non_contracting_operations_preserve_an_unattained_zero_endpoint() {
        let id = VariableID::from(1);
        let x = Function::from(linear!(id));
        let bounds = Bounds::from([(id, Bound::new(1.0, f64::INFINITY).unwrap())]);
        let atol = ATol::new(f64::from_bits(1)).unwrap();
        let reciprocal = (Function::one() / x).unwrap();
        let doubled = (Function::try_from(2.0).unwrap() * reciprocal.clone()).unwrap();
        let identity_power =
            super::super::operation::unary_expression(UnaryOperator::Powi(1), reciprocal.clone());

        let non_contracting = [
            (reciprocal.clone() + reciprocal.clone()).unwrap(),
            doubled.clone(),
            reciprocal.clone().min(doubled.clone()),
            reciprocal.clone().max(doubled),
            identity_power,
            (reciprocal / Function::one()).unwrap(),
        ];

        for function in non_contracting {
            assert!(function.powi(-1).evaluate_bound(&bounds, atol).is_ok());
        }
    }

    #[test]
    fn shrinking_product_does_not_reuse_nonzero_operand_proofs() {
        let x = Function::from(linear!(1));
        let y = Function::from(linear!(2));
        let bounds = Bounds::from([
            (VariableID::from(1), Bound::new(1.0, f64::INFINITY).unwrap()),
            (VariableID::from(2), Bound::new(1.0, f64::INFINITY).unwrap()),
        ]);
        let atol = ATol::new(f64::from_bits(1)).unwrap();
        let reciprocal_x = (Function::one() / x).unwrap();
        let reciprocal_y = (Function::one() / y).unwrap();
        let inverse_product = (reciprocal_x * reciprocal_y).unwrap().powi(-1);

        assert!(inverse_product.evaluate_bound(&bounds, atol).is_err());
    }

    #[test]
    fn polynomial_bound_reports_numeric_overflow_instead_of_panicking() {
        let function = Function::from((coeff!(f64::MAX) * linear!(1)).unwrap());
        let bounds = Bounds::from([(VariableID::from(1), Bound::new(2.0, 2.0).unwrap())]);

        assert!(function.evaluate_bound(&bounds, ATol::default()).is_err());
    }

    #[test]
    fn mixed_finite_intervals_reject_non_finite_endpoint_results() {
        let atol = ATol::default();
        let addition =
            (Function::from(linear!(1)) + Function::try_from(f64::MAX).unwrap()).unwrap();
        let multiplication = (Function::from(linear!(1)) * Function::from(linear!(2))).unwrap();
        let division = (Function::from(linear!(1)) / Function::try_from(0.5).unwrap()).unwrap();
        let power = Function::from(linear!(1)).powi(2);

        let cases = [
            (
                addition,
                Bounds::from([(VariableID::from(1), Bound::new(0.0, f64::MAX).unwrap())]),
                crate::v1::State::from_iter([(1, f64::MAX)]),
            ),
            (
                multiplication,
                Bounds::from([
                    (VariableID::from(1), Bound::new(1.0, 2.0).unwrap()),
                    (VariableID::from(2), Bound::new(f64::MAX, f64::MAX).unwrap()),
                ]),
                crate::v1::State::from_iter([(1, 2.0), (2, f64::MAX)]),
            ),
            (
                division,
                Bounds::from([(
                    VariableID::from(1),
                    Bound::new(f64::MAX / 2.0, f64::MAX).unwrap(),
                )]),
                crate::v1::State::from_iter([(1, f64::MAX)]),
            ),
            (
                power,
                Bounds::from([(VariableID::from(1), Bound::new(1.0, f64::MAX).unwrap())]),
                crate::v1::State::from_iter([(1, f64::MAX)]),
            ),
        ];

        for (function, bounds, state) in cases {
            let evaluation_error = function.evaluate(&state, atol).unwrap_err();
            assert!(matches!(
                evaluation_error.downcast_ref::<FunctionEvaluationError>(),
                Some(FunctionEvaluationError::NonFiniteResult { .. })
            ));

            let bound_error = function.evaluate_bound(&bounds, atol).unwrap_err();
            assert!(matches!(
                bound_error.downcast_ref::<FunctionEvaluationError>(),
                Some(FunctionEvaluationError::NonFiniteResult { .. })
            ));
        }
    }

    #[test]
    fn infinite_bound_endpoints_remain_unbounded_sentinels() {
        let x = Function::from(linear!(1));
        let functions = [
            (x.clone() + Function::one()).unwrap(),
            (x.clone() * Function::try_from(2.0).unwrap()).unwrap(),
            (x.clone() / Function::try_from(2.0).unwrap()).unwrap(),
            x.powi(2),
        ];

        for function in functions {
            assert!(function
                .evaluate_bound(&Bounds::new(), ATol::default())
                .is_ok());
        }
    }

    #[test]
    fn multiplication_bound_covers_an_underflowed_fma_residual() {
        let lhs = f64::MIN_POSITIVE.next_up();
        let rhs = 1.0_f64.next_up();
        let rounded = lhs * rhs;
        assert_eq!(lhs.mul_add(rhs, -rounded), 0.0);

        let function = Function::from(linear!(1) * linear!(2));
        let bounds = Bounds::from([
            (VariableID::from(1), Bound::new(lhs, lhs).unwrap()),
            (VariableID::from(2), Bound::new(rhs, rhs).unwrap()),
        ]);
        let bound = function.evaluate_bound(&bounds, ATol::default()).unwrap();

        assert!(bound.lower() <= rounded);
        assert!(bound.upper() >= rounded.next_up());
    }

    #[test]
    fn deep_expression_bound_evaluation_is_iterative() {
        let id = VariableID::from(1);
        let mut function = Function::from(linear!(id));
        for _ in 0..4096 {
            function = function.powi(2);
        }
        let bounds = Bounds::from([(id, Bound::new(-1.0, 1.0).unwrap())]);

        let bound = function.evaluate_bound(&bounds, ATol::default()).unwrap();
        assert_contains(bound, 0.0);
        assert_contains(bound, 1.0);
    }
}
