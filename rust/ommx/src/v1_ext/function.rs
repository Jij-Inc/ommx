use crate::{
    macros::*,
    v1::{
        function::{self, Function as FunctionEnum},
        Function, Linear, Polynomial, Quadratic, SampledValues, Samples, State,
    },
    Bound, Bounds, Evaluate, MonomialDyn, VariableID, VariableIDSet,
};
use anyhow::{Context, Result};
use approx::AbsDiffEq;
use num::{
    integer::{gcd, lcm},
    Zero,
};
use std::{collections::HashMap, fmt, iter::*, ops::*};

impl Zero for Function {
    fn zero() -> Self {
        Self {
            function: Some(function::Function::Constant(0.0)),
        }
    }

    fn is_zero(&self) -> bool {
        match &self.function {
            Some(FunctionEnum::Constant(c)) => c.is_zero(),
            Some(FunctionEnum::Linear(linear)) => linear.is_zero(),
            Some(FunctionEnum::Quadratic(quadratic)) => quadratic.is_zero(),
            Some(FunctionEnum::Polynomial(poly)) => poly.is_zero(),
            _ => false,
        }
    }
}

impl From<function::Function> for Function {
    fn from(f: function::Function) -> Self {
        Self { function: Some(f) }
    }
}

impl From<Linear> for Function {
    fn from(linear: Linear) -> Self {
        Self {
            function: Some(function::Function::Linear(linear)),
        }
    }
}

impl From<Quadratic> for Function {
    fn from(q: Quadratic) -> Self {
        Self {
            function: Some(function::Function::Quadratic(q)),
        }
    }
}

impl From<Polynomial> for Function {
    fn from(poly: Polynomial) -> Self {
        Self {
            function: Some(function::Function::Polynomial(poly)),
        }
    }
}

impl From<f64> for Function {
    fn from(f: f64) -> Self {
        Self {
            function: Some(function::Function::Constant(f)),
        }
    }
}

impl FromIterator<(u64, f64)> for Function {
    fn from_iter<I: IntoIterator<Item = (u64, f64)>>(iter: I) -> Self {
        let linear: Linear = iter.into_iter().collect();
        linear.into()
    }
}

impl FromIterator<((u64, u64), f64)> for Function {
    fn from_iter<I: IntoIterator<Item = ((u64, u64), f64)>>(iter: I) -> Self {
        let quad: Quadratic = iter.into_iter().collect();
        quad.into()
    }
}

impl FromIterator<(MonomialDyn, f64)> for Function {
    fn from_iter<I: IntoIterator<Item = (MonomialDyn, f64)>>(iter: I) -> Self {
        let poly: Polynomial = iter.into_iter().collect();
        poly.into()
    }
}

impl<'a> IntoIterator for &'a Function {
    type Item = (MonomialDyn, f64);
    type IntoIter = Box<dyn Iterator<Item = Self::Item> + 'a>;

    fn into_iter(self) -> Self::IntoIter {
        match &self.function {
            Some(FunctionEnum::Constant(c)) => {
                Box::new(std::iter::once((MonomialDyn::empty(), *c)))
            }
            Some(FunctionEnum::Linear(linear)) => Box::new(
                linear
                    .into_iter()
                    .map(|(id, c)| (id.map(VariableID::from).into(), c)),
            ),
            Some(FunctionEnum::Quadratic(quad)) => Box::new(quad.into_iter()),
            Some(FunctionEnum::Polynomial(poly)) => Box::new(poly.into_iter()),
            None => Box::new(std::iter::empty()),
        }
    }
}

impl Function {
    pub fn degree(&self) -> u32 {
        match &self.function {
            Some(FunctionEnum::Constant(_)) => 0,
            Some(FunctionEnum::Linear(linear)) => linear.degree(),
            Some(FunctionEnum::Quadratic(quad)) => quad.degree(),
            Some(FunctionEnum::Polynomial(poly)) => poly.degree(),
            None => 0,
        }
    }

