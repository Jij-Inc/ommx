use std::collections::{BTreeMap, BTreeSet};

use crate::logical_memory::{LogicalMemoryProfile, LogicalMemoryVisitor, Path};
use crate::{ATol, Bound, ModelingLabel};

use super::{
    DecisionVariable, DecisionVariableError, DecisionVariableLabel, EvaluatedDecisionVariable,
    SampledDecisionVariable, VariableID, VariableLabelStore,
};

#[derive(Debug, Clone, PartialEq)]
struct LabeledVariableRows<T> {
    entries: BTreeMap<VariableID, T>,
    labels: VariableLabelStore,
}

impl<T> Default for LabeledVariableRows<T> {
    fn default() -> Self {
        Self {
            entries: BTreeMap::default(),
            labels: VariableLabelStore::default(),
        }
    }
}

impl<T> LabeledVariableRows<T> {
    fn new(entries: BTreeMap<VariableID, T>, labels: VariableLabelStore) -> crate::Result<Self> {
        let owned_ids = entries.keys().copied().collect::<BTreeSet<_>>();
        crate::modeling_label::validate_modeling_label_ids(
            &labels,
            &owned_ids,
            "decision variable",
        )?;
        Ok(Self { entries, labels })
    }

    fn from_entries(entries: BTreeMap<VariableID, T>) -> Self {
        Self {
            entries,
            labels: VariableLabelStore::default(),
        }
    }

    fn into_parts(self) -> (BTreeMap<VariableID, T>, VariableLabelStore) {
        (self.entries, self.labels)
    }

    fn entries(&self) -> &BTreeMap<VariableID, T> {
        &self.entries
    }

    fn labels(&self) -> &VariableLabelStore {
        &self.labels
    }

    fn set_label(&mut self, id: VariableID, label: DecisionVariableLabel) -> crate::Result<()> {
        if !self.entries.contains_key(&id) {
            crate::bail!(
                { ?id },
                "Modeling label references unknown decision variable ID {id:?}",
            );
        }
        self.labels.insert(id, label);
        Ok(())
    }

    fn insert(&mut self, id: VariableID, row: T, label: ModelingLabel) -> Option<T> {
        self.labels.insert(id, label);
        self.entries.insert(id, row)
    }

    fn contains_key(&self, id: &VariableID) -> bool {
        self.entries.contains_key(id)
    }

    fn get(&self, id: &VariableID) -> Option<&T> {
        self.entries.get(id)
    }

    fn iter(&self) -> std::collections::btree_map::Iter<'_, VariableID, T> {
        self.entries.iter()
    }

    fn keys(&self) -> std::collections::btree_map::Keys<'_, VariableID, T> {
        self.entries.keys()
    }

    fn values(&self) -> std::collections::btree_map::Values<'_, VariableID, T> {
        self.entries.values()
    }

    fn len(&self) -> usize {
        self.entries.len()
    }

    fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn last_key_value(&self) -> Option<(&VariableID, &T)> {
        self.entries.last_key_value()
    }
}

impl<T: LogicalMemoryProfile> LogicalMemoryProfile for LabeledVariableRows<T> {
    fn visit_logical_memory<V: LogicalMemoryVisitor>(&self, path: &mut Path, visitor: &mut V) {
        self.entries
            .visit_logical_memory(path.with("entries").as_mut(), visitor);
        self.labels
            .visit_logical_memory(path.with("labels").as_mut(), visitor);
    }
}

impl<'a, T> IntoIterator for &'a LabeledVariableRows<T> {
    type Item = (&'a VariableID, &'a T);
    type IntoIter = std::collections::btree_map::Iter<'a, VariableID, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.entries.iter()
    }
}

/// Owner of decision-variable definitions, modeling labels, and fixed values.
///
/// [`Instance`](crate::Instance) and [`ParametricInstance`](crate::ParametricInstance)
/// use this table for model variable declarations. The table key owns
/// [`VariableID`], row values own intrinsic [`DecisionVariable`] definition
/// data (`kind` and `bound`), [`VariableLabelStore`] owns modeling labels, and
/// `fixed_values` is a sparse optional column for variables fixed before
/// solving.
///
/// # Table-level invariants
///
/// - Every modeling-label ID is owned by this table.
/// - Every fixed-value ID is owned by this table.
/// - Fixed values are finite and satisfy the corresponding row's kind/bound
///   under the [`ATol`] supplied to [`Self::new`] or [`Self::set_fixed_value`].
///
/// # Host-level invariants
///
/// This table does not classify variables into used, fixed, dependent, or
/// parameter roles. The enclosing [`crate::Instance`] or
/// [`crate::ParametricInstance`] validates role disjointness, expression
/// references, and the shared decision-variable / parameter ID namespace.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DecisionVariableTable {
    rows: LabeledVariableRows<DecisionVariable>,
    fixed_values: BTreeMap<VariableID, f64>,
}

