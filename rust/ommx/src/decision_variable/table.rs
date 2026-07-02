use std::collections::{BTreeMap, BTreeSet};

use crate::logical_memory::LogicalMemoryProfile;
use crate::{
    ATol, Bound, Created, Evaluated, ModelingLabel, Parse, ParseError, RawParseError, SampledStage,
};

use super::{
    DecisionVariable, DecisionVariableError, DecisionVariableLabel, EvaluatedDecisionVariable,
    SampledDecisionVariable, VariableID, VariableLabelStore,
};

mod sealed {
    pub trait Sealed {}
}

/// Maps a lifecycle stage to [`DecisionVariableTable`] rows and sidecar columns.
///
/// The stage marker itself is shared with constraints: [`Created`],
/// [`Evaluated`], or [`SampledStage`]. This trait is the decision-variable
/// table-specific binding from that lifecycle stage to the intrinsic row payload
/// and any sparse columns owned by the table at that stage.
pub trait DecisionVariableTableStage: sealed::Sealed {
    /// Intrinsic row payload stored in the table.
    type Row;
    /// Sparse sidecar columns owned by this table stage.
    ///
    /// `Default` is the domain-empty column set for the stage. For example,
    /// created-stage fixed values default to an empty map, while evaluated and
    /// sampled stages currently default to their empty column schemas.
    type Columns: Default;
    /// Additional context needed to validate stage-specific table invariants.
    type TableValidationContext: Copy;

    /// Validate stage-specific table invariants.
    fn validate_stage_table_invariants(
        entries: &BTreeMap<VariableID, Self::Row>,
        columns: &Self::Columns,
        context: Self::TableValidationContext,
    ) -> crate::Result<()>;
}

impl sealed::Sealed for Created {}
impl sealed::Sealed for Evaluated {}
impl sealed::Sealed for SampledStage {}

/// Definition-stage sparse columns.
#[derive(Debug, Clone, PartialEq, Default, LogicalMemoryProfile)]
pub struct CreatedDecisionVariableColumns {
    fixed_values: BTreeMap<VariableID, f64>,
}

/// Empty column set for evaluated decision-variable tables.
#[derive(Debug, Clone, PartialEq, Default, LogicalMemoryProfile)]
pub struct EvaluatedDecisionVariableColumns {}

/// Empty column set for sampled decision-variable tables.
#[derive(Debug, Clone, PartialEq, Default, LogicalMemoryProfile)]
pub struct SampledDecisionVariableColumns {}

impl DecisionVariableTableStage for Created {
    type Row = DecisionVariable;
    type Columns = CreatedDecisionVariableColumns;
    type TableValidationContext = ATol;

    fn validate_stage_table_invariants(
        entries: &BTreeMap<VariableID, Self::Row>,
        columns: &Self::Columns,
        atol: Self::TableValidationContext,
    ) -> crate::Result<()> {
        for (id, value) in &columns.fixed_values {
            let Some(row) = entries.get(id) else {
                crate::bail!(
                    { ?id },
                    "Fixed decision-variable value references unknown decision variable ID {id:?}",
                );
            };
            row.check_value_consistency(*id, *value, atol)?;
        }
        Ok(())
    }
}

impl DecisionVariableTableStage for Evaluated {
    type Row = EvaluatedDecisionVariable;
    type Columns = EvaluatedDecisionVariableColumns;
    type TableValidationContext = ();

    fn validate_stage_table_invariants(
        _entries: &BTreeMap<VariableID, Self::Row>,
        _columns: &Self::Columns,
        _context: Self::TableValidationContext,
    ) -> crate::Result<()> {
        Ok(())
    }
}

impl DecisionVariableTableStage for SampledStage {
    type Row = SampledDecisionVariable;
    type Columns = SampledDecisionVariableColumns;
    type TableValidationContext = ();

    fn validate_stage_table_invariants(
        _entries: &BTreeMap<VariableID, Self::Row>,
        _columns: &Self::Columns,
        _context: Self::TableValidationContext,
    ) -> crate::Result<()> {
        Ok(())
    }
}

