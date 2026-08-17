use super::*;
use crate::{Bound, Bounds};
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

fn outward_bound(lower: f64, upper: f64) -> crate::Result<Bound> {
    Ok(Bound::new(round_down(lower), round_up(upper))?)
}

fn add_enclosure(lhs: f64, rhs: f64) -> (f64, f64) {
    let rounded = lhs + rhs;
    if !rounded.is_finite() {
        return (rounded, rounded);
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
        (rounded.next_down(), rounded)
    } else if error > 0.0 {
        (rounded, rounded.next_up())
    } else {
        (rounded, rounded)
    }
}

fn mul_enclosure(lhs: f64, rhs: f64) -> (f64, f64) {
    let rounded = lhs * rhs;
    if !rounded.is_finite() {
        return (rounded, rounded);
    }
    if rounded == 0.0 {
        return if lhs == 0.0 || rhs == 0.0 {
            (rounded, rounded)
        } else {
            // A non-zero exact product underflowed below the smallest
            // subnormal. Preserve both possible directions around zero.
            (rounded.next_down(), rounded.next_up())
        };
    }
    if rounded.is_subnormal() {
        // The fused residual can itself underflow in the subnormal range.
        return (rounded.next_down(), rounded.next_up());
    }

    // The fused multiply-add recovers the exact rounding residual when the
    // rounded product is normal.
    let error = lhs.mul_add(rhs, -rounded);
    if error < 0.0 {
        (rounded.next_down(), rounded)
    } else if error > 0.0 {
        (rounded, rounded.next_up())
    } else if rounded.abs() < f64::from_bits(54_u64 << 52) {
        // A binary64 product has at most 106 significant bits. Below 2^-969,
        // its exact residual can fall below the smallest subnormal even when
        // the rounded product is normal, so an FMA result of zero does not
        // prove exactness.
        (rounded.next_down(), rounded.next_up())
    } else {
        (rounded, rounded)
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
    let (lower, _) = add_enclosure(lhs.lower(), rhs.lower());
    let (_, upper) = add_enclosure(lhs.upper(), rhs.upper());
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
        mul_enclosure(lhs.lower(), rhs.lower()),
        mul_enclosure(lhs.lower(), rhs.upper()),
        mul_enclosure(lhs.upper(), rhs.lower()),
        mul_enclosure(lhs.upper(), rhs.upper()),
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

fn div_enclosure(lhs: f64, rhs: f64) -> (f64, f64) {
    let rounded = lhs / rhs;
    if rounded.is_nan() || rounded.is_infinite() {
        return (rounded, rounded);
    }
    (round_down(rounded), round_up(rounded))
}

fn div_bounds(lhs: Bound, rhs: Bound) -> crate::Result<Bound> {
    let quotients = [
        div_enclosure(lhs.lower(), rhs.lower()),
        div_enclosure(lhs.lower(), rhs.upper()),
        div_enclosure(lhs.upper(), rhs.lower()),
        div_enclosure(lhs.upper(), rhs.upper()),
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
    outward_bound(1.0 / bound.upper(), 1.0 / bound.lower())
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
        anyhow::bail!("cannot bound a negative power when the base interval contains zero")
    }

    let scalar_endpoints = [base.lower().powi(exponent), base.upper().powi(exponent)];
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

impl Function {
    /// Conservatively determine whether floating-point evaluation can produce
    /// zero for some state in `bounds`.
    ///
    /// This is stricter than asking whether the interval enclosure contains
    /// zero. For example, `1 / x` over `x in [1, +inf)` approaches zero, so its
    /// closed enclosure starts at zero, but no finite binary64 input in that
    /// range makes the quotient zero. Preserving that distinction prevents a
    /// surrounding division from reporting a false domain error.
    fn may_evaluate_to_zero(&self, bounds: &Bounds) -> crate::Result<bool> {
        match self {
            Function::Zero => Ok(true),
            Function::Constant(value) => Ok(value.into_inner() == 0.0),
            Function::Unary(operation) => match operation.operator() {
                UnaryOperator::Powi(0) => {
                    // The operand is still evaluated by the strict expression
                    // semantics, but every defined result is one.
                    operation.operand().evaluate_bound(bounds)?;
                    Ok(false)
                }
                UnaryOperator::Powi(exponent) => {
                    let operand = operation.operand().evaluate_bound(bounds)?;
                    let operand_may_be_zero = operation.operand().may_evaluate_to_zero(bounds)?;
                    if exponent < 0 && operand_may_be_zero {
                        anyhow::bail!(
                            "cannot bound a negative power when the base interval contains zero"
                        )
                    }
                    if operand_may_be_zero {
                        return Ok(true);
                    }
                    let value = if exponent < 0 {
                        maximum_finite_absolute_value(operand)
                    } else {
                        minimum_absolute_value(operand)
                    };
                    Ok(value.powi(exponent) == 0.0)
                }
                UnaryOperator::Neg | UnaryOperator::Abs | UnaryOperator::Signum => {
                    operation.operand().may_evaluate_to_zero(bounds)
                }
            },
            Function::Binary(operation) if operation.operator() == BinaryOperator::Div => {
                let lhs = operation.lhs().evaluate_bound(bounds)?;
                let rhs = operation.rhs().evaluate_bound(bounds)?;
                if operation.rhs().may_evaluate_to_zero(bounds)? {
                    anyhow::bail!("cannot bound division when the denominator contains zero")
                }
                if operation.lhs().may_evaluate_to_zero(bounds)? {
                    return Ok(true);
                }

                // Division of nonzero operands can still round to zero. Test
                // the smallest possible numerator against the largest finite
                // denominator admitted by an unbounded endpoint.
                Ok(minimum_absolute_value(lhs) / maximum_finite_absolute_value(rhs) == 0.0)
            }
            Function::Linear(_)
            | Function::Quadratic(_)
            | Function::Polynomial(_)
            | Function::Nary(_)
            | Function::Binary(_) => Ok(bound_contains_zero(self.evaluate_bound(bounds)?)),
        }
    }

    #[cfg_attr(doc, katexit::katexit)]
    /// Compute an interval bound of this function given variable bounds.
    ///
    /// Missing IDs in `bounds` are treated as `Bound::default()` (unbounded).
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
    /// Returns an error when an interval crosses an undefined division or
    /// negative-integer-power domain, or when finite interval endpoints cannot
    /// be represented.
    pub fn evaluate_bound(&self, bounds: &Bounds) -> crate::Result<Bound> {
        if let Function::Unary(operation) = self {
            let operand = operation.operand().evaluate_bound(bounds)?;
            return match operation.operator() {
                UnaryOperator::Neg => Ok(Bound::new(-operand.upper(), -operand.lower())?),
                UnaryOperator::Abs if operand.lower() >= 0.0 => Ok(operand),
                UnaryOperator::Abs if operand.upper() <= 0.0 => {
                    Ok(Bound::new(-operand.upper(), -operand.lower())?)
                }
                UnaryOperator::Abs => Ok(Bound::new(
                    0.0,
                    operand.lower().abs().max(operand.upper().abs()),
                )?),
                UnaryOperator::Signum => {
                    let sign = |value: f64| {
                        if value < 0.0 {
                            -1.0
                        } else if value > 0.0 {
                            1.0
                        } else {
                            0.0
                        }
                    };
                    Ok(Bound::new(sign(operand.lower()), sign(operand.upper()))?)
                }
                UnaryOperator::Powi(exponent) => integer_power_bound(
                    operand,
                    exponent,
                    operation.operand().may_evaluate_to_zero(bounds)?,
                ),
            };
        }
        if let Function::Nary(operation) = self {
            let mut operands = operation
                .operands()
                .iter()
                .map(|operand| operand.evaluate_bound(bounds));
            let first = operands
                .next()
                .expect("nary operation always has at least two operands")?;
            return operands.try_fold(first, |acc, operand| {
                let operand = operand?;
                match operation.operator() {
                    NaryOperator::Add => add_bounds(acc, operand),
                    NaryOperator::Mul => mul_bounds(acc, operand),
                    NaryOperator::Min => Ok(Bound::new(
                        acc.lower().min(operand.lower()),
                        acc.upper().min(operand.upper()),
                    )?),
                    NaryOperator::Max => Ok(Bound::new(
                        acc.lower().max(operand.lower()),
                        acc.upper().max(operand.upper()),
                    )?),
                }
            });
        }
        if let Function::Binary(operation) = self {
            let lhs = operation.lhs().evaluate_bound(bounds)?;
            let rhs = operation.rhs().evaluate_bound(bounds)?;
            return match operation.operator() {
                BinaryOperator::Div => {
                    if operation.rhs().may_evaluate_to_zero(bounds)? {
                        anyhow::bail!("cannot bound division when the denominator contains zero")
                    }
                    div_bounds(lhs, rhs)
                }
            };
        }

        let mut bound = Bound::zero();
        for (ids, coefficient) in self
            .iter()
            .expect("all expression variants were handled above")
        {
            let value = coefficient.into_inner();
            if ids.is_empty() {
                bound = add_bounds(bound, Bound::new(value, value)?)?;
                continue;
            }
            let mut cur = Bound::new(1.0, 1.0)?;
            // Match PolynomialBase::evaluate's left-to-right repeated-ID fold.
            // The per-variable power still preserves correlation (for example,
            // an even power across zero has lower bound zero), unlike naively
            // multiplying the same interval by itself.
            for (id, exponent) in ids.chunks() {
                let b = bounds.get(&id).cloned().unwrap_or_default();
                let exponent = u64::try_from(exponent).map_err(|_| {
                    anyhow::anyhow!("monomial exponent is outside the supported integer range")
                })?;
                cur = mul_bounds(cur, left_fold_positive_integer_power_bound(b, exponent)?)?;
            }
            let coefficient = Bound::new(value, value)?;
            bound = add_bounds(bound, mul_bounds(coefficient, cur)?)?;
        }
        Ok(bound)
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
            f.evaluate_bound(&Bounds::new()).unwrap(),
            Bound::new(3.5, 3.5).unwrap()
        );
    }

    #[test]
    fn bound_of_linear() {
        // f = 2*x1 + 3 with x1 in [0, 2]
        let f = Function::from(((coeff!(2.0) * linear!(1)).unwrap() + coeff!(3.0)).unwrap());
        let mut bounds = Bounds::new();
        bounds.insert(VariableID::from(1), Bound::new(0.0, 2.0).unwrap());
        let bound = f.evaluate_bound(&bounds).unwrap();
        assert_contains(bound, 3.0);
        assert_contains(bound, 7.0);
    }

    #[test]
    fn bound_of_quadratic_with_squared_term() {
        // f = x1*x1 with x1 in [-2, 3].
        //
        // `Function::evaluate_bound` collapses the monomial via
        // `MonomialDyn::chunks()` and bounds its integer power. For an even
        // exponent across zero this yields [0, max(|-2|^2, 3^2)] = [0, 9], not
        // a naive interval-square [-6, 9].
        let f = Function::from(quadratic!(1, 1));
        let mut bounds = Bounds::new();
        bounds.insert(VariableID::from(1), Bound::new(-2.0, 3.0).unwrap());
        let bound = f.evaluate_bound(&bounds).unwrap();
        assert_contains(bound, 0.0);
        assert_contains(bound, 9.0);
    }

    #[test]
    fn bound_missing_id_is_unbounded() {
        let f = Function::from(linear!(1));
        assert_eq!(f.evaluate_bound(&Bounds::new()).unwrap(), Bound::default());
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
            square.evaluate_bound(&Bounds::new()).unwrap(),
            Bound::positive()
        );
        assert_eq!(
            cube.evaluate_bound(&Bounds::new()).unwrap(),
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
        let positive_bound = inverse_square.evaluate_bound(&positive_half_line).unwrap();
        assert_contains(positive_bound, 0.0);
        assert_contains(positive_bound, 1.0);

        let negative_half_line = Bounds::from([(id, Bound::new(f64::NEG_INFINITY, -1.0).unwrap())]);
        let negative_bound = inverse_cube.evaluate_bound(&negative_half_line).unwrap();
        assert_contains(negative_bound, -1.0);
        assert_contains(negative_bound, 0.0);

        assert!(inverse_square.evaluate_bound(&Bounds::new()).is_err());
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
        let bound = f.evaluate_bound(&bounds).unwrap();
        assert_contains(bound, -1.0);
        assert_contains(bound, 1.0);
    }

    #[test]
    fn bound_of_composite_operations() {
        let x = Function::from(linear!(1));
        let mut bounds = Bounds::new();
        bounds.insert(VariableID::from(1), Bound::new(-2.0, 3.0).unwrap());

        let absolute = x.clone().abs().evaluate_bound(&bounds).unwrap();
        assert_contains(absolute, 0.0);
        assert_contains(absolute, 3.0);

        let minimum = x
            .clone()
            .min(Function::try_from(1.0).unwrap())
            .evaluate_bound(&bounds)
            .unwrap();
        assert_contains(minimum, -2.0);
        assert_contains(minimum, 1.0);

        let square = x.powi(2).evaluate_bound(&bounds).unwrap();
        assert_contains(square, 0.0);
        assert_contains(square, 9.0);
    }

    #[test]
    fn bound_rejects_intervals_that_cross_an_undefined_domain() {
        let x = Function::from(linear!(1));
        let reciprocal = (Function::one() / x.clone()).unwrap();
        let mut bounds = Bounds::new();
        bounds.insert(VariableID::from(1), Bound::new(-1.0, 1.0).unwrap());
        assert!(reciprocal.evaluate_bound(&bounds).is_err());

        let inverse = x.powi(-1);
        assert!(inverse.evaluate_bound(&bounds).is_err());
    }

    #[test]
    fn polynomial_bound_preserves_exponents_larger_than_u8() {
        let id = VariableID::from(1);
        let monomial = MonomialDyn::new(vec![id; 256]);
        let function = Function::Polynomial(Polynomial::single_term(monomial, coeff!(1.0)));
        let bounds = Bounds::from([(id, Bound::new(2.0, 2.0).unwrap())]);
        let expected = 2.0_f64.powi(256);

        assert_contains(function.evaluate_bound(&bounds).unwrap(), expected);
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

        let value = function.evaluate(&state, ATol::default()).unwrap();
        assert_contains(function.evaluate_bound(&bounds).unwrap(), value);
    }

    #[test]
    fn polynomial_power_bound_contains_left_to_right_scalar_evaluation() {
        let id = VariableID::from(1);
        for (value, exponent) in [(0.1, 4), (2.1, 923), (0.906_234_158_714_408_3, 10_000)] {
            let monomial = MonomialDyn::new(vec![id; exponent]);
            let function = Function::Polynomial(Polynomial::single_term(monomial, coeff!(1.0)));
            let state = crate::v1::State::from_iter([(1, value)]);
            let bounds = Bounds::from([(id, Bound::new(value, value).unwrap())]);

            let evaluated = function.evaluate(&state, ATol::default()).unwrap();
            assert_contains(function.evaluate_bound(&bounds).unwrap(), evaluated);
        }
    }

    #[test]
    fn integer_power_bound_contains_powi_scalar_evaluation() {
        let id = VariableID::from(1);
        let function = Function::from(linear!(id)).powi(4);
        let state = crate::v1::State::from_iter([(1, 0.1)]);
        let bounds = Bounds::from([(id, Bound::new(0.1, 0.1).unwrap())]);

        let evaluated = function.evaluate(&state, ATol::default()).unwrap();
        assert_contains(function.evaluate_bound(&bounds).unwrap(), evaluated);
    }

    #[test]
    fn integer_power_handles_i32_min_without_overflowing_the_magnitude() {
        let id = VariableID::from(1);
        let function = Function::from(linear!(id)).powi(i32::MIN);
        let state = crate::v1::State::from_iter([(1, -1.0)]);
        let bounds = Bounds::from([(id, Bound::new(-1.0, -1.0).unwrap())]);

        let evaluated = function.evaluate(&state, ATol::default()).unwrap();
        assert_eq!(evaluated, 1.0);
        assert_contains(function.evaluate_bound(&bounds).unwrap(), evaluated);
    }

    #[test]
    fn negative_integer_power_inverts_before_exponentiation() {
        let id = VariableID::from(1);
        for (base, exponent) in [(1e200, -2), (2.0, -1024)] {
            let function = Function::from(linear!(id)).powi(exponent);
            let state = crate::v1::State::from_iter([(1, base)]);
            let bounds = Bounds::from([(id, Bound::new(base, base).unwrap())]);
            let evaluated = function.evaluate(&state, ATol::default()).unwrap();

            assert_contains(function.evaluate_bound(&bounds).unwrap(), evaluated);
        }
    }

    #[test]
    fn division_avoids_an_overflowing_reciprocal_intermediate() {
        let smallest = f64::from_bits(1);
        let lhs = Function::from(linear!(1));
        let rhs = Function::from(linear!(2));
        let function = (lhs / rhs).unwrap();
        let state = crate::v1::State::from_iter([(1, smallest), (2, smallest)]);
        let bounds = Bounds::from([
            (VariableID::from(1), Bound::new(smallest, smallest).unwrap()),
            (VariableID::from(2), Bound::new(smallest, smallest).unwrap()),
        ]);
        let evaluated = function.evaluate(&state, ATol::default()).unwrap();

        assert_eq!(evaluated, 1.0);
        assert_contains(function.evaluate_bound(&bounds).unwrap(), evaluated);
    }

    #[test]
    fn nested_operations_do_not_invent_a_zero_domain() {
        let id = VariableID::from(1);
        let x = Function::from(linear!(id));
        let bounds = Bounds::from([(id, Bound::new(1.0, f64::INFINITY).unwrap())]);

        let reciprocal = (Function::one() / x.clone()).unwrap();
        let inverse_reciprocal = reciprocal.clone().powi(-1);
        assert!(inverse_reciprocal.evaluate_bound(&bounds).is_ok());

        let zero_power = reciprocal.clone().powi(0);
        assert_eq!(
            zero_power.evaluate_bound(&bounds).unwrap(),
            Bound::new(1.0, 1.0).unwrap()
        );

        let reciprocal_of_reciprocal = (Function::one() / reciprocal).unwrap();
        assert!(reciprocal_of_reciprocal.evaluate_bound(&bounds).is_ok());

        let zero_to_one = Bounds::from([(id, Bound::new(0.0, 1.0).unwrap())]);
        let undefined_reciprocal = x.powi(-1);
        assert!(undefined_reciprocal.evaluate_bound(&zero_to_one).is_err());
    }

    #[test]
    fn polynomial_bound_reports_numeric_overflow_instead_of_panicking() {
        let function = Function::from((coeff!(f64::MAX) * linear!(1)).unwrap());
        let bounds = Bounds::from([(VariableID::from(1), Bound::new(2.0, 2.0).unwrap())]);

        assert!(function.evaluate_bound(&bounds).is_err());
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
        let bound = function.evaluate_bound(&bounds).unwrap();

        assert!(bound.lower() <= rounded);
        assert!(bound.upper() >= rounded.next_up());
    }
}
