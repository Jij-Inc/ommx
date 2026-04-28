mod arbitrary;
mod evaluate;
mod metadata_store;
pub(crate) mod parse;

use derive_more::{Deref, From};
use fnv::FnvHashMap;
use getset::*;

use crate::logical_memory::{LogicalMemoryProfile, LogicalMemoryVisitor, Path};
use crate::{Function, SampleID, Sampled, VariableIDSet};
pub use arbitrary::*;
pub use metadata_store::NamedFunctionMetadataStore;

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

impl LogicalMemoryProfile for NamedFunctionID {
    fn visit_logical_memory<V: LogicalMemoryVisitor>(&self, path: &mut Path, visitor: &mut V) {
        visitor.visit_leaf(path, std::mem::size_of::<NamedFunctionID>());
    }
}

/// A named function represents an arbitrary mathematical function with associated metadata.
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
/// Named function IDs are managed separately from decision variable IDs and constraint IDs,
/// so the same ID value can be used across these different namespaces.
///
/// Metadata (`name`, `subscripts`, `parameters`, `description`) is stored in a
/// per-collection [`NamedFunctionMetadataStore`] keyed by [`NamedFunctionID`];
/// the per-element struct no longer carries it.
///
/// Corresponds to `ommx.v1.NamedFunction`.
#[derive(Debug, Clone, PartialEq, LogicalMemoryProfile)]
pub struct NamedFunction {
    pub id: NamedFunctionID,
    pub function: Function,
}

impl std::fmt::Display for NamedFunction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "NamedFunction(id={}, {})", self.id, self.function)
    }
}

/// Auxiliary metadata for named functions (excluding intrinsic id and function).
///
/// Sibling type to [`DecisionVariableMetadata`](crate::DecisionVariableMetadata).
/// Stored ID-keyed in [`NamedFunctionMetadataStore`].
#[derive(Debug, Clone, PartialEq, Default, LogicalMemoryProfile)]
pub struct NamedFunctionMetadata {
    pub name: Option<String>,
    pub subscripts: Vec<i64>,
    pub parameters: FnvHashMap<String, String>,
    pub description: Option<String>,
}

/// `ommx.v1.EvaluatedNamedFunction` with validated, typed fields.
///
/// Metadata moved to a per-collection
/// [`NamedFunctionMetadataStore`] on `Solution`; the struct only carries
/// intrinsic evaluated data.
#[derive(Debug, Clone, PartialEq, CopyGetters, Getters)]
pub struct EvaluatedNamedFunction {
    #[getset(get_copy = "pub")]
    pub id: NamedFunctionID,
    #[getset(get_copy = "pub")]
    pub evaluated_value: f64,
    #[getset(get = "pub")]
    used_decision_variable_ids: VariableIDSet,
}

impl std::fmt::Display for EvaluatedNamedFunction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "EvaluatedNamedFunction(id={}, value={})",
            self.id, self.evaluated_value
        )
    }
}

/// Multiple sample evaluation results with deduplication.
///
/// Metadata moved to a per-collection
/// [`NamedFunctionMetadataStore`] on `SampleSet`; the struct only carries
/// intrinsic sampled data.
#[derive(Debug, Clone, PartialEq, Getters)]
pub struct SampledNamedFunction {
    #[getset(get = "pub")]
    id: NamedFunctionID,
    #[getset(get = "pub")]
    evaluated_values: Sampled<f64>,
    #[getset(get = "pub")]
    used_decision_variable_ids: VariableIDSet,
}

impl std::fmt::Display for SampledNamedFunction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "SampledNamedFunction(id={}, num_samples={})",
            self.id,
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
            id: *self.id(),
            evaluated_value,
            used_decision_variable_ids: self.used_decision_variable_ids.clone(),
        })
    }
}
