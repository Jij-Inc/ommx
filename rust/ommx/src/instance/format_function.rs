use super::*;
use crate::{Equality, FormattedFunction, FunctionFormatOptions};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::{self, Write as _};

const SUMMARY_FUNCTION_FORMAT_OPTIONS: FunctionFormatOptions = FunctionFormatOptions {
    max_terms: Some(20),
    max_chars: Some(500),
};

impl Instance {
    /// Format a compact, context-aware summary of this instance.
    ///
    /// This is intended for user-facing `print(instance)` / `Display` output.
    /// It resolves decision-variable labels through the instance while keeping
    /// large expressions bounded.
    pub fn format_summary(&self) -> String {
        format_instance_summary(self)
    }

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
    /// still renders raw variable IDs; use this method when the enclosing
    /// instance should resolve IDs into modeling labels.
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
    /// Format a compact, context-aware summary of this parametric instance.
    ///
    /// This is intended for user-facing `print(instance)` / `Display` output.
    /// It resolves decision-variable and parameter labels through the
    /// parametric instance while keeping large expressions bounded.
    pub fn format_summary(&self) -> String {
        format_parametric_instance_summary(self)
    }

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

impl fmt::Display for Instance {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.format_summary())
    }
}

impl fmt::Display for ParametricInstance {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.format_summary())
    }
}

fn format_instance_summary(instance: &Instance) -> String {
    let mut out = String::new();
    write_instance_header(&mut out, "Instance", &instance.sense(), None, instance);
    writeln!(
        out,
        "Objective:\n  {}",
        format_instance_function_preview(instance, instance.objective())
    )
    .unwrap();
    write_regular_constraints(
        &mut out,
        "Constraints",
        instance.constraints().iter().map(|(id, c)| (*id, c, None)),
        instance.constraint_context(),
        |function| format_instance_function_preview(instance, function),
    );
    write_regular_constraints(
        &mut out,
        "Removed constraints",
        instance
            .removed_constraints()
            .iter()
            .map(|(id, (constraint, reason))| (*id, constraint, Some(reason))),
        instance.constraint_context(),
        |function| format_instance_function_preview(instance, function),
    );
    write_indicator_constraints(
        &mut out,
        "Indicator constraints",
        instance
            .indicator_constraints()
            .iter()
            .map(|(id, c)| (*id, c, None)),
        instance.indicator_constraint_context(),
        |function| format_instance_function_preview(instance, function),
        |ids| format_instance_variable_set(instance, ids),
    );
    write_indicator_constraints(
        &mut out,
        "Removed indicator constraints",
        instance
            .removed_indicator_constraints()
            .iter()
            .map(|(id, (constraint, reason))| (*id, constraint, Some(reason))),
        instance.indicator_constraint_context(),
        |function| format_instance_function_preview(instance, function),
        |ids| format_instance_variable_set(instance, ids),
    );
    write_one_hot_constraints(
        &mut out,
        "One-hot constraints",
        instance
            .one_hot_constraints()
            .iter()
            .map(|(id, c)| (*id, c, None)),
        instance.one_hot_constraint_context(),
        |ids| format_instance_variable_set(instance, ids),
    );
    write_one_hot_constraints(
        &mut out,
        "Removed one-hot constraints",
        instance
            .removed_one_hot_constraints()
            .iter()
            .map(|(id, (constraint, reason))| (*id, constraint, Some(reason))),
        instance.one_hot_constraint_context(),
        |ids| format_instance_variable_set(instance, ids),
    );
    write_sos1_constraints(
        &mut out,
        "SOS1 constraints",
        instance
            .sos1_constraints()
            .iter()
            .map(|(id, c)| (*id, c, None)),
        instance.sos1_constraint_context(),
        |ids| format_instance_variable_set(instance, ids),
    );
    write_sos1_constraints(
        &mut out,
        "Removed SOS1 constraints",
        instance
            .removed_sos1_constraints()
            .iter()
            .map(|(id, (constraint, reason))| (*id, constraint, Some(reason))),
        instance.sos1_constraint_context(),
        |ids| format_instance_variable_set(instance, ids),
    );
    write_named_functions(
        &mut out,
        "Named functions",
        instance.named_functions(),
        instance.named_function_labels(),
        |function| format_instance_function_preview(instance, function),
    );
    trim_trailing_newline(out)
}