/// Owner of decision-variable rows, modeling labels, and stage-specific columns.
///
/// [`Instance`](crate::Instance) and [`ParametricInstance`](crate::ParametricInstance)
/// use the default definition-stage table. [`Solution`](crate::Solution) uses
/// [`EvaluatedDecisionVariableTable`], and [`SampleSet`](crate::SampleSet) uses
/// [`SampledDecisionVariableTable`]. The table key owns [`VariableID`]; row
/// values own only stage-specific intrinsic payloads. [`VariableLabelStore`]
/// owns modeling labels as sidecar columns, and the stage column store owns any
/// additional sparse columns such as created-stage fixed values.
///
/// Mathematically, this table is the variable-space component
/// `X = {variable_id -> domain row}` of an enclosing root object. It may enforce
/// only facts expressible from its own rows, labels, and stage columns.
///
/// # Table-level invariants
///
/// - Every modeling-label ID is owned by this table.
/// - The table key is the only source of truth for [`VariableID`].
/// - Stage-specific columns may only reference IDs owned by this table.
/// - Created-stage fixed values are finite and satisfy the corresponding row's
///   kind/bound under the [`ATol`] supplied to [`Self::with_fixed_values`] or
///   [`Self::set_fixed_value`].
///
/// # Host-level invariants
///
/// This table does not validate cross-table semantics. The enclosing
/// [`crate::Instance`], [`crate::ParametricInstance`], [`crate::Solution`], or
/// [`crate::SampleSet`] validates role disjointness, expression references,
/// sample-ID consistency, and the shared decision-variable / parameter ID
/// namespace.
///
/// # Table-local operations
///
/// The table supports operations that are local to `X`:
///
/// - construction from rows, labels, and stage columns with key consistency
///   checks;
/// - read access to keys, rows, labels, and stage columns;
/// - fresh insertion of a created, evaluated, or sampled row with its label;
/// - created-stage fixed-value updates for existing rows;
/// - label updates for existing rows;
/// - created-stage domain intersection, preserving fixed-value consistency;
/// - host-computed by-value row replacement plans.
///
/// It intentionally does not expose arbitrary deletion or raw mutable row
/// access. Removing or semantically replacing a decision variable requires the
/// enclosing root object to prove that objective, constraints, named functions,
/// dependencies, fixed values, and parameter namespaces remain valid.
#[derive(Debug, Clone, PartialEq, LogicalMemoryProfile)]
pub struct DecisionVariableTable<S: DecisionVariableTableStage = Created> {
    entries: BTreeMap<VariableID, S::Row>,
    labels: VariableLabelStore,
    columns: S::Columns,
}

/// Evaluated-stage decision-variable table used by [`crate::Solution`].
pub type EvaluatedDecisionVariableTable = DecisionVariableTable<Evaluated>;

/// Sampled-stage decision-variable table used by [`crate::SampleSet`].
pub type SampledDecisionVariableTable = DecisionVariableTable<SampledStage>;

impl<S: DecisionVariableTableStage> DecisionVariableTable<S> {
    fn with_columns(
        entries: BTreeMap<VariableID, S::Row>,
        labels: VariableLabelStore,
        columns: S::Columns,
        table_context: S::TableValidationContext,
    ) -> crate::Result<Self> {
        Self::validate_labels(&entries, &labels)?;
        S::validate_stage_table_invariants(&entries, &columns, table_context)?;
        Ok(Self {
            entries,
            labels,
            columns,
        })
    }

    /// Intrinsic row map, keyed by table-owned [`VariableID`].
    pub fn entries(&self) -> &BTreeMap<VariableID, S::Row> {
        &self.entries
    }

    /// Per-row modeling label store.
    pub fn labels(&self) -> &VariableLabelStore {
        &self.labels
    }

    /// Replace the modeling label for an existing decision-variable row.
    pub fn set_label(&mut self, id: VariableID, label: DecisionVariableLabel) -> crate::Result<()> {
        if !self.entries.contains_key(&id) {
            crate::bail!(
                { ?id },
                "Modeling label references unknown decision variable ID {id:?}",
            );
        }
        self.labels.insert(id, label);
        Ok(())
    }

    fn insert_labeled_row(
        &mut self,
        id: VariableID,
        row: S::Row,
        label: ModelingLabel,
    ) -> Result<(), DecisionVariableError> {
        if self.entries.contains_key(&id) {
            return Err(DecisionVariableError::DuplicateID { id });
        }
        self.labels.insert(id, label);
        self.entries.insert(id, row);
        Ok(())
    }

    pub fn contains_key(&self, id: &VariableID) -> bool {
        self.entries.contains_key(id)
    }