    pub fn as_linear(self) -> Option<Linear> {
        match self.function? {
            FunctionEnum::Constant(c) => Some(Linear::from(c)),
            FunctionEnum::Linear(linear) => Some(linear),
            FunctionEnum::Quadratic(quadratic) => quadratic.as_linear(),
            FunctionEnum::Polynomial(poly) => poly.as_linear(),
        }
    }

    pub fn as_constant(self) -> Option<f64> {
        match self.function? {
            FunctionEnum::Constant(c) => Some(c),
            FunctionEnum::Linear(linear) => linear.as_constant(),
            FunctionEnum::Quadratic(quadratic) => quadratic.as_constant(),
            FunctionEnum::Polynomial(poly) => poly.as_constant(),
        }
    }

    /// Get 0-th order term.
    pub fn get_constant(&self) -> f64 {
        match &self.function {
            Some(FunctionEnum::Constant(c)) => *c,
            Some(FunctionEnum::Linear(linear)) => linear.constant,
            Some(FunctionEnum::Quadratic(quad)) => quad.get_constant(),
            Some(FunctionEnum::Polynomial(poly)) => poly.get_constant(),
            None => 0.0,
        }
    }

    /// Substitute decision variable with a function
    ///
    /// For example, `x = f(y, z, ...)` into `g(x, y, z, ...)` yielding `g(f(y, z), y, z, ...)`.
    ///
    pub fn substitute(&self, replacements: &HashMap<u64, Self>) -> Result<Self> {
        if replacements.is_empty() {
            return Ok(self.clone());
        }
        let mut out = Function::zero();
        for (ids, coefficient) in self {
            let mut v = Function::from(coefficient);
            for id in ids.iter() {
                if let Some(replacement) = replacements.get(id) {
                    v = v * replacement.clone();
                } else {
                    v = v * Linear::single_term(id.into_inner(), 1.0);
                }
            }
            out = out + v;
        }
        Ok(out)
    }

    pub fn evaluate_bound(&self, bounds: &Bounds) -> Bound {
        let mut bound = Bound::zero();
        for (ids, value) in self.into_iter() {
            if value.is_zero() {
                continue;
            }
            if ids.is_empty() {
                bound += value;
                continue;
            }
            let mut cur = Bound::new(1.0, 1.0).unwrap();
            for (id, exp) in ids.chunks() {
                let b = bounds.get(&id).cloned().unwrap_or_default();
                cur *= b.pow(exp as u8);
                if cur == Bound::default() {
                    return Bound::default();
                }
            }
            bound += value * cur;
        }
        bound
    }

    /// Get a minimal positive factor `a` which make all coefficients of `a * self` integer.
    ///
    /// This returns `1` for zero function. See also <https://en.wikipedia.org/wiki/Primitive_part_and_content>.
    pub fn content_factor(&self) -> Result<f64> {
        let mut numer_gcd = 0;
        let mut denom_lcm: i64 = 1;
        for (_, coefficient) in self {
            let r = num::Rational64::approximate_float(coefficient)
                .context("Cannot approximate coefficient in 64-bit rational")?;
            numer_gcd = gcd(numer_gcd, *r.numer());
            denom_lcm
                .checked_mul(*r.denom())
                .context("Overflow detected while evaluating minimal integer coefficient multiplier. This means it is hard to make the all coefficient integer")?;
            denom_lcm = lcm(denom_lcm, *r.denom());
        }

        if numer_gcd == 0 {
            Ok(1.0)
        } else {
            Ok((denom_lcm as f64 / numer_gcd as f64).abs())
        }
    }
}

