use super::*;
use crate::{FormattedFunction, FunctionFormatOptions};
use std::collections::BTreeMap;

impl Instance {
    /// Format a function using this instance's decision-variable modeling labels.
    ///
    /// This validates that every ID referenced by `function` is a decision
    /// variable owned by this instance before applying any output budget.
    pub fn format_function(&self, function: &Function) -> crate::Result<String> {
        Ok(self
            .format_function_with(function, FunctionFormatOptions::default())?
            .text)
    }

    /// Format a function using this instance's decision-variable modeling labels
    /// and explicit output limits.
    ///
    /// The context-free [`std::fmt::Display`] implementation for [`Function`]
    /// is unchanged; use this method when the enclosing instance should resolve
    /// variable IDs into modeling labels.
    pub fn format_function_with(
        &self,
        function: &Function,
        opts: FunctionFormatOptions,
    ) -> crate::Result<FormattedFunction> {
        let ids = validate_instance_ids(function, self.decision_variables())?;
        let symbols = symbols_for_ids(&ids, |id| self.variable_labels().collect_for(id));
        crate::format::format_function_with_symbols(function, &symbols, opts)
    }
}

impl ParametricInstance {
    /// Format a function using this parametric instance's decision-variable and
    /// parameter modeling labels.
    ///
    /// This validates that every ID referenced by `function` belongs to exactly
    /// one of the decision-variable table or parameter table before applying
    /// any output budget.
    pub fn format_function(&self, function: &Function) -> crate::Result<String> {
        Ok(self
            .format_function_with(function, FunctionFormatOptions::default())?
            .text)
    }

    /// Format a function using this parametric instance's decision-variable and
    /// parameter modeling labels and explicit output limits.
    ///
    /// An ID that resolves as both a decision variable and a parameter, or as
    /// neither, is rejected before any term or character truncation is applied.
    pub fn format_function_with(
        &self,
        function: &Function,
        opts: FunctionFormatOptions,
    ) -> crate::Result<FormattedFunction> {
        let ids = validate_parametric_instance_ids(
            function,
            self.decision_variables(),
            self.parameters(),
        )?;
        let symbols = symbols_for_ids(&ids, |id| {
            if self.decision_variables().contains_key(&id) {
                self.variable_labels().collect_for(id)
            } else {
                self.parameters().labels().collect_for(id)
            }
        });
        crate::format::format_function_with_symbols(function, &symbols, opts)
    }
}

fn validate_instance_ids(
    function: &Function,
    decision_variables: &BTreeMap<VariableID, DecisionVariable>,
) -> crate::Result<Vec<VariableID>> {
    let ids = sorted_required_ids(function);
    for id in &ids {
        if !decision_variables.contains_key(id) {
            crate::bail!(
                { ?id },
                "Function references unknown decision variable ID {id:?}",
            );
        }
    }
    Ok(ids)
}

fn validate_parametric_instance_ids(
    function: &Function,
    decision_variables: &BTreeMap<VariableID, DecisionVariable>,
    parameters: &ParameterTable,
) -> crate::Result<Vec<VariableID>> {
    let ids = sorted_required_ids(function);
    for id in &ids {
        match (
            decision_variables.contains_key(id),
            parameters.contains_key(id),
        ) {
            (true, false) | (false, true) => {}
            (false, false) => {
                crate::bail!(
                    { ?id },
                    "Function references unknown decision variable or parameter ID {id:?}",
                );
            }
            (true, true) => {
                crate::bail!(
                    { ?id },
                    "Function ID {id:?} is both a decision variable and a parameter",
                );
            }
        }
    }
    Ok(ids)
}

fn sorted_required_ids(function: &Function) -> Vec<VariableID> {
    function.required_ids().into_iter().collect()
}

fn symbols_for_ids(
    ids: &[VariableID],
    mut label_for: impl FnMut(VariableID) -> ModelingLabel,
) -> BTreeMap<VariableID, String> {
    let mut symbols = BTreeMap::new();
    let mut collisions: BTreeMap<String, Vec<VariableID>> = BTreeMap::new();

    for &id in ids {
        let symbol = label_to_symbol(id, label_for(id));
        collisions.entry(symbol.clone()).or_default().push(id);
        symbols.insert(id, symbol);
    }

    for (_, ids) in collisions.into_iter().filter(|(_, ids)| ids.len() > 1) {
        for id in ids {
            if let Some(symbol) = symbols.get_mut(&id) {
                symbol.push_str(&format!("{{id={}}}", id.into_inner()));
            }
        }
    }

    symbols
}

