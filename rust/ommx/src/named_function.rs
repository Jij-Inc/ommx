mod arbitrary;
mod evaluate;
mod label_store;
pub(crate) mod parse;
mod substitute;

use derive_more::{Deref, From};
use getset::*;
use std::collections::{BTreeMap, BTreeSet};

use crate::logical_memory::LogicalMemoryProfile;
use crate::{Function, SampleID, Sampled, VariableIDSet};
pub use arbitrary::*;
pub use label_store::NamedFunctionLabelStore;

/// ID for named function
#[derive(
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    From,
    Deref,
    serde::Serialize,
    serde::Deserialize,
    LogicalMemoryProfile,
)]
#[serde(transparent)]
pub struct NamedFunctionID(u64);

impl std::fmt::Debug for NamedFunctionID {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "NamedFunctionID({})", self.0)
    }
}

impl std::fmt::Display for NamedFunctionID {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl NamedFunctionID {
    pub fn into_inner(self) -> u64 {
        self.0
    }
}

impl From<NamedFunctionID> for u64 {
    fn from(id: NamedFunctionID) -> Self {
        id.0
    }
}

/// A named function represents an arbitrary mathematical function with an associated modeling label.
///
/// Named functions allow attaching names, subscripts, parameters, and descriptions to
/// mathematical expressions. This is useful for tracking auxiliary quantities, objectives,
/// or derived values in optimization problems.
///
/// # Examples
///
/// A series of named functions `x[i, j] + y[i, j]` for `i = 1, 2, 3` and `j = 4, 5`
/// would create 6 `NamedFunction` instances, each with:
/// - `name`: A human-readable identifier (e.g., "f")
/// - `subscripts`: The index values (e.g., `[1, 5]` for `f[1, 5]`)
///
/// Named function IDs are managed by the enclosing named-function table key,
/// separately from decision variable IDs and constraint IDs, so the same ID
/// value can be used across these different namespaces.
///
/// The modeling label (`name`, `subscripts`, `parameters`, `description`) is stored in a
/// per-collection [`NamedFunctionLabelStore`] keyed by [`NamedFunctionID`];
/// the per-element struct no longer carries it or the ID.
///
/// Corresponds to `ommx.v1.NamedFunction`, but the legacy protobuf inline `id`
/// is drained into / filled from the enclosing map key at the parse/serialize
/// boundary.
#[derive(Debug, Clone, PartialEq, LogicalMemoryProfile)]
pub struct NamedFunction {
    pub function: Function,
}

impl std::fmt::Display for NamedFunction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "NamedFunction({})", self.function)
    }
}

/// Modeling label for named functions.
pub type NamedFunctionLabel = crate::ModelingLabel;

/// Owner of named-function rows and their modeling labels.
///
/// The table key owns [`NamedFunctionID`], the row value owns intrinsic data
/// (`NamedFunction`, `EvaluatedNamedFunction`, or `SampledNamedFunction`), and
/// [`NamedFunctionLabelStore`] owns `name`, `subscripts`, `parameters`, and
/// `description`. This mirrors `ConstraintCollection` for named functions,
/// without active/removed state.
///
/// Mathematically, this table is the named-expression component
/// `N = {named_function_id -> expression row}` of an enclosing root object.
/// The table owns row identity and labels; the root object owns the meaning of
/// the expression row in its variable, parameter, solution, or sample space.
///
/// # Invariants
///
/// - Every modeling-label ID is owned by this table; labels for unknown
///   [`NamedFunctionID`] values are rejected by [`Self::new`] and
///   [`Self::set_label`].
/// - Public mutation preserves the row/label ownership boundary. Rows can be
///   freshly inserted only together with the corresponding label via
///   [`Self::insert`]; mutable row iteration is not exposed.
///
/// # Host-level invariants
///
/// This table intentionally does not validate row semantics that require a
/// surrounding top-level object. For example, whether a created
/// [`NamedFunction`] references defined decision variables, or whether an
/// evaluated/sampled named function's `used_decision_variable_ids` exist in the
/// evaluated/sampled decision-variable table, is validated by host builders such
/// as [`crate::Instance::builder`], [`crate::Solution::builder`], and
/// [`crate::SampleSet::builder`].
///
/// # Table-local operations
///
/// The table supports construction from rows and labels, read access, fresh
/// insertion of a row with its label, label updates for existing rows, and
/// consuming rows and labels at serialization or conversion boundaries.
/// Duplicate insertion is rejected rather than interpreted as replacement.
///
/// Expression substitution, evaluation, and sampling are root-object
/// operations. They may induce host-validated row replacement, but the table
/// does not expose raw mutable access that would let callers edit expression
/// rows without the enclosing root operation.
#[derive(Debug, Clone, PartialEq, LogicalMemoryProfile)]
pub struct NamedFunctionTable<T> {
    entries: BTreeMap<NamedFunctionID, T>,
    labels: NamedFunctionLabelStore,
}