impl Add for Function {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        let lhs = self.function.expect("Empty Function");
        let rhs = rhs.function.expect("Empty Function");
        match (lhs, rhs) {
            (FunctionEnum::Constant(lhs), FunctionEnum::Constant(rhs)) => Function::from(lhs + rhs),
            // Linear output
            (FunctionEnum::Linear(lhs), FunctionEnum::Constant(rhs))
            | (FunctionEnum::Constant(rhs), FunctionEnum::Linear(lhs)) => Function::from(lhs + rhs),
            (FunctionEnum::Linear(lhs), FunctionEnum::Linear(rhs)) => Function::from(lhs + rhs),
            // Quadratic output
            (FunctionEnum::Quadratic(lhs), FunctionEnum::Constant(rhs))
            | (FunctionEnum::Constant(rhs), FunctionEnum::Quadratic(lhs)) => {
                Function::from(lhs + rhs)
            }
            (FunctionEnum::Quadratic(lhs), FunctionEnum::Linear(rhs))
            | (FunctionEnum::Linear(rhs), FunctionEnum::Quadratic(lhs)) => {
                Function::from(lhs + rhs)
            }
            (FunctionEnum::Quadratic(lhs), FunctionEnum::Quadratic(rhs)) => {
                Function::from(lhs + rhs)
            }
            // Polynomial output
            (FunctionEnum::Polynomial(lhs), FunctionEnum::Constant(rhs))
            | (FunctionEnum::Constant(rhs), FunctionEnum::Polynomial(lhs)) => {
                Function::from(lhs + rhs)
            }
            (FunctionEnum::Polynomial(lhs), FunctionEnum::Linear(rhs))
            | (FunctionEnum::Linear(rhs), FunctionEnum::Polynomial(lhs)) => {
                Function::from(lhs + rhs)
            }
            (FunctionEnum::Polynomial(lhs), FunctionEnum::Quadratic(rhs))
            | (FunctionEnum::Quadratic(rhs), FunctionEnum::Polynomial(lhs)) => {
                Function::from(lhs + rhs)
            }
            (FunctionEnum::Polynomial(lhs), FunctionEnum::Polynomial(rhs)) => {
                Function::from(lhs + rhs)
            }
        }
    }
}

impl_add_from!(Function, f64);
impl_add_from!(Function, Linear);
impl_add_from!(Function, Quadratic);
impl_add_from!(Function, Polynomial);
impl_add_inverse!(f64, Function);
impl_add_inverse!(Linear, Function);
impl_add_inverse!(Quadratic, Function);
impl_add_inverse!(Polynomial, Function);
impl_sub_by_neg_add!(Function, Function);
impl_sub_by_neg_add!(Function, f64);
impl_sub_by_neg_add!(Function, Linear);
impl_sub_by_neg_add!(Function, Quadratic);
impl_sub_by_neg_add!(Function, Polynomial);

impl Mul for Function {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self {
        let lhs = self.function.expect("Empty Function");
        let rhs = rhs.function.expect("Empty Function");
        match (lhs, rhs) {
            (FunctionEnum::Constant(lhs), FunctionEnum::Constant(rhs)) => Function::from(lhs * rhs),
            (FunctionEnum::Linear(lhs), FunctionEnum::Constant(rhs))
            | (FunctionEnum::Constant(rhs), FunctionEnum::Linear(lhs)) => Function::from(lhs * rhs),
            (FunctionEnum::Linear(lhs), FunctionEnum::Linear(rhs)) => Function::from(lhs * rhs),
            (FunctionEnum::Quadratic(lhs), FunctionEnum::Constant(rhs))
            | (FunctionEnum::Constant(rhs), FunctionEnum::Quadratic(lhs)) => {
                Function::from(lhs * rhs)
            }
            (FunctionEnum::Quadratic(lhs), FunctionEnum::Linear(rhs))
            | (FunctionEnum::Linear(rhs), FunctionEnum::Quadratic(lhs)) => {
                Function::from(lhs * rhs)
            }
            (FunctionEnum::Quadratic(lhs), FunctionEnum::Quadratic(rhs)) => {
                Function::from(lhs * rhs)
            }
            (FunctionEnum::Polynomial(lhs), FunctionEnum::Constant(rhs))
            | (FunctionEnum::Constant(rhs), FunctionEnum::Polynomial(lhs)) => {
                Function::from(lhs * rhs)
            }
            (FunctionEnum::Polynomial(lhs), FunctionEnum::Linear(rhs))
            | (FunctionEnum::Linear(rhs), FunctionEnum::Polynomial(lhs)) => {
                Function::from(lhs * rhs)
            }
            (FunctionEnum::Polynomial(lhs), FunctionEnum::Quadratic(rhs))
            | (FunctionEnum::Quadratic(rhs), FunctionEnum::Polynomial(lhs)) => {
                Function::from(lhs * rhs)
            }
            (FunctionEnum::Polynomial(lhs), FunctionEnum::Polynomial(rhs)) => {
                Function::from(lhs * rhs)
            }
        }
    }
}