fn format_parametric_instance_summary(instance: &ParametricInstance) -> String {
    let mut out = String::new();
    write_instance_header(
        &mut out,
        "ParametricInstance",
        instance.sense(),
        Some(instance.parameters().len()),
        instance,
    );
    writeln!(
        out,
        "Objective:\n  {}",
        format_parametric_instance_function_preview(instance, instance.objective())
    )
    .unwrap();
    write_regular_constraints(
        &mut out,
        "Constraints",
        instance.constraints().iter().map(|(id, c)| (*id, c, None)),
        instance.constraint_context(),
        |function| format_parametric_instance_function_preview(instance, function),
    );
    write_regular_constraints(
        &mut out,
        "Removed constraints",
        instance
            .removed_constraints()
            .iter()
            .map(|(id, (constraint, reason))| (*id, constraint, Some(reason))),
        instance.constraint_context(),
        |function| format_parametric_instance_function_preview(instance, function),
    );
    write_indicator_constraints(
        &mut out,
        "Indicator constraints",
        instance
            .indicator_constraints()
            .iter()
            .map(|(id, c)| (*id, c, None)),
        instance.indicator_constraint_context(),
        |function| format_parametric_instance_function_preview(instance, function),
        |ids| format_parametric_instance_variable_set(instance, ids),
    );
    write_indicator_constraints(
        &mut out,
        "Removed indicator constraints",
        instance
            .removed_indicator_constraints()
            .iter()
            .map(|(id, (constraint, reason))| (*id, constraint, Some(reason))),
        instance.indicator_constraint_context(),
        |function| format_parametric_instance_function_preview(instance, function),
        |ids| format_parametric_instance_variable_set(instance, ids),
    );
    write_one_hot_constraints(
        &mut out,
        "One-hot constraints",
        instance
            .one_hot_constraints()
            .iter()
            .map(|(id, c)| (*id, c, None)),
        instance.one_hot_constraint_context(),
        |ids| format_parametric_instance_variable_set(instance, ids),
    );
    write_one_hot_constraints(
        &mut out,
        "Removed one-hot constraints",
        instance
            .removed_one_hot_constraints()
            .iter()
            .map(|(id, (constraint, reason))| (*id, constraint, Some(reason))),
        instance.one_hot_constraint_context(),
        |ids| format_parametric_instance_variable_set(instance, ids),
    );
    write_sos1_constraints(
        &mut out,
        "SOS1 constraints",
        instance
            .sos1_constraints()
            .iter()
            .map(|(id, c)| (*id, c, None)),
        instance.sos1_constraint_context(),
        |ids| format_parametric_instance_variable_set(instance, ids),
    );
    write_sos1_constraints(
        &mut out,
        "Removed SOS1 constraints",
        instance
            .removed_sos1_constraints()
            .iter()
            .map(|(id, (constraint, reason))| (*id, constraint, Some(reason))),
        instance.sos1_constraint_context(),
        |ids| format_parametric_instance_variable_set(instance, ids),
    );
    write_named_functions(
        &mut out,
        "Named functions",
        instance.named_functions(),
        instance.named_function_labels(),
        |function| format_parametric_instance_function_preview(instance, function),
    );
    trim_trailing_newline(out)
}

fn write_instance_header<T>(
    out: &mut String,
    type_name: &str,
    sense: &Sense,
    parameters: Option<usize>,
    instance: &T,
) where
    T: InstanceSummaryCounts,
{
    write!(
        out,
        "{type_name}(sense={}, decision_variables={}",
        sense_label(*sense),
        instance.decision_variable_count(),
    )
    .unwrap();
    if let Some(parameters) = parameters {
        write!(out, ", parameters={parameters}").unwrap();
    }
    writeln!(
        out,
        ", active_constraints={}, removed_constraints={}, named_functions={})",
        instance.active_constraint_count(),
        instance.removed_constraint_count(),
        instance.named_function_count(),
    )
    .unwrap();
}

trait InstanceSummaryCounts {
    fn decision_variable_count(&self) -> usize;
    fn active_constraint_count(&self) -> usize;
    fn removed_constraint_count(&self) -> usize;
    fn named_function_count(&self) -> usize;
}

