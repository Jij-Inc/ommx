use std::collections::{BTreeMap, BTreeSet};

use crate::logical_memory::{LogicalMemoryProfile, LogicalMemoryVisitor, Path};
use crate::{ATol, Bound, ModelingLabel};

use super::{
    DecisionVariable, DecisionVariableError, DecisionVariableLabel, VariableID, VariableLabelStore,
};

/// Owner of decision-variable rows and their modeling labels.
///
/// The table key owns [`VariableID`], the row value owns intrinsic data
/// (`DecisionVariable`, `EvaluatedDecisionVariable`, or
/// `SampledDecisionVariable`), and [`VariableLabelStore`] owns `name`,
/// `subscripts`, `parameters`, and `description`.
///
/// # Table-level invariants
///
/// - Every modeling-label ID is owned by this table; labels for unknown
///   [`VariableID`] values are rejected by [`Self::new`] and
///   [`Self::set_label`].
/// - Public mutation preserves the row/label ownership boundary. Rows can be
///   inserted or replaced only together with the corresponding label via
///   [`Self::insert`]; mutable row iteration is not exposed.
///
/// # Host-level invariants
///
/// This table intentionally does not validate facts that require a surrounding
/// top-level object. For example, whether created decision variables are
/// referenced by objective or constraint functions, whether sampled rows all
/// use the same sample-ID universe, or whether evaluated/sampled constraints
/// reference known variables is validated by host builders such as
/// [`crate::Instance::builder`], [`crate::Solution::builder`], and
/// [`crate::SampleSet::builder`].
#[derive(Debug, Clone, PartialEq)]
pub struct DecisionVariableTable<T> {
    entries: BTreeMap<VariableID, T>,
    labels: VariableLabelStore,
}

impl<T> Default for DecisionVariableTable<T> {
    fn default() -> Self {
        Self {
            entries: BTreeMap::default(),
            labels: VariableLabelStore::default(),
        }
    }
}

impl<T> DecisionVariableTable<T> {
    /// Construct a decision-variable table, rejecting labels for unknown IDs.
    pub fn new(
        entries: BTreeMap<VariableID, T>,
        labels: VariableLabelStore,
    ) -> crate::Result<Self> {
        let owned_ids = entries.keys().copied().collect::<BTreeSet<_>>();
        crate::modeling_label::validate_modeling_label_ids(
            &labels,
            &owned_ids,
            "decision variable",
        )?;
        Ok(Self { entries, labels })
    }

    /// Construct a table with no labels.
    pub fn from_entries(entries: BTreeMap<VariableID, T>) -> Self {
        Self {
            entries,
            labels: VariableLabelStore::default(),
        }
    }

    /// Split the table into its row map and label store.
    ///
    /// Use this at serialization or conversion boundaries that must join
    /// labels back onto row payloads. Iterating by value is intentionally not
    /// provided, so consuming code cannot silently drop labels.
    pub fn into_parts(self) -> (BTreeMap<VariableID, T>, VariableLabelStore) {
        (self.entries, self.labels)
    }

    /// Intrinsic row map, keyed by table-owned [`VariableID`].
    pub fn entries(&self) -> &BTreeMap<VariableID, T> {
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

    /// Insert or replace one row and its modeling label.
    pub fn insert(&mut self, id: VariableID, row: T, label: ModelingLabel) -> Option<T> {
        self.labels.insert(id, label);
        self.entries.insert(id, row)
    }

    pub fn contains_key(&self, id: &VariableID) -> bool {
        self.entries.contains_key(id)
    }

    pub fn get(&self, id: &VariableID) -> Option<&T> {
        self.entries.get(id)
    }

    pub fn iter(&self) -> std::collections::btree_map::Iter<'_, VariableID, T> {
        self.entries.iter()
    }

    pub fn keys(&self) -> std::collections::btree_map::Keys<'_, VariableID, T> {
        self.entries.keys()
    }

    pub fn values(&self) -> std::collections::btree_map::Values<'_, VariableID, T> {
        self.entries.values()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn last_key_value(&self) -> Option<(&VariableID, &T)> {
        self.entries.last_key_value()
    }
}

impl<T: LogicalMemoryProfile> LogicalMemoryProfile for DecisionVariableTable<T> {
    fn visit_logical_memory<V: LogicalMemoryVisitor>(&self, path: &mut Path, visitor: &mut V) {
        self.entries
            .visit_logical_memory(path.with("entries").as_mut(), visitor);
        self.labels
            .visit_logical_memory(path.with("labels").as_mut(), visitor);
    }
}

impl<'a, T> IntoIterator for &'a DecisionVariableTable<T> {
    type Item = (&'a VariableID, &'a T);
    type IntoIter = std::collections::btree_map::Iter<'a, VariableID, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.entries.iter()
    }
}

/// Owner of created decision-variable rows, modeling labels, and fixed values.
///
/// Created-stage instances carry an additional fixed-value column. The table
/// owns the fact that fixed-value IDs must belong to the row table and that
/// each fixed value is finite and consistent with the row's kind/bound.
///
/// # Table-level invariants
///
/// - All [`DecisionVariableTable`] invariants hold.
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
pub struct CreatedDecisionVariableTable {
    table: DecisionVariableTable<DecisionVariable>,
    fixed_values: BTreeMap<VariableID, f64>,
}