impl_neg_by_mul!(Function);
impl_mul_from!(Function, f64, Function);
impl_mul_from!(Function, Linear, Function);
impl_mul_from!(Function, Quadratic, Function);
impl_mul_from!(Function, Polynomial, Function);
impl_mul_inverse!(f64, Function);
impl_mul_inverse!(Linear, Function);
impl_mul_inverse!(Quadratic, Function);
impl_mul_inverse!(Polynomial, Function);

impl Sum for Function {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.fold(Function::from(0.0), |acc, x| acc + x)
    }
}

impl Product for Function {
    fn product<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.fold(Function::from(1.0), |acc, x| acc * x)
    }
}

impl AbsDiffEq for Function {
    type Epsilon = crate::ATol;

    fn default_epsilon() -> Self::Epsilon {
        crate::ATol::default()
    }

    fn abs_diff_eq(&self, other: &Self, epsilon: Self::Epsilon) -> bool {
        let lhs = self.function.as_ref().expect("Empty Function");
        let rhs = other.function.as_ref().expect("Empty Function");
        match (lhs, rhs) {
            // Same order
            (FunctionEnum::Constant(lhs), FunctionEnum::Constant(rhs)) => {
                lhs.abs_diff_eq(rhs, *epsilon)
            }
            (FunctionEnum::Linear(lhs), FunctionEnum::Linear(rhs)) => lhs.abs_diff_eq(rhs, epsilon),
            (FunctionEnum::Quadratic(lhs), FunctionEnum::Quadratic(rhs)) => {
                lhs.abs_diff_eq(rhs, epsilon)
            }
            (FunctionEnum::Polynomial(lhs), FunctionEnum::Polynomial(rhs)) => {
                lhs.abs_diff_eq(rhs, epsilon)
            }
            // Upcast to higher order
            (FunctionEnum::Constant(lhs), FunctionEnum::Linear(rhs))
            | (FunctionEnum::Linear(rhs), FunctionEnum::Constant(lhs)) => {
                let lhs = Linear::from(*lhs);
                lhs.abs_diff_eq(rhs, epsilon)
            }
            (FunctionEnum::Constant(lhs), FunctionEnum::Quadratic(rhs))
            | (FunctionEnum::Quadratic(rhs), FunctionEnum::Constant(lhs)) => {
                let lhs = Quadratic::from(*lhs);
                lhs.abs_diff_eq(rhs, epsilon)
            }
            (FunctionEnum::Constant(lhs), FunctionEnum::Polynomial(rhs))
            | (FunctionEnum::Polynomial(rhs), FunctionEnum::Constant(lhs)) => {
                let lhs = Polynomial::from(*lhs);
                lhs.abs_diff_eq(rhs, epsilon)
            }
            (FunctionEnum::Linear(lhs), FunctionEnum::Quadratic(rhs))
            | (FunctionEnum::Quadratic(rhs), FunctionEnum::Linear(lhs)) => {
                let lhs = Quadratic::from(lhs.clone());
                lhs.abs_diff_eq(rhs, epsilon)
            }
            (FunctionEnum::Linear(lhs), FunctionEnum::Polynomial(rhs))
            | (FunctionEnum::Polynomial(rhs), FunctionEnum::Linear(lhs)) => {
                let lhs = Polynomial::from(lhs.clone());
                lhs.abs_diff_eq(rhs, epsilon)
            }
            (FunctionEnum::Quadratic(lhs), FunctionEnum::Polynomial(rhs))
            | (FunctionEnum::Polynomial(rhs), FunctionEnum::Quadratic(lhs)) => {
                let lhs = Polynomial::from(lhs.clone());
                lhs.abs_diff_eq(rhs, epsilon)
            }
        }
    }
}