impl InstanceSummaryCounts for Instance {
    fn decision_variable_count(&self) -> usize {
        self.decision_variables().len()
    }

    fn active_constraint_count(&self) -> usize {
        active_constraint_count(self)
    }

    fn removed_constraint_count(&self) -> usize {
        removed_constraint_count(self)
    }

    fn named_function_count(&self) -> usize {
        self.named_functions().len()
    }
}

impl InstanceSummaryCounts for ParametricInstance {
    fn decision_variable_count(&self) -> usize {
        self.decision_variables().len()
    }

    fn active_constraint_count(&self) -> usize {
        active_constraint_count(self)
    }

    fn removed_constraint_count(&self) -> usize {
        removed_constraint_count(self)
    }

    fn named_function_count(&self) -> usize {
        self.named_functions().len()
    }
}

trait ConstraintFamilyCounts {
    fn regular_constraints_count(&self) -> usize;
    fn removed_regular_constraints_count(&self) -> usize;
    fn indicator_constraints_count(&self) -> usize;
    fn removed_indicator_constraints_count(&self) -> usize;
    fn one_hot_constraints_count(&self) -> usize;
    fn removed_one_hot_constraints_count(&self) -> usize;
    fn sos1_constraints_count(&self) -> usize;
    fn removed_sos1_constraints_count(&self) -> usize;
}

impl ConstraintFamilyCounts for Instance {
    fn regular_constraints_count(&self) -> usize {
        self.constraints().len()
    }

    fn removed_regular_constraints_count(&self) -> usize {
        self.removed_constraints().len()
    }

    fn indicator_constraints_count(&self) -> usize {
        self.indicator_constraints().len()
    }

    fn removed_indicator_constraints_count(&self) -> usize {
        self.removed_indicator_constraints().len()
    }

    fn one_hot_constraints_count(&self) -> usize {
        self.one_hot_constraints().len()
    }

    fn removed_one_hot_constraints_count(&self) -> usize {
        self.removed_one_hot_constraints().len()
    }

    fn sos1_constraints_count(&self) -> usize {
        self.sos1_constraints().len()
    }

    fn removed_sos1_constraints_count(&self) -> usize {
        self.removed_sos1_constraints().len()
    }
}

impl ConstraintFamilyCounts for ParametricInstance {
    fn regular_constraints_count(&self) -> usize {
        self.constraints().len()
    }

    fn removed_regular_constraints_count(&self) -> usize {
        self.removed_constraints().len()
    }

    fn indicator_constraints_count(&self) -> usize {
        self.indicator_constraints().len()
    }

    fn removed_indicator_constraints_count(&self) -> usize {
        self.removed_indicator_constraints().len()
    }

    fn one_hot_constraints_count(&self) -> usize {
        self.one_hot_constraints().len()
    }

    fn removed_one_hot_constraints_count(&self) -> usize {
        self.removed_one_hot_constraints().len()
    }

    fn sos1_constraints_count(&self) -> usize {
        self.sos1_constraints().len()
    }

    fn removed_sos1_constraints_count(&self) -> usize {
        self.removed_sos1_constraints().len()
    }
}

fn active_constraint_count(instance: &impl ConstraintFamilyCounts) -> usize {
    instance.regular_constraints_count()
        + instance.indicator_constraints_count()
        + instance.one_hot_constraints_count()
        + instance.sos1_constraints_count()
}

fn removed_constraint_count(instance: &impl ConstraintFamilyCounts) -> usize {
    instance.removed_regular_constraints_count()
        + instance.removed_indicator_constraints_count()
        + instance.removed_one_hot_constraints_count()
        + instance.removed_sos1_constraints_count()
}

fn write_regular_constraints<'a, I>(
    out: &mut String,
    title: &str,
    constraints: I,
    context: &ConstraintContextStore<ConstraintID>,
    mut format_function: impl FnMut(&Function) -> String,
) where
    I: IntoIterator<Item = (ConstraintID, &'a Constraint, Option<&'a RemovedReason>)>,
{
    let mut iter = constraints.into_iter().peekable();
    if iter.peek().is_none() {
        return;
    }

    writeln!(out, "{title}:").unwrap();
    for (id, constraint, removed_reason) in iter {
        write!(out, "  [{id}] ").unwrap();
        write_optional_row_label(
            out,
            row_label("c", id.into_inner(), context.collect_for(id).label),
        );
        writeln!(
            out,
            "{} {} 0{}",
            format_function(constraint.function()),
            equality_symbol(constraint.equality),
            removed_suffix(removed_reason),
        )
        .unwrap();
    }
}

