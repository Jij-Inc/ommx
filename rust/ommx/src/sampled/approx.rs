use ::approx::AbsDiffEq;

use super::*;

fn compare<T>(a: &Sampled<T>, b: &Sampled<T>, mut f: impl FnMut(&T, &T) -> bool) -> bool {
    if a.offsets.len() != b.offsets.len() {
        return false;
    }
    for (id, offset) in a.offsets.iter() {
        debug_assert!(*offset < a.data.len());
        let Some(other_offset) = b.offsets.get(id) else {
            return false;
        };
        debug_assert!(*other_offset < b.data.len());
        if !f(&a.data[*offset], &b.data[*other_offset]) {
            return false;
        }
    }
    true
}

impl<T> PartialEq for Sampled<T>
where
    T: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        compare(self, other, |a, b| a == b)
    }
}

impl<T> AbsDiffEq for Sampled<T>
where
    T: AbsDiffEq,
    <T as AbsDiffEq>::Epsilon: Clone,
{
    type Epsilon = <T as AbsDiffEq>::Epsilon;

    fn default_epsilon() -> Self::Epsilon {
        T::default_epsilon()
    }

    fn abs_diff_eq(&self, other: &Self, epsilon: Self::Epsilon) -> bool {
        compare(self, other, |a, b| a.abs_diff_eq(b, epsilon.clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_partial_eq() {
        // Different as struct, but mathematical equal
        let a = Sampled::new(
            [[SampleID(1), SampleID(2)], [SampleID(5), SampleID(7)]],
            [1, 2],
        )
        .unwrap();
        let b = Sampled::new(
            [[SampleID(7), SampleID(5)], [SampleID(1), SampleID(2)]],
            [2, 1],
        )
        .unwrap();
        assert_eq!(a, b);
    }
}