impl fmt::Display for Function {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.function {
            Some(FunctionEnum::Constant(c)) => write!(f, "{}", c),
            Some(FunctionEnum::Linear(linear)) => write!(f, "{}", linear),
            Some(FunctionEnum::Quadratic(quadratic)) => write!(f, "{}", quadratic),
            Some(FunctionEnum::Polynomial(poly)) => write!(f, "{}", poly),
            None => write!(f, "0"),
        }
    }
}

impl Evaluate for Function {
    type Output = f64;
    type SampledOutput = SampledValues;

    fn evaluate(&self, solution: &State, atol: crate::ATol) -> Result<f64> {
        let out = match &self.function {
            Some(FunctionEnum::Constant(c)) => *c,
            Some(FunctionEnum::Linear(linear)) => linear.evaluate(solution, atol)?,
            Some(FunctionEnum::Quadratic(quadratic)) => quadratic.evaluate(solution, atol)?,
            Some(FunctionEnum::Polynomial(poly)) => poly.evaluate(solution, atol)?,
            None => 0.0,
        };
        Ok(out)
    }

    fn partial_evaluate(&mut self, state: &State, atol: crate::ATol) -> Result<()> {
        match &mut self.function {
            Some(FunctionEnum::Linear(linear)) => linear.partial_evaluate(state, atol)?,
            Some(FunctionEnum::Quadratic(quadratic)) => quadratic.partial_evaluate(state, atol)?,
            Some(FunctionEnum::Polynomial(poly)) => poly.partial_evaluate(state, atol)?,
            _ => {}
        };
        Ok(())
    }

    fn evaluate_samples(&self, samples: &Samples, atol: crate::ATol) -> Result<Self::SampledOutput> {
        let out = samples.map(|s| {
            let value = self.evaluate(s, atol)?;
            Ok(value)
        })?;
        Ok(out)
    }