fn write_indicator_constraints<'a, I>(
    out: &mut String,
    title: &str,
    constraints: I,
    context: &ConstraintContextStore<crate::IndicatorConstraintID>,
    mut format_function: impl FnMut(&Function) -> String,
    mut format_variables: impl FnMut(Vec<VariableID>) -> String,
) where
    I: IntoIterator<
        Item = (
            crate::IndicatorConstraintID,
            &'a IndicatorConstraint,
            Option<&'a RemovedReason>,
        ),
    >,
{
    let mut iter = constraints.into_iter().peekable();
    if iter.peek().is_none() {
        return;
    }

    writeln!(out, "{title}:").unwrap();
    for (id, constraint, removed_reason) in iter {
        let indicator = format_variables(vec![constraint.indicator_variable]);
        write!(out, "  [{id}] ").unwrap();
        write_optional_row_label(
            out,
            row_label("i", id.into_inner(), context.collect_for(id).label),
        );
        writeln!(
            out,
            "{indicator} = 1 -> {} {} 0{}",
            format_function(constraint.function()),
            equality_symbol(constraint.equality),
            removed_suffix(removed_reason),
        )
        .unwrap();
    }
}

fn write_one_hot_constraints<'a, I>(
    out: &mut String,
    title: &str,
    constraints: I,
    context: &ConstraintContextStore<crate::OneHotConstraintID>,
    mut format_variables: impl FnMut(Vec<VariableID>) -> String,
) where
    I: IntoIterator<
        Item = (
            crate::OneHotConstraintID,
            &'a OneHotConstraint,
            Option<&'a RemovedReason>,
        ),
    >,
{
    let mut iter = constraints.into_iter().peekable();
    if iter.peek().is_none() {
        return;
    }

    writeln!(out, "{title}:").unwrap();
    for (id, constraint, removed_reason) in iter {
        write!(out, "  [{id}] ").unwrap();
        write_optional_row_label(
            out,
            row_label("oh", id.into_inner(), context.collect_for(id).label),
        );
        writeln!(
            out,
            "exactly one of {{{}}} = 1{}",
            format_variables(constraint.variables.iter().copied().collect()),
            removed_suffix(removed_reason),
        )
        .unwrap();
    }
}

fn write_sos1_constraints<'a, I>(
    out: &mut String,
    title: &str,
    constraints: I,
    context: &ConstraintContextStore<crate::Sos1ConstraintID>,
    mut format_variables: impl FnMut(Vec<VariableID>) -> String,
) where
    I: IntoIterator<
        Item = (
            crate::Sos1ConstraintID,
            &'a Sos1Constraint,
            Option<&'a RemovedReason>,
        ),
    >,
{
    let mut iter = constraints.into_iter().peekable();
    if iter.peek().is_none() {
        return;
    }

    writeln!(out, "{title}:").unwrap();
    for (id, constraint, removed_reason) in iter {
        write!(out, "  [{id}] ").unwrap();
        write_optional_row_label(
            out,
            row_label("sos", id.into_inner(), context.collect_for(id).label),
        );
        writeln!(
            out,
            "at most one of {{{}}} != 0{}",
            format_variables(constraint.variables.iter().copied().collect()),
            removed_suffix(removed_reason),
        )
        .unwrap();
    }
}

fn write_named_functions(
    out: &mut String,
    title: &str,
    named_functions: &BTreeMap<NamedFunctionID, NamedFunction>,
    labels: &crate::named_function::NamedFunctionLabelStore,
    mut format_function: impl FnMut(&Function) -> String,
) {
    if named_functions.is_empty() {
        return;
    }

    writeln!(out, "{title}:").unwrap();
    for (id, named_function) in named_functions {
        write!(out, "  [{id}] ").unwrap();
        write_optional_row_label(
            out,
            row_label("f", id.into_inner(), labels.collect_for(*id)),
        );
        writeln!(out, "{}", format_function(&named_function.function)).unwrap();
    }
}