impl<T> Default for NamedFunctionTable<T> {
    fn default() -> Self {
        Self {
            entries: BTreeMap::default(),
            labels: NamedFunctionLabelStore::default(),
        }
    }
}

impl<T> NamedFunctionTable<T> {
    /// Construct a named-function table, rejecting labels for unknown IDs.
    pub fn new(
        entries: BTreeMap<NamedFunctionID, T>,
        labels: NamedFunctionLabelStore,
    ) -> crate::Result<Self> {
        let owned_ids = entries.keys().copied().collect::<BTreeSet<_>>();
        crate::modeling_label::validate_modeling_label_ids(&labels, &owned_ids, "named function")?;
        Ok(Self { entries, labels })
    }

    /// Construct a table with no labels.
    pub fn from_entries(entries: BTreeMap<NamedFunctionID, T>) -> Self {
        Self {
            entries,
            labels: NamedFunctionLabelStore::default(),
        }
    }

    /// Split the table into its row map and label store.
    ///
    /// Use this at serialization or conversion boundaries that must join
    /// labels back onto row payloads. Iterating by value is intentionally not
    /// provided, so consuming code cannot silently drop labels.
    pub fn into_parts(self) -> (BTreeMap<NamedFunctionID, T>, NamedFunctionLabelStore) {
        (self.entries, self.labels)
    }

    /// Intrinsic row map, keyed by table-owned [`NamedFunctionID`].
    pub fn entries(&self) -> &BTreeMap<NamedFunctionID, T> {
        &self.entries
    }

    /// Per-row modeling label store.
    pub fn labels(&self) -> &NamedFunctionLabelStore {
        &self.labels
    }

    /// Replace the modeling label for an existing named-function row.
    pub fn set_label(
        &mut self,
        id: NamedFunctionID,
        label: NamedFunctionLabel,
    ) -> crate::Result<()> {
        if !self.entries.contains_key(&id) {
            crate::bail!(
                { ?id },
                "Modeling label references unknown named function ID {id:?}",
            );
        }
        self.labels.insert(id, label);
        Ok(())
    }

    /// Insert one fresh row and its modeling label.
    pub fn insert(
        &mut self,
        id: NamedFunctionID,
        row: T,
        label: NamedFunctionLabel,
    ) -> crate::Result<()> {
        if self.entries.contains_key(&id) {
            crate::bail!({ ?id }, "Duplicate named function ID: {id:?}");
        }
        self.labels.insert(id, label);
        self.entries.insert(id, row);
        Ok(())
    }

    pub fn contains_key(&self, id: &NamedFunctionID) -> bool {
        self.entries.contains_key(id)
    }

    pub fn get(&self, id: &NamedFunctionID) -> Option<&T> {
        self.entries.get(id)
    }

    pub fn iter(&self) -> std::collections::btree_map::Iter<'_, NamedFunctionID, T> {
        self.entries.iter()
    }

    pub fn keys(&self) -> std::collections::btree_map::Keys<'_, NamedFunctionID, T> {
        self.entries.keys()
    }

    pub fn values(&self) -> std::collections::btree_map::Values<'_, NamedFunctionID, T> {
        self.entries.values()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn last_key_value(&self) -> Option<(&NamedFunctionID, &T)> {
        self.entries.last_key_value()
    }
}