impl DecisionVariableTable {
    /// Construct a decision-variable definition table.
    pub fn new(
        entries: BTreeMap<VariableID, DecisionVariable>,
        labels: VariableLabelStore,
        fixed_values: BTreeMap<VariableID, f64>,
        atol: ATol,
    ) -> crate::Result<Self> {
        let rows = LabeledVariableRows::new(entries, labels)?;
        Self::validate_fixed_values(rows.entries(), &fixed_values, atol)?;
        Ok(Self { rows, fixed_values })
    }

    /// Construct a table with no labels or fixed values.
    pub fn from_entries(entries: BTreeMap<VariableID, DecisionVariable>) -> Self {
        Self {
            rows: LabeledVariableRows::from_entries(entries),
            fixed_values: BTreeMap::default(),
        }
    }

    /// Split the table into row map, label store, and fixed-value column.
    pub fn into_parts(
        self,
    ) -> (
        BTreeMap<VariableID, DecisionVariable>,
        VariableLabelStore,
        BTreeMap<VariableID, f64>,
    ) {
        let (entries, labels) = self.rows.into_parts();
        (entries, labels, self.fixed_values)
    }

    /// Intrinsic row map, keyed by table-owned [`VariableID`].
    pub fn entries(&self) -> &BTreeMap<VariableID, DecisionVariable> {
        self.rows.entries()
    }

    /// Per-row modeling label store.
    pub fn labels(&self) -> &VariableLabelStore {
        self.rows.labels()
    }

    /// Fixed decision-variable values keyed by table-owned [`VariableID`].
    pub fn fixed_values(&self) -> &BTreeMap<VariableID, f64> {
        &self.fixed_values
    }

    /// Return the fixed value for one decision variable, if it is fixed.
    pub fn fixed_value(&self, id: VariableID) -> Option<f64> {
        self.fixed_values.get(&id).copied()
    }

    /// Replace the modeling label for an existing decision-variable row.
    pub fn set_label(&mut self, id: VariableID, label: DecisionVariableLabel) -> crate::Result<()> {
        self.rows.set_label(id, label)
    }

    /// Set a fixed value for an existing decision-variable row.
    pub fn set_fixed_value(&mut self, id: VariableID, value: f64, atol: ATol) -> crate::Result<()> {
        let Some(row) = self.rows.get(&id) else {
            crate::bail!(
                { ?id },
                "Fixed decision-variable value references unknown decision variable ID {id:?}",
            );
        };
        row.check_value_consistency(id, value, atol)?;
        self.fixed_values.insert(id, value);
        Ok(())
    }

    /// Add a fixed value unless the row is already fixed consistently.
    pub fn ensure_fixed_value(
        &mut self,
        id: VariableID,
        value: f64,
        atol: ATol,
    ) -> crate::Result<()> {
        let Some(row) = self.rows.get(&id) else {
            crate::bail!(
                { ?id },
                "Fixed decision-variable value references unknown decision variable ID {id:?}",
            );
        };
        row.check_value_consistency(id, value, atol)?;
        if let Some(previous_value) = self.fixed_values.get(&id).copied() {
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
            self.fixed_values.insert(id, value);
        }
        Ok(())
    }

    /// Insert or replace one row, its label, and optionally its fixed value.
    pub fn insert(
        &mut self,
        id: VariableID,
        row: DecisionVariable,
        label: DecisionVariableLabel,
        fixed_value: Option<f64>,
        atol: ATol,
    ) -> Result<Option<DecisionVariable>, DecisionVariableError> {
        if let Some(value) = fixed_value {
            row.check_value_consistency(id, value, atol)?;
        }
        match fixed_value {
            Some(value) => {
                self.fixed_values.insert(id, value);
            }
            None => {
                self.fixed_values.remove(&id);
            }
        }
        Ok(self.rows.insert(id, row, label))
    }

    /// Impose an additional bound on one row while preserving table invariants.
    pub fn clip_bound(&mut self, id: VariableID, bound: Bound, atol: ATol) -> crate::Result<bool> {
        let Some(row) = self.rows.entries.get(&id) else {
            crate::bail!({ ?id }, "Undefined variable ID is used: {id:?}");
        };
        let mut updated = row.clone();
        let changed = updated.clip_bound(id, bound, atol)?;
        if changed {
            if let Some(value) = self.fixed_values.get(&id).copied() {
                updated.check_value_consistency(id, value, atol)?;
            }
            self.rows.entries.insert(id, updated);
        }
        Ok(changed)
    }

    pub fn contains_key(&self, id: &VariableID) -> bool {
        self.rows.contains_key(id)
    }

    pub fn get(&self, id: &VariableID) -> Option<&DecisionVariable> {
        self.rows.get(id)
    }