fn label_to_symbol(id: VariableID, label: ModelingLabel) -> String {
    let ModelingLabel {
        name,
        subscripts,
        parameters,
        description: _,
    } = label;
    let mut symbol = name.unwrap_or_else(|| format!("x{}", id.into_inner()));

    let mut parts: Vec<String> = subscripts.into_iter().map(|i| i.to_string()).collect();
    let mut parameters: Vec<_> = parameters.into_iter().collect();
    parameters.sort_unstable_by(|(a_key, a_value), (b_key, b_value)| {
        a_key.cmp(b_key).then_with(|| a_value.cmp(b_value))
    });
    parts.extend(
        parameters
            .into_iter()
            .map(|(key, value)| format!("{key}={value}")),
    );

    if !parts.is_empty() {
        symbol.push('[');
        symbol.push_str(&parts.join(", "));
        symbol.push(']');
    }

    symbol
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{coeff, linear, quadratic, DecisionVariable, ParameterTable, VariableID};
    use maplit::btreemap;
    use std::collections::BTreeSet;

    fn instance_with_labels(labels: Vec<(u64, ModelingLabel)>) -> Instance {
        let mut variable_labels = VariableLabelStore::default();
        let mut decision_variables = BTreeMap::new();
        for (id, label) in labels {
            let id = VariableID::from(id);
            decision_variables.insert(id, DecisionVariable::binary());
            variable_labels.insert(id, label);
        }
        Instance::builder()
            .sense(Sense::Minimize)
            .objective(Function::Zero)
            .decision_variables(decision_variables)
            .variable_labels(variable_labels)
            .constraints(BTreeMap::new())
            .build()
            .unwrap()
    }

    fn label(
        name: Option<&str>,
        subscripts: Vec<i64>,
        parameters: Vec<(&str, &str)>,
    ) -> ModelingLabel {
        ModelingLabel {
            name: name.map(str::to_string),
            subscripts,
            parameters: parameters
                .into_iter()
                .map(|(key, value)| (key.to_string(), value.to_string()))
                .collect(),
            description: None,
        }
    }

    #[test]
    fn context_free_display_is_unchanged() {
        let function: Function =
            (((coeff!(2.0) * linear!(1)).unwrap() - (coeff!(3.0) * linear!(2)).unwrap()).unwrap()
                + coeff!(1.0))
            .unwrap()
            .into();

        assert_eq!(function.to_string(), "2*x1 - 3*x2 + 1");
    }

    #[test]
    fn formats_named_unlabeled_subscripted_and_parameterized_variables() {
        let instance = instance_with_labels(vec![
            (1, label(Some("x"), vec![2, 1], vec![("scenario", "base")])),
            (2, label(None, vec![3], vec![("k", "v"), ("a", "b")])),
        ]);
        let function: Function = ((linear!(1) + linear!(2)).unwrap() + coeff!(5.0))
            .unwrap()
            .into();

        assert_eq!(
            instance.format_function(&function).unwrap(),
            "x[2, 1, scenario=base] + x2[3, a=b, k=v] + 5"
        );
    }

    #[test]
    fn appends_id_only_to_colliding_symbols() {
        let instance = instance_with_labels(vec![
            (1, label(Some("x"), vec![0], vec![])),
            (2, label(Some("x"), vec![0], vec![])),
            (3, label(Some("x"), vec![1], vec![])),
        ]);
        let function: Function = ((linear!(1) + linear!(2)).unwrap() + linear!(3))
            .unwrap()
            .into();

        assert_eq!(
            instance.format_function(&function).unwrap(),
            "x[0]{id=1} + x[0]{id=2} + x[1]"
        );
    }

    #[test]
    fn rejects_unknown_ids_before_truncation() {
        let instance = instance_with_labels(vec![(1, label(Some("x"), vec![], vec![]))]);
        let function: Function = ((linear!(1) + linear!(999)).unwrap() + coeff!(1.0))
            .unwrap()
            .into();

        let err = instance
            .format_function_with(
                &function,
                FunctionFormatOptions {
                    max_terms: Some(1),
                    max_chars: Some(2),
                },
            )
            .unwrap_err();
        assert!(err
            .to_string()
            .contains("unknown decision variable ID VariableID(999)"));
    }

    #[test]
    fn reports_term_and_character_truncation_metadata() {
        let instance = instance_with_labels(vec![
            (1, label(Some("x"), vec![], vec![])),
            (2, label(Some("y"), vec![], vec![])),
            (3, label(Some("z"), vec![], vec![])),
        ]);
        let function: Function = ((quadratic!(1, 2) + quadratic!(3)).unwrap() + coeff!(1.0))
            .unwrap()
            .into();

        let formatted = instance
            .format_function_with(
                &function,
                FunctionFormatOptions {
                    max_terms: Some(2),
                    max_chars: None,
                },
            )
            .unwrap();
        assert_eq!(formatted.text, "x*y + z");
        assert_eq!(formatted.total_terms, 3);
        assert_eq!(formatted.written_terms, 2);
        assert_eq!(formatted.omitted_terms, 1);
        assert!(!formatted.truncated_by_chars);

        let formatted = instance
            .format_function_with(
                &function,
                FunctionFormatOptions {
                    max_terms: None,
                    max_chars: Some(3),
                },
            )
            .unwrap();
        assert_eq!(formatted.text, "x*y");
        assert_eq!(formatted.total_terms, 3);
        assert_eq!(formatted.written_terms, 1);
        assert_eq!(formatted.omitted_terms, 2);
        assert!(formatted.truncated_by_chars);
    }

    #[test]
    fn formats_parametric_instance_parameters_with_labels() {
        let mut variable_labels = VariableLabelStore::default();
        variable_labels.insert(VariableID::from(1), label(Some("x"), vec![1], vec![]));

        let mut parameter_labels = crate::ParameterLabelStore::default();
        parameter_labels.insert(
            VariableID::from(100),
            label(Some("p"), vec![], vec![("scenario", "base")]),
        );
        let parameters =
            ParameterTable::new(BTreeSet::from([VariableID::from(100)]), parameter_labels).unwrap();

        let instance = ParametricInstance::builder()
            .sense(Sense::Minimize)
            .objective(Function::Zero)
            .decision_variables(btreemap! {
                VariableID::from(1) => DecisionVariable::binary(),
            })
            .variable_labels(variable_labels)
            .parameters(parameters)
            .constraints(BTreeMap::new())
            .build()
            .unwrap();
        let function: Function = (linear!(1) + linear!(100)).unwrap().into();

        assert_eq!(
            instance.format_function(&function).unwrap(),
            "x[1] + p[scenario=base]"
        );
    }

    #[test]
    fn parametric_instance_rejects_ids_that_resolve_to_both_tables() {
        let mut instance = ParametricInstance::builder()
            .sense(Sense::Minimize)
            .objective(Function::Zero)
            .decision_variables(btreemap! {
                VariableID::from(1) => DecisionVariable::binary(),
            })
            .parameters(ParameterTable::default())
            .constraints(BTreeMap::new())
            .build()
            .unwrap();
        instance
            .parameters
            .insert(VariableID::from(1), ModelingLabel::default())
            .unwrap();

        let function: Function = linear!(1).into();
        let err = instance.format_function(&function).unwrap_err();
        assert!(err
            .to_string()
            .contains("both a decision variable and a parameter"));
    }

    #[test]
    fn parametric_instance_rejects_unknown_ids() {
        let instance = ParametricInstance::builder()
            .sense(Sense::Minimize)
            .objective(Function::Zero)
            .decision_variables(BTreeMap::new())
            .parameters(ParameterTable::default())
            .constraints(BTreeMap::new())
            .build()
            .unwrap();

        let function: Function = linear!(999).into();
        let err = instance.format_function(&function).unwrap_err();
        assert!(err
            .to_string()
            .contains("unknown decision variable or parameter ID VariableID(999)"));
    }
}