    pub fn get(&self, id: &VariableID) -> Option<&S::Row> {
        self.entries.get(id)
    }

    pub fn iter(&self) -> std::collections::btree_map::Iter<'_, VariableID, S::Row> {
        self.entries.iter()
    }

    pub fn keys(&self) -> std::collections::btree_map::Keys<'_, VariableID, S::Row> {
        self.entries.keys()
    }

    pub fn values(&self) -> std::collections::btree_map::Values<'_, VariableID, S::Row> {
        self.entries.values()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn last_key_value(&self) -> Option<(&VariableID, &S::Row)> {
        self.entries.last_key_value()
    }

    fn validate_labels(
        entries: &BTreeMap<VariableID, S::Row>,
        labels: &VariableLabelStore,
    ) -> crate::Result<()> {
        let owned_ids = entries.keys().copied().collect::<BTreeSet<_>>();
        crate::modeling_label::validate_modeling_label_ids(labels, &owned_ids, "decision variable")
    }
}

impl<S: DecisionVariableTableStage> Default for DecisionVariableTable<S> {
    fn default() -> Self {
        Self {
            entries: BTreeMap::default(),
            labels: VariableLabelStore::default(),
            columns: S::Columns::default(),
        }
    }
}

impl DecisionVariableTable<Created> {
    /// Construct a decision-variable definition table with fixed values.
    pub fn with_fixed_values(
        entries: BTreeMap<VariableID, DecisionVariable>,
        labels: VariableLabelStore,
        fixed_values: BTreeMap<VariableID, f64>,
        atol: ATol,
    ) -> crate::Result<Self> {
        Self::with_columns(
            entries,
            labels,
            CreatedDecisionVariableColumns { fixed_values },
            atol,
        )
    }

    /// Fixed decision-variable values keyed by table-owned [`VariableID`].
    pub fn fixed_values(&self) -> &BTreeMap<VariableID, f64> {
        &self.columns.fixed_values
    }

    /// Return the fixed value for one decision variable, if it is fixed.
    pub fn fixed_value(&self, id: VariableID) -> Option<f64> {
        self.columns.fixed_values.get(&id).copied()
    }

    /// Set a fixed value for an existing decision-variable row.
    pub fn set_fixed_value(&mut self, id: VariableID, value: f64, atol: ATol) -> crate::Result<()> {
        let Some(row) = self.entries.get(&id) else {
            crate::bail!(
                { ?id },
                "Fixed decision-variable value references unknown decision variable ID {id:?}",
            );
        };
        row.check_value_consistency(id, value, atol)?;
        self.columns.fixed_values.insert(id, value);
        Ok(())
    }

    /// Add a fixed value unless the row is already fixed consistently.
    pub fn ensure_fixed_value(
        &mut self,
        id: VariableID,
        value: f64,
        atol: ATol,
    ) -> crate::Result<()> {
        let Some(row) = self.entries.get(&id) else {
            crate::bail!(
                { ?id },
                "Fixed decision-variable value references unknown decision variable ID {id:?}",
            );
        };
        row.check_value_consistency(id, value, atol)?;
        if let Some(previous_value) = self.columns.fixed_values.get(&id).copied() {
            if !previous_value.is_finite() || (previous_value - value).abs() > *atol {
                return Err(DecisionVariableError::SubstitutedValueOverwrite {
                    id,
                    previous_value,
                    new_value: value,
                    atol,
                }
                .into());
            }
        } else {
            self.columns.fixed_values.insert(id, value);
        }
        Ok(())
    }

    /// Insert one fresh row, its label, and optionally its fixed value.
    pub fn insert(
        &mut self,
        id: VariableID,
        row: DecisionVariable,
        label: DecisionVariableLabel,
        fixed_value: Option<f64>,
        atol: ATol,
    ) -> Result<(), DecisionVariableError> {
        if self.entries.contains_key(&id) {
            return Err(DecisionVariableError::DuplicateID { id });
        }
        if let Some(value) = fixed_value {
            row.check_value_consistency(id, value, atol)?;
            self.columns.fixed_values.insert(id, value);
        }
        self.insert_labeled_row(id, row, label)
    }

