use crate::{
    v1::{Samples, State},
    VariableIDSet,
};
use anyhow::Result;

/// Evaluate with a [State]
pub trait Evaluate {
    type Output;
    type SampledOutput;

    /// Evaluate to return the output with used variable ids
    fn evaluate(&self, state: &State, atol: crate::ATol) -> Result<Self::Output>;

    /// Evaluate for each sample
    fn evaluate_samples(&self, samples: &Samples, atol: crate::ATol) -> Result<Self::SampledOutput>;

    /// Partially evaluate the function to return the used variable ids
    fn partial_evaluate(&mut self, state: &State, atol: crate::ATol) -> Result<()>;

    /// Decision variable IDs required for evaluation
    fn required_ids(&self) -> VariableIDSet;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        random::*,
        v1::{Function, Instance, Linear, Polynomial, Quadratic},
    };
    use approx::*;
    use maplit::*;
    use proptest::prelude::*;

    #[test]
    fn linear_partial_evaluate() {
        let mut linear = Linear::new([(1, 1.0), (2, 2.0), (3, 3.0), (4, 4.0)].into_iter(), 5.0);
        let state = State {
            entries: hashmap! { 1 => 1.0, 2 => 2.0, 3 => 3.0, 5 => 5.0, 6 => 6.0 },
        };
        linear.partial_evaluate(&state, 1e-9).unwrap();
        assert_eq!(linear.constant, 5.0 + 1.0 * 1.0 + 2.0 * 2.0 + 3.0 * 3.0);
        assert_eq!(linear.terms.len(), 1);
        assert_eq!(linear.terms[0].id, 4);
        assert_eq!(linear.terms[0].coefficient, 4.0);
    }

    macro_rules! pair_with_state {
        ($t:ty) => {
            (<$t>::arbitrary(), <$t>::arbitrary()).prop_flat_map(|(f, g)| {
                let ids = f.required_ids().union(&g.required_ids()).cloned().collect();
                (Just(f), Just(g), arbitrary_state(ids))
            })
        };
    }

    /// f(x) + g(x) = (f + g)(x)
    macro_rules! evaluate_add_commutativity {
        ($t:ty, $name:ident) => {
            proptest! {
                #[test]
                fn $name((f, g, s) in pair_with_state!($t)) {
                    let f_value = f.evaluate(&s, 1e-9).unwrap();
                    let g_value = g.evaluate(&s, 1e-9).unwrap();
                    let h_value = (f + g).evaluate(&s, 1e-9).unwrap();
                    prop_assert!(abs_diff_eq!(dbg!(f_value + g_value), dbg!(h_value), epsilon = 1e-9));
                }
            }
        };
    }
    /// f(x) * g(x) = (f * g)(x)
    macro_rules! evaluate_mul_commutativity {
        ($t:ty, $name:ident) => {
            proptest! {
                #[test]
                fn $name((f, g, s) in pair_with_state!($t)) {
                    let f_value = f.evaluate(&s, 1e-9).unwrap();
                    let g_value = g.evaluate(&s, 1e-9).unwrap();
                    let h_value = (f * g).evaluate(&s, 1e-9).unwrap();
                    prop_assert!(abs_diff_eq!(dbg!(f_value * g_value), dbg!(h_value), epsilon = 1e-9));
                }
            }
        };
    }
    evaluate_add_commutativity!(Linear, linear_evaluate_add_commutativity);
    evaluate_mul_commutativity!(Linear, linear_evaluate_mul_commutativity);
    evaluate_add_commutativity!(Quadratic, quadratic_evaluate_add_commutativity);
    evaluate_mul_commutativity!(Quadratic, quadratic_evaluate_mul_commutativity);
    evaluate_add_commutativity!(Polynomial, polynomial_evaluate_add_commutativity);
    evaluate_mul_commutativity!(Polynomial, polynomial_evaluate_mul_commutativity);
    evaluate_add_commutativity!(Function, function_evaluate_add_commutativity);
    evaluate_mul_commutativity!(Function, function_evaluate_mul_commutativity);

    macro_rules! function_with_state {
        ($t:ty) => {
            <$t>::arbitrary().prop_flat_map(|f| {
                let ids = f.required_ids();
                (Just(f), arbitrary_state(ids))
            })
        };
    }

    macro_rules! partial_evaluate_to_constant {
        ($t:ty, $name:ident) => {
            proptest! {
                #[test]
                fn $name((mut f, s) in function_with_state!($t)) {
                    let v = f.evaluate(&s, 1e-9).unwrap();
                    f.partial_evaluate(&s, 1e-9).unwrap();
                    let c = dbg!(f).as_constant().expect("Non constant");
                    prop_assert!(abs_diff_eq!(v, c, epsilon = 1e-9));
                }
            }
        };
    }
    partial_evaluate_to_constant!(Linear, linear_partial_evaluate_to_constant);
    partial_evaluate_to_constant!(Quadratic, quadratic_partial_evaluate_to_constant);
    partial_evaluate_to_constant!(Polynomial, polynomial_partial_evaluate_to_constant);
    partial_evaluate_to_constant!(Function, function_partial_evaluate_to_constant);

    fn split_state(state: State) -> BoxedStrategy<(State, State)> {
        let ids: Vec<(u64, f64)> = state.entries.into_iter().collect();
        let flips = proptest::collection::vec(bool::arbitrary(), ids.len());
        (Just(ids), flips)
            .prop_map(|(ids, flips)| {
                let mut a = State::default();
                let mut b = State::default();
                for (flip, (id, value)) in flips.into_iter().zip(ids.into_iter()) {
                    if flip {
                        a.entries.insert(id, value);
                    } else {
                        b.entries.insert(id, value);
                    }
                }
                (a, b)
            })
            .boxed()
    }

    macro_rules! function_with_split_state {
        ($t:ty) => {
            <$t>::arbitrary().prop_flat_map(|f| {
                let ids = f.required_ids();
                (Just(f), arbitrary_state(ids))
                    .prop_flat_map(|(f, s)| (Just(f), Just(s.clone()), split_state(s)))
            })
        };
    }

    macro_rules! half_partial_evaluate {
        ($t:ty, $name:ident) => {
            proptest! {
                #[test]
                fn $name((mut f, s, (s1, s2)) in function_with_split_state!($t)) {
                    let v = f.evaluate(&s, 1e-9).unwrap();
                    f.partial_evaluate(&s1, 1e-9).unwrap();
                    let u = f.evaluate(&s2, 1e-9).unwrap();
                    prop_assert!(abs_diff_eq!(v, u, epsilon = 1e-9));
                }
            }
        };
    }
    half_partial_evaluate!(Linear, linear_half_partial_evaluate);
    half_partial_evaluate!(Quadratic, quadratic_half_partial_evaluate);
    half_partial_evaluate!(Polynomial, polynomial_half_partial_evaluate);
    half_partial_evaluate!(Function, function_half_partial_evaluate);

    fn instance_with_state() -> BoxedStrategy<(Instance, State)> {
        Instance::arbitrary()
            .prop_flat_map(|instance| {
                let bounds = instance.get_bounds().expect("Invalid Bound in Instance");
                let state = arbitrary_state_within_bounds(&bounds, 100.0);
                (Just(instance), state)
            })
            .boxed()
    }

    proptest! {
        #[test]
        fn evaluate_instance((instance, state) in instance_with_state()) {
            let solution = instance.evaluate(&state, 1e-9).unwrap();
            let mut cids = instance.constraint_ids();
            cids.extend(instance.removed_constraint_ids());
            prop_assert!(solution.constraint_ids() == cids);
        }
    }

    proptest! {
        #[test]
        fn partial_eval_instance(mut instance in Instance::arbitrary(), state in any::<State>()) {
            instance.partial_evaluate(&state, 1e-9).unwrap();
            for v in &instance.decision_variables {
                if let Some(value) = state.entries.get(&v.id) {
                    prop_assert_eq!(v.substituted_value, Some(*value));
                } else {
                    prop_assert_eq!(v.substituted_value, None);
                }
            }
        }
    }

    fn instance_with_split_state() -> BoxedStrategy<(Instance, State, (State, State))> {
        Instance::arbitrary()
            .prop_flat_map(|instance| {
                let bounds = instance.get_bounds().expect("Invalid Bound in Instance");
                let state = arbitrary_state_within_bounds(&bounds, 100.0);
                (Just(instance), state).prop_flat_map(|(instance, state)| {
                    (Just(instance), Just(state.clone()), split_state(state))
                })
            })
            .boxed()
    }

    proptest! {
        #[test]
        fn partial_eval_instance_to_solution((mut instance, state, (s1, s2)) in instance_with_split_state()) {
            let solution = instance.evaluate(&state, 1e-9).unwrap();
            instance.partial_evaluate(&s1, 1e-9).unwrap();
            let solution1 = instance.evaluate(&s2, 1e-9).unwrap();
            prop_assert_eq!(solution.decision_variable_ids(), solution1.decision_variable_ids());
            prop_assert_eq!(solution.constraint_ids(), solution1.constraint_ids());
            prop_assert_eq!(solution.state, solution1.state);
        }
    }

    proptest! {
        #[test]
        fn evaluate_samples((instance, state) in instance_with_state()) {
            let solution = instance.evaluate(&state, 1e-9).unwrap();

            let mut samples = Samples::default();
            samples.add_sample(0, state);
            let sample_set = instance.evaluate_samples(&samples, 1e-9).unwrap();

            prop_assert_eq!(solution, sample_set.get(0).unwrap());
        }
    }

    proptest! {
        #[test]
        fn substitute((f, mut g, mut s) in pair_with_state!(Function)) {
            // Determine ID to be substituted
            let ids = f.required_ids();
            let Some(id) = ids.iter().next().cloned() else { return Ok(()) };
            g.partial_evaluate(&State { entries: hashmap!{ id.into_inner() => 1.0 } }, 1e-9).unwrap();
            let substituted = f.substitute(&hashmap!{ id.into_inner() => g.clone() }).unwrap();

            let g_value = g.evaluate(&s, 1e-9).unwrap();
            s.entries.insert(id.into_inner(), g_value);

            let f_value = f.evaluate(&s, 1e-9).unwrap();
            let substituted_value = substituted.evaluate(&s, 1e-9).unwrap();

            prop_assert!(abs_diff_eq!(f_value, substituted_value, epsilon = 1e-9));
        }
    }
}
