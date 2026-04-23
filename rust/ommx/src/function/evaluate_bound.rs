use super::*;
use crate::{Bound, Bounds};
use num::Zero;

impl Function {
    #[cfg_attr(doc, katexit::katexit)]
    /// Compute an interval bound of this function given variable bounds.
    ///
    /// Missing IDs in `bounds` are treated as `Bound::default()` (unbounded).
    ///
    /// # Tightness
    ///
    /// This evaluates the bound **term by term** (monomial-wise) and sums the
    /// per-term intervals. The result is a **sound over-approximation** of the
    /// true range $[\inf f, \sup f]$ but is **not guaranteed to be tight**,
    /// because it ignores dependencies between terms that share variables.
    ///
    /// For example, $f = x^2 - x$ with $x \in [0, 1]$ has true range
    /// $[-1/4, 0]$ (minimum at $x = 1/2$), but term-wise evaluation yields
    /// $[0, 1] + (-[0, 1]) = [-1, 1]$.
    pub fn evaluate_bound(&self, bounds: &Bounds) -> Bound {
        let mut bound = Bound::zero();
        for (ids, coefficient) in self.iter() {
            let value = coefficient.into_inner();
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{coeff, linear, quadratic, Bound, VariableID};

    #[test]
    fn bound_of_constant() {
        let f = Function::Constant(coeff!(3.5));
        assert_eq!(
            f.evaluate_bound(&Bounds::new()),
            Bound::new(3.5, 3.5).unwrap()
        );
    }

    #[test]
    fn bound_of_linear() {
        // f = 2*x1 + 3 with x1 in [0, 2]
        let f = Function::from(coeff!(2.0) * linear!(1) + coeff!(3.0));
        let mut bounds = Bounds::new();
        bounds.insert(VariableID::from(1), Bound::new(0.0, 2.0).unwrap());
        assert_eq!(f.evaluate_bound(&bounds), Bound::new(3.0, 7.0).unwrap());
    }

    #[test]
    fn bound_of_quadratic_with_squared_term() {
        // f = x1*x1 with x1 in [-2, 3].
        //
        // `Function::evaluate_bound` collapses the monomial via `MonomialDyn::chunks()`
        // into `Bound::pow(2)`. For an even exponent across zero, `Bound::pow` uses
        // sound interval-power semantics and yields [0, max(|-2|^2, 3^2)] = [0, 9],
        // not a naive interval-square [-6, 9].
        let f = Function::from(quadratic!(1, 1));
        let mut bounds = Bounds::new();
        bounds.insert(VariableID::from(1), Bound::new(-2.0, 3.0).unwrap());
        let expected = Bound::new(0.0, 9.0).unwrap();
        assert_eq!(f.evaluate_bound(&bounds), expected);
    }

    #[test]
    fn bound_missing_id_is_unbounded() {
        let f = Function::from(linear!(1));
        assert_eq!(f.evaluate_bound(&Bounds::new()), Bound::default());
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
        let f = Function::from(quadratic!(1, 1)) + Function::from(-coeff!(1.0) * linear!(1));
        let mut bounds = Bounds::new();
        bounds.insert(VariableID::from(1), Bound::new(0.0, 1.0).unwrap());
        assert_eq!(f.evaluate_bound(&bounds), Bound::new(-1.0, 1.0).unwrap());
    }
}