    /// Impose an additional bound on one row while preserving table invariants.
    pub fn clip_bound(&mut self, id: VariableID, bound: Bound, atol: ATol) -> crate::Result<bool> {
        let Some(row) = self.entries.get(&id) else {
            crate::bail!({ ?id }, "Undefined variable ID is used: {id:?}");
        };
        let mut updated = row.clone();
        let changed = updated.clip_bound(id, bound, atol)?;
        if changed {
            if let Some(value) = self.columns.fixed_values.get(&id).copied() {
                updated.check_value_consistency(id, value, atol)?;
            }
            self.entries.insert(id, updated);
        }
        Ok(changed)
    }

    /// Apply additional bounds atomically.
    ///
    /// This uses a plan-and-commit flow: all row updates are computed and
    /// checked first, then only the changed rows are written back. If any
    /// update fails, this table remains unchanged.
    pub fn clip_bounds(&mut self, bounds: &crate::Bounds, atol: ATol) -> crate::Result<()> {
        let mut updates = BTreeMap::new();

        for (id, bound) in bounds {
            let Some(row) = self.entries.get(id) else {
                crate::bail!({ ?id }, "Undefined variable ID is used: {id:?}");
            };
            let mut updated = row.clone();
            let changed = updated.clip_bound(*id, *bound, atol)?;
            if changed {
                if let Some(value) = self.columns.fixed_values.get(id).copied() {
                    updated.check_value_consistency(*id, value, atol)?;
                }
                updates.insert(*id, updated);
            }
        }

        for (id, row) in updates {
            self.entries.insert(id, row);
        }
        Ok(())
    }
}

impl EvaluatedDecisionVariableTable {
    /// Construct an evaluated decision-variable table.
    pub fn new(
        entries: BTreeMap<VariableID, EvaluatedDecisionVariable>,
        labels: VariableLabelStore,
    ) -> crate::Result<Self> {
        Self::with_columns(entries, labels, EvaluatedDecisionVariableColumns {}, ())
    }

    /// Insert one fresh evaluated row and its modeling label.
    pub fn insert(
        &mut self,
        id: VariableID,
        row: EvaluatedDecisionVariable,
        label: DecisionVariableLabel,
    ) -> Result<(), DecisionVariableError> {
        self.insert_labeled_row(id, row, label)
    }
}

impl SampledDecisionVariableTable {
    /// Construct a sampled decision-variable table.
    pub fn new(
        entries: BTreeMap<VariableID, SampledDecisionVariable>,
        labels: VariableLabelStore,
    ) -> crate::Result<Self> {
        Self::with_columns(entries, labels, SampledDecisionVariableColumns {}, ())
    }

    /// Insert one fresh sampled row and its modeling label.
    pub fn insert(
        &mut self,
        id: VariableID,
        row: SampledDecisionVariable,
        label: DecisionVariableLabel,
    ) -> Result<(), DecisionVariableError> {
        self.insert_labeled_row(id, row, label)
    }
}

impl From<&DecisionVariableTable<Created>> for Vec<crate::v1::DecisionVariable> {
    fn from(table: &DecisionVariableTable<Created>) -> Self {
        table
            .entries
            .iter()
            .map(|(id, dv)| {
                let label = table.labels.collect_for(*id);
                let fixed_value = table.columns.fixed_values.get(id).copied();
                decision_variable_to_v1(*id, dv.clone(), label, fixed_value)
            })
            .collect()
    }
}

impl From<&EvaluatedDecisionVariableTable> for Vec<crate::v1::DecisionVariable> {
    fn from(table: &EvaluatedDecisionVariableTable) -> Self {
        table
            .entries
            .iter()
            .map(|(id, dv)| {
                let label = table.labels.collect_for(*id);
                evaluated_decision_variable_to_v1(*id, dv.clone(), label)
            })
            .collect()
    }
}

impl From<&SampledDecisionVariableTable> for Vec<crate::v1::SampledDecisionVariable> {
    fn from(table: &SampledDecisionVariableTable) -> Self {
        table
            .entries
            .iter()
            .map(|(id, dv)| {
                let label = table.labels.collect_for(*id);
                sampled_decision_variable_to_v1(*id, dv.clone(), label)
            })
            .collect()
    }
}