fn write_optional_row_label(out: &mut String, label: Option<String>) {
    if let Some(label) = label {
        write!(out, "{label}: ").unwrap();
    }
}

fn removed_suffix(reason: Option<&RemovedReason>) -> String {
    match reason {
        Some(reason) if reason.reason.is_empty() => " (removed)".to_string(),
        Some(reason) => format!(" (removed: {})", reason.reason),
        None => String::new(),
    }
}

fn sense_label(sense: Sense) -> &'static str {
    match sense {
        Sense::Minimize => "minimize",
        Sense::Maximize => "maximize",
    }
}

fn equality_symbol(equality: Equality) -> &'static str {
    match equality {
        Equality::EqualToZero => "==",
        Equality::LessThanOrEqualToZero => "<=",
    }
}

fn row_label(prefix: &str, id: u64, label: ModelingLabel) -> Option<String> {
    if label.name.as_deref().is_none_or(str::is_empty)
        && label.subscripts.is_empty()
        && label.parameters.is_empty()
    {
        None
    } else {
        Some(modeling_label_to_symbol(format!("{prefix}{id}"), label))
    }
}

fn format_instance_function_preview(instance: &Instance, function: &Function) -> String {
    format_function_preview(
        instance
            .format_function_with(function, SUMMARY_FUNCTION_FORMAT_OPTIONS)
            .unwrap_or_else(|_| fallback_formatted_function(function)),
    )
}

fn format_parametric_instance_function_preview(
    instance: &ParametricInstance,
    function: &Function,
) -> String {
    format_function_preview(
        instance
            .format_function_with(function, SUMMARY_FUNCTION_FORMAT_OPTIONS)
            .unwrap_or_else(|_| fallback_formatted_function(function)),
    )
}

fn format_function_preview(formatted: FormattedFunction) -> String {
    let mut text = formatted.text;
    if formatted.truncated_by_chars {
        text.push_str("...");
    }
    if formatted.omitted_terms > 0 {
        if !text.is_empty() {
            text.push(' ');
        }
        write!(
            text,
            "({} term{} omitted)",
            formatted.omitted_terms,
            if formatted.omitted_terms == 1 {
                ""
            } else {
                "s"
            },
        )
        .unwrap();
    }
    text
}

fn fallback_formatted_function(function: &Function) -> FormattedFunction {
    FormattedFunction {
        text: function.to_string(),
        total_terms: 0,
        written_terms: 0,
        omitted_terms: 0,
        truncated_by_chars: false,
    }
}

fn format_instance_variable_set(instance: &Instance, ids: Vec<VariableID>) -> String {
    format_variable_set(ids, |id| instance.variable_labels().collect_for(id))
}

fn format_parametric_instance_variable_set(
    instance: &ParametricInstance,
    ids: Vec<VariableID>,
) -> String {
    format_variable_set(ids, |id| instance.variable_labels().collect_for(id))
}

fn format_variable_set(
    ids: Vec<VariableID>,
    mut label_for: impl FnMut(VariableID) -> ModelingLabel,
) -> String {
    let symbols = symbols_for_ids(&ids, |id| label_for(id));
    ids.into_iter()
        .filter_map(|id| symbols.get(&id).cloned())
        .collect::<Vec<_>>()
        .join(", ")
}

fn trim_trailing_newline(mut text: String) -> String {
    if text.ends_with('\n') {
        text.pop();
    }
    text
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
    let mut ids: Vec<_> = function.required_ids().into_iter().collect();
    ids.sort_unstable();
    ids
}

