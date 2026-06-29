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
            evaluated_values,
            used_decision_variable_ids,
        })
    }
}

impl Evaluate for NamedFunctionTable<NamedFunction> {
    type Output = NamedFunctionTable<EvaluatedNamedFunction>;
    type SampledOutput = NamedFunctionTable<SampledNamedFunction>;

    fn evaluate(&self, state: &crate::v1::State, atol: crate::ATol) -> crate::Result<Self::Output> {
        let mut results = std::collections::BTreeMap::new();
        for (id, named_function) in &self.entries {
            let evaluated = named_function.evaluate(state, atol).inspect_err(|e| {
                tracing::error!(?id, error = %e, "failed to evaluate named function");
            })?;
            results.insert(*id, evaluated);
        }
        NamedFunctionTable::new(results, self.labels.clone())
    }

    fn partial_evaluate(
        &mut self,
        state: &crate::v1::State,
        atol: crate::ATol,
    ) -> crate::Result<()> {
        let mut updated = self.clone();
        for (id, named_function) in updated.entries.iter_mut() {
            named_function
                .partial_evaluate(state, atol)
                .inspect_err(|e| {
                    tracing::error!(?id, error = %e, "failed to partial_evaluate named function");
                })?;
        }
        *self = updated;
        Ok(())
    }

    fn required_ids(&self) -> VariableIDSet {
        self.entries
            .values()
            .flat_map(Evaluate::required_ids)
            .collect()
    }

    fn evaluate_samples(
        &self,
        samples: &crate::Sampled<crate::v1::State>,
        atol: crate::ATol,
    ) -> crate::Result<Self::SampledOutput> {
        let mut results = std::collections::BTreeMap::new();
        for (id, named_function) in &self.entries {
            let sampled = named_function
                .evaluate_samples(samples, atol)
                .inspect_err(|e| {
                    tracing::error!(?id, error = %e, "failed to evaluate_samples named function");
                })?;
            results.insert(*id, sampled);
        }
        NamedFunctionTable::new(results, self.labels.clone())
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
            function: Function::Constant(Coefficient::try_from(42.0).unwrap()),
        };

        let state = crate::v1::State::default();
        let result = nf.evaluate(&state, crate::ATol::default()).unwrap();

        assert_eq!(result.evaluated_value(), 42.0);
        assert!(result.used_decision_variable_ids().is_empty());
    }

    #[test]
    fn test_evaluate_linear_function() {
        // NamedFunction with 2*x1 + 3*x2
        let nf = NamedFunction {
            function: Function::Linear(
                ((coeff!(2.0) * linear!(1)).unwrap() + (coeff!(3.0) * linear!(2)).unwrap())
                    .unwrap(),
            ),
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
            function: Function::Linear(
                ((coeff!(2.0) * linear!(1)).unwrap() + (coeff!(3.0) * linear!(2)).unwrap())
                    .unwrap(),
            ),
        };

        let ids = nf.required_ids();
        assert_eq!(ids, btreeset! { VariableID::from(1), VariableID::from(2) });
    }

    #[test]
    fn test_table_evaluate_preserves_labels() {
        let id = NamedFunctionID::from(7);
        let mut entries = std::collections::BTreeMap::new();
        entries.insert(
            id,
            NamedFunction {
                function: Function::Linear(linear!(1).into()),
            },
        );
        let mut labels = NamedFunctionLabelStore::new();
        labels.set_name(id, "cost");
        labels.set_subscripts(id, vec![3]);
        let table = NamedFunctionTable::new(entries, labels).unwrap();

        let state = crate::v1::State {
            entries: [(1, 4.0)].into_iter().collect(),
        };
        let evaluated = table.evaluate(&state, crate::ATol::default()).unwrap();

        assert_eq!(evaluated.labels().name(id), Some("cost"));
        assert_eq!(evaluated.labels().subscripts(id), &[3]);
        let row = evaluated.get(&id).unwrap();
        assert_eq!(row.evaluated_value(), 4.0);
        assert_eq!(
            *row.used_decision_variable_ids(),
            btreeset! { VariableID::from(1) }
        );
    }

    #[test]
    fn test_table_evaluate_samples_preserves_labels() {
        let id = NamedFunctionID::from(8);
        let mut entries = std::collections::BTreeMap::new();
        entries.insert(
            id,
            NamedFunction {
                function: Function::Linear(linear!(1).into()),
            },
        );
        let mut labels = NamedFunctionLabelStore::new();
        labels.set_name(id, "load");
        let table = NamedFunctionTable::new(entries, labels).unwrap();

        let samples = crate::Sampled::new(
            vec![
                vec![crate::SampleID::from(0)],
                vec![crate::SampleID::from(1)],
            ],
            [
                crate::v1::State {
                    entries: [(1, 2.0)].into_iter().collect(),
                },
                crate::v1::State {
                    entries: [(1, 5.0)].into_iter().collect(),
                },
            ],
        )
        .unwrap();
        let sampled = table
            .evaluate_samples(&samples, crate::ATol::default())
            .unwrap();

        assert_eq!(sampled.labels().name(id), Some("load"));
        let row = sampled.get(&id).unwrap();
        assert_eq!(
            row.get(crate::SampleID::from(0)).unwrap().evaluated_value(),
            2.0
        );
        assert_eq!(
            row.get(crate::SampleID::from(1)).unwrap().evaluated_value(),
            5.0
        );
    }
}