fn decision_variable_to_v1(
    id: VariableID,
    decision_variable: DecisionVariable,
    label: DecisionVariableLabel,
    substituted_value: Option<f64>,
) -> crate::v1::DecisionVariable {
    let DecisionVariable { kind, bound } = decision_variable;
    crate::v1::DecisionVariable {
        id: id.into_inner(),
        kind: kind.into(),
        bound: Some(bound.into()),
        substituted_value,
        name: label.name,
        subscripts: label.subscripts,
        parameters: label.parameters.into_iter().collect(),
        description: label.description,
    }
}

fn evaluated_decision_variable_to_v1(
    id: VariableID,
    evaluated: EvaluatedDecisionVariable,
    label: DecisionVariableLabel,
) -> crate::v1::DecisionVariable {
    let EvaluatedDecisionVariable { kind, bound, value } = evaluated;
    crate::v1::DecisionVariable {
        id: id.into_inner(),
        kind: kind.into(),
        bound: Some(bound.into()),
        substituted_value: Some(value),
        name: label.name,
        subscripts: label.subscripts,
        parameters: label.parameters.into_iter().collect(),
        description: label.description,
    }
}

fn sampled_decision_variable_to_v1(
    id: VariableID,
    sampled: SampledDecisionVariable,
    label: DecisionVariableLabel,
) -> crate::v1::SampledDecisionVariable {
    let SampledDecisionVariable {
        kind,
        bound,
        samples,
    } = sampled;
    crate::v1::SampledDecisionVariable {
        decision_variable: Some(decision_variable_to_v1(
            id,
            DecisionVariable { kind, bound },
            label,
            None,
        )),
        samples: Some(samples.into()),
    }
}

impl From<DecisionVariable> for crate::v2::DecisionVariable {
    fn from(value: DecisionVariable) -> Self {
        let DecisionVariable { kind, bound } = value;
        Self {
            kind: kind.into(),
            bound: Some(bound.into()),
        }
    }
}

impl Parse for crate::v2::DecisionVariable {
    type Output = DecisionVariable;
    type Context = VariableID;

    fn parse(self, id: &Self::Context) -> Result<Self::Output, ParseError> {
        let message = "ommx.v2.DecisionVariable";
        let kind = crate::v1::decision_variable::Kind::try_from(self.kind)
            .map_err(|_| RawParseError::UnknownEnumValue {
                enum_name: "ommx.v1.decision_variable.Kind",
                value: self.kind,
            })
            .map_err(|e| ParseError::from(e).context(message, "kind"))?
            .parse_as(&(), message, "kind")?;
        let bound = self
            .bound
            .ok_or(RawParseError::MissingField {
                message,
                field: "bound",
            })?
            .parse_as(&(), message, "bound")?;
        DecisionVariable::new(kind, bound, ATol::default()).map_err(|source| {
            RawParseError::InvalidDecisionVariable(DecisionVariableError::InvalidDefinition {
                id: *id,
                source: Box::new(source),
            })
            .context(message, "bound")
        })
    }
}

impl From<EvaluatedDecisionVariable> for crate::v2::EvaluatedDecisionVariable {
    fn from(value: EvaluatedDecisionVariable) -> Self {
        let EvaluatedDecisionVariable { kind, bound, value } = value;
        Self {
            kind: kind.into(),
            bound: Some(bound.into()),
            value,
        }
    }
}

impl Parse for crate::v2::EvaluatedDecisionVariable {
    type Output = EvaluatedDecisionVariable;
    type Context = VariableID;

    fn parse(self, id: &Self::Context) -> Result<Self::Output, ParseError> {
        let decision_variable = crate::v2::DecisionVariable {
            kind: self.kind,
            bound: self.bound,
        }
        .parse(id)?;
        EvaluatedDecisionVariable::new(*id, decision_variable, self.value)
            .map_err(RawParseError::InvalidDecisionVariable)
            .map_err(|e| ParseError::from(e).context("ommx.v2.EvaluatedDecisionVariable", "value"))
    }
}

impl From<SampledDecisionVariable> for crate::v2::SampledDecisionVariable {
    fn from(value: SampledDecisionVariable) -> Self {
        let SampledDecisionVariable {
            kind,
            bound,
            samples,
        } = value;
        Self {
            kind: kind.into(),
            bound: Some(bound.into()),
            samples: Some(samples.into()),
        }
    }
}

impl Parse for crate::v2::SampledDecisionVariable {
    type Output = SampledDecisionVariable;
    type Context = VariableID;