    pub fn iter(&self) -> std::collections::btree_map::Iter<'_, VariableID, DecisionVariable> {
        self.rows.iter()
    }

    pub fn keys(&self) -> std::collections::btree_map::Keys<'_, VariableID, DecisionVariable> {
        self.rows.keys()
    }

    pub fn values(&self) -> std::collections::btree_map::Values<'_, VariableID, DecisionVariable> {
        self.rows.values()
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    pub fn last_key_value(&self) -> Option<(&VariableID, &DecisionVariable)> {
        self.rows.last_key_value()
    }

    fn validate_fixed_values(
        entries: &BTreeMap<VariableID, DecisionVariable>,
        fixed_values: &BTreeMap<VariableID, f64>,
        atol: ATol,
    ) -> crate::Result<()> {
        for (id, value) in fixed_values {
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

impl LogicalMemoryProfile for DecisionVariableTable {
    fn visit_logical_memory<V: LogicalMemoryVisitor>(&self, path: &mut Path, visitor: &mut V) {
        self.rows
            .entries
            .visit_logical_memory(path.with("entries").as_mut(), visitor);
        self.rows
            .labels
            .visit_logical_memory(path.with("labels").as_mut(), visitor);
        self.fixed_values
            .visit_logical_memory(path.with("fixed_values").as_mut(), visitor);
    }
}

impl<'a> IntoIterator for &'a DecisionVariableTable {
    type Item = (&'a VariableID, &'a DecisionVariable);
    type IntoIter = std::collections::btree_map::Iter<'a, VariableID, DecisionVariable>;

    fn into_iter(self) -> Self::IntoIter {
        self.rows.iter()
    }
}

/// Owner of evaluated decision-variable rows and their modeling labels.
///
/// [`Solution`](crate::Solution) uses this table for variable values at one
/// state. The table key owns [`VariableID`]; row values own evaluated
/// `kind`/`bound`/`value` payloads. Host-level invariants, such as whether
/// evaluated constraints reference known variables, remain owned by
/// [`Solution`](crate::Solution).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct EvaluatedDecisionVariableTable {
    rows: LabeledVariableRows<EvaluatedDecisionVariable>,
}

impl EvaluatedDecisionVariableTable {
    /// Construct an evaluated decision-variable table, rejecting labels for
    /// unknown IDs.
    pub fn new(
        entries: BTreeMap<VariableID, EvaluatedDecisionVariable>,
        labels: VariableLabelStore,
    ) -> crate::Result<Self> {
        Ok(Self {
            rows: LabeledVariableRows::new(entries, labels)?,
        })
    }

    /// Construct a table with no labels.
    pub fn from_entries(entries: BTreeMap<VariableID, EvaluatedDecisionVariable>) -> Self {
        Self {
            rows: LabeledVariableRows::from_entries(entries),
        }
    }

    /// Split the table into its row map and label store.
    pub fn into_parts(
        self,
    ) -> (
        BTreeMap<VariableID, EvaluatedDecisionVariable>,
        VariableLabelStore,
    ) {
        self.rows.into_parts()
    }

    /// Evaluated row map, keyed by table-owned [`VariableID`].
    pub fn entries(&self) -> &BTreeMap<VariableID, EvaluatedDecisionVariable> {
        self.rows.entries()
    }

    /// Per-row modeling label store.
    pub fn labels(&self) -> &VariableLabelStore {
        self.rows.labels()
    }

    /// Replace the modeling label for an existing evaluated row.
    pub fn set_label(&mut self, id: VariableID, label: DecisionVariableLabel) -> crate::Result<()> {
        self.rows.set_label(id, label)
    }

    /// Insert or replace one evaluated row and its modeling label.
    pub fn insert(
        &mut self,
        id: VariableID,
        row: EvaluatedDecisionVariable,
        label: DecisionVariableLabel,
    ) -> Option<EvaluatedDecisionVariable> {
        self.rows.insert(id, row, label)
    }

    pub fn contains_key(&self, id: &VariableID) -> bool {
        self.rows.contains_key(id)
    }

    pub fn get(&self, id: &VariableID) -> Option<&EvaluatedDecisionVariable> {
        self.rows.get(id)
    }

    pub fn iter(
        &self,
    ) -> std::collections::btree_map::Iter<'_, VariableID, EvaluatedDecisionVariable> {
        self.rows.iter()
    }

    pub fn keys(
        &self,
    ) -> std::collections::btree_map::Keys<'_, VariableID, EvaluatedDecisionVariable> {
        self.rows.keys()
    }

    pub fn values(
        &self,
    ) -> std::collections::btree_map::Values<'_, VariableID, EvaluatedDecisionVariable> {
        self.rows.values()
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    pub fn last_key_value(&self) -> Option<(&VariableID, &EvaluatedDecisionVariable)> {
        self.rows.last_key_value()
    }
}

