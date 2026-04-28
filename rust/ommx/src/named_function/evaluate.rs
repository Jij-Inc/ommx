use super::*;
use crate::{Evaluate, VariableIDSet};

impl Evaluate for NamedFunction {
    type Output = EvaluatedNamedFunction;
    type SampledOutput = SampledNamedFunction;

    fn evaluate(
        &self,
        solution: &crate::v1::State,
        atol: crate::ATol,
    ) -> crate::Result<Self::Output> {
        let evaluated_value = self.function.evaluate(solution, atol)?;
        let used_decision_variable_ids = self.function.required_ids();
        Ok(EvaluatedNamedFunction {
            id: self.id,
            evaluated_value,
            used_decision_variable_ids,
        })
    }

    fn partial_evaluate(
        &mut self,
        state: &crate::v1::State,
        atol: crate::ATol,
    ) -> crate::Result<()> {
        self.function.partial_evaluate(state, atol)
    }

    fn required_ids(&self) -> VariableIDSet {
        self.function.required_ids()
    }

    fn evaluate_samples(
        &self,
        samples: &crate::Sampled<crate::v1::State>,
        atol: crate::ATol,
    ) -> crate::Result<Self::SampledOutput> {
        let evaluated_values = self.function.evaluate_samples(samples, atol)?;
        let used_decision_variable_ids = self.function.required_ids();
        Ok(SampledNamedFunction {
            id: self.id,
            evaluated_values,
            used_decision_variable_ids,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{coeff, linear, Coefficient, Evaluate, Function, VariableID};
    use maplit::btreeset;

    #[test]
    fn test_evaluate_constant_function() {
        // NamedFunction with a constant function
        let nf = NamedFunction {
            id: NamedFunctionID::from(1),
            function: Function::Constant(Coefficient::try_from(42.0).unwrap()),
        };

        let state = crate::v1::State::default();
        let result = nf.evaluate(&state, crate::ATol::default()).unwrap();

        assert_eq!(result.id(), NamedFunctionID::from(1));
        assert_eq!(result.evaluated_value(), 42.0);
        assert!(result.used_decision_variable_ids().is_empty());
    }

    #[test]
    fn test_evaluate_linear_function() {
        // NamedFunction with 2*x1 + 3*x2
        let nf = NamedFunction {
            id: NamedFunctionID::from(2),
            function: Function::Linear(coeff!(2.0) * linear!(1) + coeff!(3.0) * linear!(2)),
        };

        // x1 = 5.0, x2 = 10.0 => 2*5 + 3*10 = 40.0
        let state = crate::v1::State {
            entries: [(1, 5.0), (2, 10.0)].into_iter().collect(),
        };
        let result = nf.evaluate(&state, crate::ATol::default()).unwrap();

        assert_eq!(result.evaluated_value(), 40.0);
        assert_eq!(
            *result.used_decision_variable_ids(),
            btreeset! { VariableID::from(1), VariableID::from(2) }
        );
    }

    #[test]
    fn test_required_ids() {
        // NamedFunction with a linear function referencing variables 1 and 2
        let nf = NamedFunction {
            id: NamedFunctionID::from(3),
            function: Function::Linear(coeff!(2.0) * linear!(1) + coeff!(3.0) * linear!(2)),
        };

        let ids = nf.required_ids();
        assert_eq!(ids, btreeset! { VariableID::from(1), VariableID::from(2) });
    }
}