impl CreatedDecisionVariableTable {
    /// Construct a created decision-variable table.
    pub fn new(
        entries: BTreeMap<VariableID, DecisionVariable>,
        labels: VariableLabelStore,
        fixed_values: BTreeMap<VariableID, f64>,
        atol: ATol,
    ) -> crate::Result<Self> {
        let table = DecisionVariableTable::new(entries, labels)?;
        Self::validate_fixed_values(table.entries(), &fixed_values, atol)?;
        Ok(Self {
            table,
            fixed_values,
        })
    }

    /// Construct a created decision-variable table with no labels or fixed
    /// values.
    pub fn from_entries(entries: BTreeMap<VariableID, DecisionVariable>) -> Self {
        Self {
            table: DecisionVariableTable::from_entries(entries),
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
        let (entries, labels) = self.table.into_parts();
        (entries, labels, self.fixed_values)
    }

    /// Intrinsic row table plus modeling labels.
    pub fn table(&self) -> &DecisionVariableTable<DecisionVariable> {
        &self.table
    }

    /// Intrinsic row map, keyed by table-owned [`VariableID`].
    pub fn entries(&self) -> &BTreeMap<VariableID, DecisionVariable> {
        self.table.entries()
    }

    /// Per-row modeling label store.
    pub fn labels(&self) -> &VariableLabelStore {
        self.table.labels()
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
        self.table.set_label(id, label)
    }

    /// Set a fixed value for an existing decision-variable row.
    pub fn set_fixed_value(&mut self, id: VariableID, value: f64, atol: ATol) -> crate::Result<()> {
        let Some(row) = self.table.get(&id) else {
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
        let Some(row) = self.table.get(&id) else {
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
        Ok(self.table.insert(id, row, label))
    }

    /// Impose an additional bound on one row while preserving table invariants.
    pub fn clip_bound(&mut self, id: VariableID, bound: Bound, atol: ATol) -> crate::Result<bool> {
        let Some(row) = self.table.entries.get(&id) else {
            crate::bail!({ ?id }, "Undefined variable ID is used: {id:?}");
        };
        let mut updated = row.clone();
        let changed = updated.clip_bound(id, bound, atol)?;
        if changed {
            if let Some(value) = self.fixed_values.get(&id).copied() {
                updated.check_value_consistency(id, value, atol)?;
            }
            self.table.entries.insert(id, updated);
        }
        Ok(changed)
    }

    pub fn contains_key(&self, id: &VariableID) -> bool {
        self.table.contains_key(id)
    }

    pub fn get(&self, id: &VariableID) -> Option<&DecisionVariable> {
        self.table.get(id)
    }

    pub fn iter(&self) -> std::collections::btree_map::Iter<'_, VariableID, DecisionVariable> {
        self.table.iter()
    }

    pub fn keys(&self) -> std::collections::btree_map::Keys<'_, VariableID, DecisionVariable> {
        self.table.keys()
    }

    pub fn values(&self) -> std::collections::btree_map::Values<'_, VariableID, DecisionVariable> {
        self.table.values()
    }

    pub fn len(&self) -> usize {
        self.table.len()
    }

    pub fn is_empty(&self) -> bool {
        self.table.is_empty()
    }

    pub fn last_key_value(&self) -> Option<(&VariableID, &DecisionVariable)> {
        self.table.last_key_value()
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

impl LogicalMemoryProfile for CreatedDecisionVariableTable {
    fn visit_logical_memory<V: LogicalMemoryVisitor>(&self, path: &mut Path, visitor: &mut V) {
        self.table
            .visit_logical_memory(path.with("table").as_mut(), visitor);
        self.fixed_values
            .visit_logical_memory(path.with("fixed_values").as_mut(), visitor);
    }
}

impl<'a> IntoIterator for &'a CreatedDecisionVariableTable {
    type Item = (&'a VariableID, &'a DecisionVariable);
    type IntoIter = std::collections::btree_map::Iter<'a, VariableID, DecisionVariable>;

    fn into_iter(self) -> Self::IntoIter {
        self.table.iter()
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
            DecisionVariableTable::<DecisionVariable>::new(BTreeMap::new(), labels).unwrap_err();

        assert!(
            err.to_string().contains("unknown decision variable ID")
                && err.to_string().contains("VariableID(1)"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn created_table_rejects_orphan_fixed_values() {
        let id = VariableID::from(1);
        let err = CreatedDecisionVariableTable::new(
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
    fn created_table_rejects_inconsistent_fixed_values() {
        let id = VariableID::from(1);
        let row = DecisionVariable::new(
            crate::Kind::Integer,
            Bound::new(0.0, 2.0).unwrap(),
            ATol::default(),
        )
        .unwrap();
        let err = CreatedDecisionVariableTable::new(
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
    fn created_table_preserves_rows_labels_and_fixed_values() {
        let id = VariableID::from(1);
        let row = DecisionVariable::binary();
        let mut labels = VariableLabelStore::default();
        labels.set_name(id, "x");

        let table = CreatedDecisionVariableTable::new(
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
    fn created_table_rejects_inconsistent_fixed_overwrite() {
        let id = VariableID::from(1);
        let mut table = CreatedDecisionVariableTable::from_entries(BTreeMap::from([(
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
}