impl<'a> IntoIterator for &'a EvaluatedDecisionVariableTable {
    type Item = (&'a VariableID, &'a EvaluatedDecisionVariable);
    type IntoIter = std::collections::btree_map::Iter<'a, VariableID, EvaluatedDecisionVariable>;

    fn into_iter(self) -> Self::IntoIter {
        self.rows.iter()
    }
}

/// Owner of sampled decision-variable rows and their modeling labels.
///
/// [`SampleSet`](crate::SampleSet) uses this table for per-sample variable
/// values. The table key owns [`VariableID`]; row values own sampled
/// `kind`/`bound`/`samples` payloads. The enclosing
/// [`SampleSet`](crate::SampleSet) owns cross-table sample-ID consistency.
#[derive(Debug, Clone, Default)]
pub struct SampledDecisionVariableTable {
    rows: LabeledVariableRows<SampledDecisionVariable>,
}

impl SampledDecisionVariableTable {
    /// Construct a sampled decision-variable table, rejecting labels for
    /// unknown IDs.
    pub fn new(
        entries: BTreeMap<VariableID, SampledDecisionVariable>,
        labels: VariableLabelStore,
    ) -> crate::Result<Self> {
        Ok(Self {
            rows: LabeledVariableRows::new(entries, labels)?,
        })
    }

    /// Construct a table with no labels.
    pub fn from_entries(entries: BTreeMap<VariableID, SampledDecisionVariable>) -> Self {
        Self {
            rows: LabeledVariableRows::from_entries(entries),
        }
    }

    /// Split the table into its row map and label store.
    pub fn into_parts(
        self,
    ) -> (
        BTreeMap<VariableID, SampledDecisionVariable>,
        VariableLabelStore,
    ) {
        self.rows.into_parts()
    }

    /// Sampled row map, keyed by table-owned [`VariableID`].
    pub fn entries(&self) -> &BTreeMap<VariableID, SampledDecisionVariable> {
        self.rows.entries()
    }

    /// Per-row modeling label store.
    pub fn labels(&self) -> &VariableLabelStore {
        self.rows.labels()
    }

    /// Replace the modeling label for an existing sampled row.
    pub fn set_label(&mut self, id: VariableID, label: DecisionVariableLabel) -> crate::Result<()> {
        self.rows.set_label(id, label)
    }

    /// Insert or replace one sampled row and its modeling label.
    pub fn insert(
        &mut self,
        id: VariableID,
        row: SampledDecisionVariable,
        label: DecisionVariableLabel,
    ) -> Option<SampledDecisionVariable> {
        self.rows.insert(id, row, label)
    }

    pub fn contains_key(&self, id: &VariableID) -> bool {
        self.rows.contains_key(id)
    }

    pub fn get(&self, id: &VariableID) -> Option<&SampledDecisionVariable> {
        self.rows.get(id)
    }

    pub fn iter(
        &self,
    ) -> std::collections::btree_map::Iter<'_, VariableID, SampledDecisionVariable> {
        self.rows.iter()
    }

    pub fn keys(
        &self,
    ) -> std::collections::btree_map::Keys<'_, VariableID, SampledDecisionVariable> {
        self.rows.keys()
    }

    pub fn values(
        &self,
    ) -> std::collections::btree_map::Values<'_, VariableID, SampledDecisionVariable> {
        self.rows.values()
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    pub fn last_key_value(&self) -> Option<(&VariableID, &SampledDecisionVariable)> {
        self.rows.last_key_value()
    }
}

impl<'a> IntoIterator for &'a SampledDecisionVariableTable {
    type Item = (&'a VariableID, &'a SampledDecisionVariable);
    type IntoIter = std::collections::btree_map::Iter<'a, VariableID, SampledDecisionVariable>;

    fn into_iter(self) -> Self::IntoIter {
        self.rows.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Bound;

    #[test]
    fn decision_variable_table_rejects_orphan_labels() {
        let id = VariableID::from(1);
        let mut labels = VariableLabelStore::default();
        labels.set_name(id, "x");

        let err =
            DecisionVariableTable::new(BTreeMap::new(), labels, BTreeMap::new(), ATol::default())
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
        let err = DecisionVariableTable::new(
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
    fn definition_table_rejects_inconsistent_fixed_values() {
        let id = VariableID::from(1);
        let row = DecisionVariable::new(
            crate::Kind::Integer,
            Bound::new(0.0, 2.0).unwrap(),
            ATol::default(),
        )
        .unwrap();
        let err = DecisionVariableTable::new(
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

        let table = DecisionVariableTable::new(
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
    fn definition_table_rejects_inconsistent_fixed_overwrite() {
        let id = VariableID::from(1);
        let mut table = DecisionVariableTable::from_entries(BTreeMap::from([(
            id,
            DecisionVariable::continuous(),
        )]));
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
}