    fn parse(self, id: &Self::Context) -> Result<Self::Output, ParseError> {
        let message = "ommx.v2.SampledDecisionVariable";
        let decision_variable = crate::v2::DecisionVariable {
            kind: self.kind,
            bound: self.bound,
        }
        .parse_as(id, message, "decision_variable")?;
        let samples = self
            .samples
            .ok_or(RawParseError::MissingField {
                message,
                field: "samples",
            })?
            .parse_as(&(), message, "samples")?;
        SampledDecisionVariable::new(*id, decision_variable, samples)
            .map_err(RawParseError::InvalidDecisionVariable)
            .map_err(|e| ParseError::from(e).context(message, "samples"))
    }
}

impl From<DecisionVariableTable<Created>> for crate::v2::DecisionVariableTable {
    fn from(table: DecisionVariableTable<Created>) -> Self {
        Self {
            entries: table_entries_to_v2_map(table.entries),
            labels: crate::v2_io::modeling_label_store_to_v2_map(&table.labels),
            fixed_values: table
                .columns
                .fixed_values
                .into_iter()
                .map(|(id, value)| (id.into_inner(), value))
                .collect(),
        }
    }
}

impl Parse for crate::v2::DecisionVariableTable {
    type Output = DecisionVariableTable<Created>;
    type Context = ();

    fn parse(self, _: &Self::Context) -> Result<Self::Output, ParseError> {
        let message = "ommx.v2.DecisionVariableTable";
        let mut entries = BTreeMap::new();
        for (id, row) in self.entries {
            let id = VariableID::from(id);
            entries.insert(id, row.parse_as(&id, message, "entries")?);
        }
        let labels = crate::v2_io::modeling_label_store_from_v2_map(self.labels);
        let fixed_values = self
            .fixed_values
            .into_iter()
            .map(|(id, value)| (VariableID::from(id), value))
            .collect();
        DecisionVariableTable::with_fixed_values(entries, labels, fixed_values, ATol::default())
            .map_err(|e| RawParseError::InvalidInstance(e.to_string()).context(message, "entries"))
    }
}

impl From<EvaluatedDecisionVariableTable> for crate::v2::EvaluatedDecisionVariableTable {
    fn from(table: EvaluatedDecisionVariableTable) -> Self {
        Self {
            entries: table_entries_to_v2_map(table.entries),
            labels: crate::v2_io::modeling_label_store_to_v2_map(&table.labels),
        }
    }
}

impl Parse for crate::v2::EvaluatedDecisionVariableTable {
    type Output = EvaluatedDecisionVariableTable;
    type Context = ();

    fn parse(self, _: &Self::Context) -> Result<Self::Output, ParseError> {
        let message = "ommx.v2.EvaluatedDecisionVariableTable";
        let mut entries = BTreeMap::new();
        for (id, row) in self.entries {
            let id = VariableID::from(id);
            entries.insert(id, row.parse_as(&id, message, "entries")?);
        }
        let labels = crate::v2_io::modeling_label_store_from_v2_map(self.labels);
        EvaluatedDecisionVariableTable::new(entries, labels)
            .map_err(|e| RawParseError::InvalidInstance(e.to_string()).context(message, "entries"))
    }
}

impl From<SampledDecisionVariableTable> for crate::v2::SampledDecisionVariableTable {
    fn from(table: SampledDecisionVariableTable) -> Self {
        Self {
            entries: table_entries_to_v2_map(table.entries),
            labels: crate::v2_io::modeling_label_store_to_v2_map(&table.labels),
        }
    }
}

impl Parse for crate::v2::SampledDecisionVariableTable {
    type Output = SampledDecisionVariableTable;
    type Context = ();

    fn parse(self, _: &Self::Context) -> Result<Self::Output, ParseError> {
        let message = "ommx.v2.SampledDecisionVariableTable";
        let mut entries = BTreeMap::new();
        for (id, row) in self.entries {
            let id = VariableID::from(id);
            entries.insert(id, row.parse_as(&id, message, "entries")?);
        }
        let labels = crate::v2_io::modeling_label_store_from_v2_map(self.labels);
        SampledDecisionVariableTable::new(entries, labels)
            .map_err(|e| RawParseError::InvalidInstance(e.to_string()).context(message, "entries"))
    }
}

fn table_entries_to_v2_map<T, V2>(entries: BTreeMap<VariableID, T>) -> BTreeMap<u64, V2>
where
    T: Into<V2>,
{
    entries
        .into_iter()
        .map(|(id, row)| (id.into_inner(), row.into()))
        .collect()
}