fn symbols_for_ids(
    ids: &[VariableID],
    mut label_for: impl FnMut(VariableID) -> ModelingLabel,
) -> BTreeMap<VariableID, String> {
    let mut base_symbols = BTreeMap::new();
    let mut base_collisions: BTreeMap<String, Vec<VariableID>> = BTreeMap::new();

    for &id in ids {
        let symbol = label_to_symbol(id, label_for(id));
        base_collisions.entry(symbol.clone()).or_default().push(id);
        base_symbols.insert(id, symbol);
    }

    let disambiguated_ids: BTreeSet<_> = base_collisions
        .values()
        .filter(|ids| ids.len() > 1)
        .flatten()
        .copied()
        .collect();
    let mut reserved_symbols: BTreeSet<_> = base_symbols
        .iter()
        .filter(|(id, _)| !disambiguated_ids.contains(id))
        .map(|(_, symbol)| symbol.clone())
        .collect();

    let mut symbols = BTreeMap::new();
    for (id, mut symbol) in base_symbols {
        if disambiguated_ids.contains(&id) {
            append_id_suffix(&mut symbol, id);
            while reserved_symbols.contains(&symbol) {
                append_id_suffix(&mut symbol, id);
            }
        }
        reserved_symbols.insert(symbol.clone());
        symbols.insert(id, symbol);
    }

    symbols
}

fn append_id_suffix(symbol: &mut String, id: VariableID) {
    symbol.push_str(&format!("{{id={}}}", id.into_inner()));
}

fn label_to_symbol(id: VariableID, label: ModelingLabel) -> String {
    modeling_label_to_symbol(format!("x{}", id.into_inner()), label)
}