    fn required_ids(&self) -> VariableIDSet {
        match &self.function {
            Some(FunctionEnum::Linear(linear)) => linear.required_ids(),
            Some(FunctionEnum::Quadratic(quadratic)) => quadratic.required_ids(),
            Some(FunctionEnum::Polynomial(poly)) => poly.required_ids(),
            _ => VariableIDSet::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{random::*, Evaluate, VariableID};
    use maplit::*;

    test_algebraic!(Function);

    #[test]
    fn evaluate_bound_missing() {
        let f: Function = Linear::new([(1, 1.0), (2, 2.0)].into_iter(), 1.0).into();
        // Missing bounds of x1 and x2
        let bounds = Bounds::default();
        assert_eq!(f.evaluate_bound(&bounds), Bound::default());
    }

    #[test]
    fn evaluate_bound() {
        let x1 = Linear::single_term(1, 1.0);
        let x2 = Linear::single_term(2, 2.0);
        let f: Function = (x1.clone() + x2 + 1.0).into();
        let bounds = btreemap! {
            VariableID::from(1) => Bound::new(-1.0, 1.0).unwrap(),
            VariableID::from(2) => Bound::new(2.0, 3.0).unwrap(),
        };
        // [-1, 1] + 2*[2, 3] + 1 = [4, 8]
        insta::assert_debug_snapshot!(f.evaluate_bound(&bounds), @r###"
        Bound {
            lower: 4.0,
            upper: 8.0,
        }
        "###);

        let f: Function = (x1.clone() * x1).into();
        // [-1, 1]^2 = [0, 1]
        insta::assert_debug_snapshot!(f.evaluate_bound(&bounds), @r###"
        Bound {
            lower: 0.0,
            upper: 1.0,
        }
        "###);
        // (-inf, inf)^2 = [0, inf)
        insta::assert_debug_snapshot!(f.evaluate_bound(&Bounds::default()), @r###"
        Bound {
            lower: 0.0,
            upper: inf,
        }
        "###);
    }

    #[test]
    fn content_factor() {
        let x1 = Linear::single_term(1, 1.0);
        let x2 = Linear::single_term(2, 1.0);

        // x1 + x2
        // => 1
        let f: Function = (x1.clone() + x2.clone()).into();
        assert_eq!(f.content_factor().unwrap(), 1.0);

        // x1 / 2 + x2 / 3
        // => 6 / 1
        let f: Function = (0.5 * x1.clone() + (1.0 / 3.0) * x2.clone()).into();
        assert_eq!(f.content_factor().unwrap(), 6.0);

        // 2 * x1 / 3 + 2 * x2 / 5
        // => 15 / 2
        let f: Function = (2.0 / 3.0 * x1.clone() + 2.0 / 5.0 * x2.clone()).into();
        assert_eq!(f.content_factor().unwrap(), 15.0 / 2.0);

        // 3 * x1 / 4 + 3 * x2 / 8
        // => 8 / 3
        let f: Function = (3.0 / 4.0 * x1.clone() + 3.0 / 8.0 * x2.clone()).into();
        assert_eq!(f.content_factor().unwrap(), 8.0 / 3.0);

        use std::f64::consts::PI;
        let f: Function = (PI * x1 + 2.0 * PI * x2).into();
        assert_eq!(f.content_factor().unwrap(), 1.0 / PI,);
    }

    proptest! {
        #[test]
        fn test_as_linear_roundtrip(f in Function::arbitrary_with(FunctionParameters{ num_terms: 5, max_degree: 1, max_id: 10})) {
            let linear = f.clone().as_linear().unwrap();
            // `Function::Constant(c)` and `Function::Linear(Linear { terms: [], constant: c })` are mathematically same, but not structurally same.
            prop_assert!(f.abs_diff_eq(&Function::from(linear), crate::ATol::default()));
        }

        #[test]
        fn test_as_constant_roundtrip(f in Function::arbitrary_with(FunctionParameters{ num_terms: 1, max_degree: 0,  max_id: 10})) {
            let c = f.clone().as_constant().unwrap();
            prop_assert!(f.abs_diff_eq(&Function::from(c), crate::ATol::default()));
        }

        #[test]
        fn test_max_degree_0(f in Function::arbitrary_with(FunctionParameters{ num_terms: 1, max_degree: 0, max_id: 10})) {
            prop_assert!(f.degree() == 0);
        }

        #[test]
        fn test_max_degree_1(f in Function::arbitrary_with(FunctionParameters{ num_terms: 5, max_degree: 1, max_id: 10})) {
            prop_assert!(f.degree() <= 1);
        }

        #[test]
        fn test_max_degree_2(f in Function::arbitrary_with(FunctionParameters{ num_terms: 5, max_degree: 2, max_id: 10})) {
            prop_assert!(f.degree() <= 2);
        }

        #[test]
        fn test_as_linear_any(f in Function::arbitrary()) {
            prop_assert!((dbg!(f.degree()) >= 2) ^ dbg!(f.as_linear()).is_some());
        }

        #[test]
        fn test_as_const_any(f in Function::arbitrary()) {
            prop_assert!((dbg!(f.degree()) >= 1) ^ dbg!(f.as_constant()).is_some());
        }

        #[test]
        fn evaluate_bound_arb(
            (f, bounds, state) in Function::arbitrary()
                .prop_flat_map(|f| {
                    let bounds = arbitrary_bounds(f.required_ids().into_iter());
                    (Just(f), bounds)
                        .prop_flat_map(|(f, bounds)| {
                            let state = arbitrary_state_within_bounds(&bounds, 1e5);
                            (Just(f), Just(bounds), state)
                        })
                })
        ) {
            let bound = f.evaluate_bound(&bounds);
            let value = f.evaluate(&state, 1e-9).unwrap();
            prop_assert!(bound.contains(value, 1e-7));
        }

        #[test]
        fn content_factor_arb(f in Function::arbitrary()) {
            let Ok(multiplier) = f.content_factor() else { return Ok(()) };
            prop_assert!(multiplier > 0.0);
            let f = f * multiplier;
            for (_, c) in &f {
                if c.abs() > 1.0 {
                    prop_assert!((c - c.round()).abs() / c.abs() < 1e-10, "c = {c}");
                } else {
                    prop_assert!((c - c.round()).abs() < 1e-10, "c = {c}");
                }
            }
        }
    }
}