impl<'a, S: DecisionVariableTableStage> IntoIterator for &'a DecisionVariableTable<S> {
    type Item = (&'a VariableID, &'a S::Row);
    type IntoIter = std::collections::btree_map::Iter<'a, VariableID, S::Row>;

    fn into_iter(self) -> Self::IntoIter {
        self.entries.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Bound;

    fn definition_table_without_fixed_values(
        entries: BTreeMap<VariableID, DecisionVariable>,
        labels: VariableLabelStore,
    ) -> crate::Result<DecisionVariableTable> {
        DecisionVariableTable::with_fixed_values(entries, labels, BTreeMap::new(), ATol::default())
    }

    #[test]
    fn default_tables_use_domain_empty_sidecars() {
        let created = DecisionVariableTable::default();
        assert!(created.is_empty());
        assert!(created.fixed_values().is_empty());

        let evaluated = EvaluatedDecisionVariableTable::default();
        assert!(evaluated.is_empty());

        let sampled = SampledDecisionVariableTable::default();
        assert!(sampled.is_empty());
    }

    #[test]
    fn decision_variable_table_rejects_orphan_labels() {
        let id = VariableID::from(1);
        let mut labels = VariableLabelStore::default();
        labels.set_name(id, "x");

        let err = definition_table_without_fixed_values(
            BTreeMap::<VariableID, DecisionVariable>::new(),
            labels,
        )
        .unwrap_err();

        assert!(
            err.to_string().contains("unknown decision variable ID")
                && err.to_string().contains("VariableID(1)"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn definition_table_rejects_orphan_fixed_values() {
        let id = VariableID::from(1);
        let err = DecisionVariableTable::with_fixed_values(
            BTreeMap::new(),
            VariableLabelStore::default(),
            BTreeMap::from([(id, 0.0)]),
            ATol::default(),
        )
        .unwrap_err();

        assert!(
            err.to_string()
                .contains("Fixed decision-variable value references unknown decision variable ID"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn definition_table_with_columns_rejects_orphan_fixed_values() {
        let id = VariableID::from(1);
        let err = DecisionVariableTable::<Created>::with_columns(
            BTreeMap::new(),
            VariableLabelStore::default(),
            CreatedDecisionVariableColumns {
                fixed_values: BTreeMap::from([(id, 0.0)]),
            },
            ATol::default(),
        )
        .unwrap_err();

        assert!(
            err.to_string()
                .contains("Fixed decision-variable value references unknown decision variable ID"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn definition_table_rejects_inconsistent_fixed_values() {
        let id = VariableID::from(1);
        let row = DecisionVariable::new(
            crate::Kind::Integer,
            Bound::new(0.0, 2.0).unwrap(),
            ATol::default(),
        )
        .unwrap();
        let err = DecisionVariableTable::with_fixed_values(
            BTreeMap::from([(id, row)]),
            VariableLabelStore::default(),
            BTreeMap::from([(id, 0.5)]),
            ATol::default(),
        )
        .unwrap_err();

        assert!(
            err.to_string().contains("Substituted value") && err.to_string().contains("ID=1"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn definition_table_preserves_rows_labels_and_fixed_values() {
        let id = VariableID::from(1);
        let row = DecisionVariable::binary();
        let mut labels = VariableLabelStore::default();
        labels.set_name(id, "x");

        let table = DecisionVariableTable::with_fixed_values(
            BTreeMap::from([(id, row.clone())]),
            labels,
            BTreeMap::from([(id, 1.0)]),
            ATol::default(),
        )
        .unwrap();

        assert_eq!(table.get(&id), Some(&row));
        assert_eq!(table.labels().name(id), Some("x"));
        assert_eq!(table.fixed_value(id), Some(1.0));
    }

    #[test]
    fn definition_table_insert_rejects_duplicate_without_replacing_sidecars() {
        let id = VariableID::from(1);
        let original = DecisionVariable::binary();
        let mut table = DecisionVariableTable::with_fixed_values(
            BTreeMap::from([(id, original.clone())]),
            VariableLabelStore::default(),
            BTreeMap::from([(id, 1.0)]),
            ATol::default(),
        )
        .unwrap();

        let err = table
            .insert(
                id,
                DecisionVariable::integer(),
                DecisionVariableLabel {
                    name: Some("new".to_string()),
                    ..Default::default()
                },
                Some(0.0),
                ATol::default(),
            )
            .unwrap_err();

        assert!(err.to_string().contains("Duplicate decision variable ID"));
        assert_eq!(table.get(&id), Some(&original));
        assert_eq!(table.labels().name(id), None);
        assert_eq!(table.fixed_value(id), Some(1.0));
    }

    #[test]
    fn definition_table_to_v1_rows_preserves_labels_and_fixed_values() {
        let id = VariableID::from(1);
        let row = DecisionVariable::binary();
        let mut labels = VariableLabelStore::default();
        labels.set_name(id, "x");
        labels.set_subscripts(id, vec![2, 3]);

        let table = DecisionVariableTable::with_fixed_values(
            BTreeMap::from([(id, row)]),
            labels,
            BTreeMap::from([(id, 1.0)]),
            ATol::default(),
        )
        .unwrap();

        let rows: Vec<crate::v1::DecisionVariable> = (&table).into();

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, 1);
        assert_eq!(rows[0].name.as_deref(), Some("x"));
        assert_eq!(rows[0].subscripts, vec![2, 3]);
        assert_eq!(rows[0].substituted_value, Some(1.0));
    }

    #[test]
    fn definition_table_rejects_inconsistent_fixed_overwrite() {
        let id = VariableID::from(1);
        let mut table = definition_table_without_fixed_values(
            BTreeMap::from([(id, DecisionVariable::continuous())]),
            VariableLabelStore::default(),
        )
        .unwrap();
        table.ensure_fixed_value(id, 1.0, ATol::default()).unwrap();

        let err = table
            .ensure_fixed_value(id, 2.0, ATol::default())
            .unwrap_err();

        assert!(
            err.to_string().contains("cannot be overwritten") && err.to_string().contains("ID=1"),
            "unexpected error: {err}"
        );
        assert_eq!(table.fixed_value(id), Some(1.0));
    }

    #[test]
    fn evaluated_table_rejects_orphan_labels() {
        let id = VariableID::from(1);
        let mut labels = VariableLabelStore::default();
        labels.set_name(id, "x");

        let err = EvaluatedDecisionVariableTable::new(BTreeMap::new(), labels).unwrap_err();

        assert!(
            err.to_string().contains("unknown decision variable ID")
                && err.to_string().contains("VariableID(1)"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn evaluated_table_to_v1_rows_preserves_labels_and_values() {
        let id = VariableID::from(1);
        let row = EvaluatedDecisionVariable::new(id, DecisionVariable::continuous(), 2.5).unwrap();
        let mut labels = VariableLabelStore::default();
        labels.set_name(id, "x");

        let table =
            EvaluatedDecisionVariableTable::new(BTreeMap::from([(id, row)]), labels).unwrap();

        let rows: Vec<crate::v1::DecisionVariable> = (&table).into();

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, 1);
        assert_eq!(rows[0].name.as_deref(), Some("x"));
        assert_eq!(rows[0].substituted_value, Some(2.5));
    }

    #[test]
    fn sampled_table_rejects_orphan_labels() {
        let id = VariableID::from(1);
        let mut labels = VariableLabelStore::default();
        labels.set_name(id, "x");

        let err = SampledDecisionVariableTable::new(BTreeMap::new(), labels).unwrap_err();

        assert!(
            err.to_string().contains("unknown decision variable ID")
                && err.to_string().contains("VariableID(1)"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn sampled_table_to_v1_rows_preserves_labels_and_samples() {
        let id = VariableID::from(1);
        let row = SampledDecisionVariable::new(
            id,
            DecisionVariable::continuous(),
            crate::Sampled::from((crate::SampleID::from(7), 2.5)),
        )
        .unwrap();
        let mut labels = VariableLabelStore::default();
        labels.set_name(id, "x");

        let table = SampledDecisionVariableTable::new(BTreeMap::from([(id, row)]), labels).unwrap();

        let rows: Vec<crate::v1::SampledDecisionVariable> = (&table).into();

        assert_eq!(rows.len(), 1);
        let variable = rows[0].decision_variable.as_ref().unwrap();
        assert_eq!(variable.id, 1);
        assert_eq!(variable.name.as_deref(), Some("x"));
        assert!(rows[0].samples.is_some());
    }
}