fn modeling_label_to_symbol(fallback: String, label: ModelingLabel) -> String {
    let ModelingLabel {
        name,
        subscripts,
        parameters,
        description: _,
    } = label;
    let mut symbol = name.filter(|name| !name.is_empty()).unwrap_or(fallback);

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
    fn context_free_display_remains_raw_id_based() {
        let function: Function =
            (((coeff!(2.0) * linear!(1)).unwrap() - (coeff!(3.0) * linear!(2)).unwrap()).unwrap()
                + coeff!(1.0))
            .unwrap()
            .into();

        insta::assert_snapshot!(function.to_string(), @"2*x1 - 3*x2 + 1");
    }

    #[test]
    fn context_free_display_preserves_tiny_nonzero_coefficients() {
        let function: Function = (coeff!(1e-20) * linear!(1)).unwrap().into();

        insta::assert_snapshot!(function.to_string(), @"0.00000000000000000001*x1");
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

        insta::assert_snapshot!(
            instance.format_function(&function).unwrap(),
            @"x[2, 1, scenario=base] + x2[3, a=b, k=v] + 5"
        );
    }

    #[test]
    fn empty_names_fall_back_to_id_symbols() {
        let instance = instance_with_labels(vec![(
            1,
            label(Some(""), vec![2], vec![("scenario", "base")]),
        )]);
        let function: Function = linear!(1).into();

        insta::assert_snapshot!(
            instance.format_function(&function).unwrap(),
            @"x1[2, scenario=base]"
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

        insta::assert_snapshot!(
            instance.format_function(&function).unwrap(),
            @"x[0]{id=1} + x[0]{id=2} + x[1]"
        );
    }

    #[test]
    fn context_aware_display_preserves_tiny_nonzero_coefficients() {
        let instance = instance_with_labels(vec![(1, label(Some("x"), vec![], vec![]))]);
        let function: Function = (coeff!(1e-20) * linear!(1)).unwrap().into();

        let formatted = instance
            .format_function_with(&function, FunctionFormatOptions::default())
            .unwrap();
        insta::assert_debug_snapshot!(formatted, @r###"
        FormattedFunction {
            text: "0.00000000000000000001*x",
            total_terms: 1,
            written_terms: 1,
            omitted_terms: 0,
            truncated_by_chars: false,
        }
        "###);
    }

    #[test]
    fn avoids_generated_suffix_collisions_with_user_labels() {
        let instance = instance_with_labels(vec![
            (1, label(Some("x"), vec![], vec![])),
            (2, label(Some("x"), vec![], vec![])),
            (3, label(Some("x{id=1}"), vec![], vec![])),
        ]);
        let function: Function = ((linear!(1) + linear!(2)).unwrap() + linear!(3))
            .unwrap()
            .into();

        insta::assert_snapshot!(
            instance.format_function(&function).unwrap(),
            @"x{id=1}{id=1} + x{id=2} + x{id=1}"
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
        insta::assert_snapshot!(
            err.to_string(),
            @"Function references unknown decision variable ID VariableID(999)"
        );
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
        insta::assert_debug_snapshot!(formatted, @r###"
        FormattedFunction {
            text: "x*y + z",
            total_terms: 3,
            written_terms: 2,
            omitted_terms: 1,
            truncated_by_chars: false,
        }
        "###);

        let formatted = instance
            .format_function_with(
                &function,
                FunctionFormatOptions {
                    max_terms: None,
                    max_chars: Some(3),
                },
            )
            .unwrap();
        insta::assert_debug_snapshot!(formatted, @r###"
        FormattedFunction {
            text: "x*y",
            total_terms: 3,
            written_terms: 1,
            omitted_terms: 2,
            truncated_by_chars: true,
        }
        "###);
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

        insta::assert_snapshot!(
            instance.format_function(&function).unwrap(),
            @"x[1] + p[scenario=base]"
        );
    }

    #[test]
    fn parametric_instance_empty_parameter_names_fall_back_to_id_symbols() {
        let mut parameter_labels = crate::ParameterLabelStore::default();
        parameter_labels.insert(
            VariableID::from(100),
            label(Some(""), vec![], vec![("scenario", "base")]),
        );
        let parameters =
            ParameterTable::new(BTreeSet::from([VariableID::from(100)]), parameter_labels).unwrap();

        let instance = ParametricInstance::builder()
            .sense(Sense::Minimize)
            .objective(Function::Zero)
            .decision_variables(BTreeMap::new())
            .parameters(parameters)
            .constraints(BTreeMap::new())
            .build()
            .unwrap();
        let function: Function = linear!(100).into();

        insta::assert_snapshot!(
            instance.format_function(&function).unwrap(),
            @"x100[scenario=base]"
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
        insta::assert_snapshot!(
            err.to_string(),
            @"Function ID VariableID(1) is both a decision variable and a parameter"
        );
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
        insta::assert_snapshot!(
            err.to_string(),
            @"Function references unknown decision variable or parameter ID VariableID(999)"
        );
    }

    #[test]
    fn instance_summary_uses_owned_modeling_labels() {
        let instance = {
            let mut variable_labels = VariableLabelStore::default();
            variable_labels.insert(VariableID::from(0), label(Some("x"), vec![0], vec![]));
            variable_labels.insert(VariableID::from(1), label(Some("x"), vec![1], vec![]));
            variable_labels.insert(VariableID::from(2), label(Some("y"), vec![], vec![]));

            let decision_variables = btreemap! {
                VariableID::from(0) => DecisionVariable::binary(),
                VariableID::from(1) => DecisionVariable::binary(),
                VariableID::from(2) => DecisionVariable::integer(),
            };

            let constraint_function: Function = ((linear!(0) + linear!(1)).unwrap() - coeff!(1.0))
                .unwrap()
                .into();
            let constraints = btreemap! {
                ConstraintID::from(10) => Constraint::less_than_or_equal_to_zero(constraint_function),
            };
            let mut constraint_context = ConstraintContextStore::default();
            constraint_context.set_name(ConstraintID::from(10), "capacity");
            constraint_context.set_subscripts(ConstraintID::from(10), vec![0]);

            let named_function: Function = ((coeff!(2.0) * linear!(2)).unwrap() + coeff!(3.0))
                .unwrap()
                .into();
            let named_functions = btreemap! {
                NamedFunctionID::from(5) => NamedFunction { function: named_function },
            };
            let mut named_function_labels =
                crate::named_function::NamedFunctionLabelStore::default();
            named_function_labels.set_name(NamedFunctionID::from(5), "score");

            Instance::builder()
                .sense(Sense::Maximize)
                .objective((linear!(0) + linear!(1)).unwrap().into())
                .decision_variables(decision_variables)
                .variable_labels(variable_labels)
                .constraints(constraints)
                .constraint_context(constraint_context)
                .named_functions(named_functions)
                .named_function_labels(named_function_labels)
                .build()
                .unwrap()
        };

        insta::assert_snapshot!(instance.format_summary(), @r###"
        Instance(sense=maximize, decision_variables=3, active_constraints=1, removed_constraints=0, named_functions=1)
        Objective:
          x[0] + x[1]
        Constraints:
          [10] capacity[0]: x[0] + x[1] - 1 <= 0
        Named functions:
          [5] score: 2*y + 3
        "###);
        assert_eq!(instance.to_string(), instance.format_summary());
    }
}