impl<'a, T> IntoIterator for &'a NamedFunctionTable<T> {
    type Item = (&'a NamedFunctionID, &'a T);
    type IntoIter = std::collections::btree_map::Iter<'a, NamedFunctionID, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.entries.iter()
    }
}

/// `ommx.v1.EvaluatedNamedFunction` with validated, typed fields.
///
/// Modeling labels moved to a per-collection
/// [`NamedFunctionLabelStore`] on `Solution`; the struct only carries
/// intrinsic evaluated data. The legacy protobuf inline `id` is owned by the
/// enclosing `Solution` map key in the Rust domain model.
#[derive(Debug, Clone, PartialEq, CopyGetters, Getters)]
pub struct EvaluatedNamedFunction {
    #[getset(get_copy = "pub")]
    pub evaluated_value: f64,
    #[getset(get = "pub")]
    used_decision_variable_ids: VariableIDSet,
}

impl std::fmt::Display for EvaluatedNamedFunction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "EvaluatedNamedFunction(value={})", self.evaluated_value)
    }
}

/// Multiple sample evaluation results with deduplication.
///
/// Modeling labels moved to a per-collection
/// [`NamedFunctionLabelStore`] on `SampleSet`; the struct only carries
/// intrinsic sampled data. The legacy protobuf inline `id` is owned by the
/// enclosing `SampleSet` map key in the Rust domain model.
#[derive(Debug, Clone, PartialEq, Getters)]
pub struct SampledNamedFunction {
    #[getset(get = "pub")]
    evaluated_values: Sampled<f64>,
    #[getset(get = "pub")]
    used_decision_variable_ids: VariableIDSet,
}

impl std::fmt::Display for SampledNamedFunction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "SampledNamedFunction(num_samples={})",
            self.evaluated_values.num_samples()
        )
    }
}

impl SampledNamedFunction {
    /// Get an evaluated named function for a specific sample ID.
    ///
    /// Returns [`None`] if `sample_id` is not present in the sampled data.
    pub fn get(&self, sample_id: SampleID) -> Option<EvaluatedNamedFunction> {
        let evaluated_value = *self.evaluated_values.get(sample_id)?;

        Some(EvaluatedNamedFunction {
            evaluated_value,
            used_decision_variable_ids: self.used_decision_variable_ids.clone(),
        })
    }
}

#[cfg(test)]
mod table_tests {
    use super::*;

    #[test]
    fn rejects_label_for_unknown_id() {
        let mut labels = NamedFunctionLabelStore::default();
        labels.set_name(NamedFunctionID::from(1), "unknown");

        let err = NamedFunctionTable::<NamedFunction>::new(BTreeMap::new(), labels).unwrap_err();

        assert!(
            err.to_string()
                .contains("Modeling label references unknown named function ID"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn preserves_rows_and_labels() {
        let id = NamedFunctionID::from(0);
        let row = NamedFunction {
            function: Function::Zero,
        };
        let mut labels = NamedFunctionLabelStore::default();
        labels.set_name(id, "cost");

        let table = NamedFunctionTable::new(BTreeMap::from([(id, row.clone())]), labels).unwrap();

        assert_eq!(table.get(&id), Some(&row));
        assert_eq!(table.labels().name(id), Some("cost"));
    }

    #[test]
    fn insert_rejects_duplicate_without_replacing_label() {
        let id = NamedFunctionID::from(0);
        let row = NamedFunction {
            function: Function::Zero,
        };
        let mut labels = NamedFunctionLabelStore::default();
        labels.set_name(id, "cost");
        let mut table =
            NamedFunctionTable::new(BTreeMap::from([(id, row.clone())]), labels).unwrap();

        let err = table
            .insert(
                id,
                NamedFunction {
                    function: Function::Zero,
                },
                NamedFunctionLabel {
                    name: Some("new".to_string()),
                    ..Default::default()
                },
            )
            .unwrap_err();

        assert!(err.to_string().contains("Duplicate named function ID"));
        assert_eq!(table.get(&id), Some(&row));
        assert_eq!(table.labels().name(id), Some("cost"));
    }
}
