mod arbitrary;
mod evaluate;
mod parse;
mod serialize;

use derive_more::{Deref, From};
use fnv::FnvHashMap;
use getset::*;

use crate::{Function, SampleID, Sampled, UnknownSampleIDError, VariableIDSet};
pub use arbitrary::*;

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
/// Corresponds to `ommx.v1.NamedFunction`.
#[derive(Debug, Clone, PartialEq)]
pub struct NamedFunction {
    pub id: NamedFunctionID,
    pub function: Function,
    pub name: Option<String>,
    pub subscripts: Vec<i64>,
    pub parameters: FnvHashMap<String, String>,
    pub description: Option<String>,
}

/// `ommx.v1.EvaluatedNamedFunction` with validated, typed fields.
#[derive(Debug, Clone, PartialEq, CopyGetters, Getters)]
pub struct EvaluatedNamedFunction {
    #[getset(get_copy = "pub")]
    pub id: NamedFunctionID,
    #[getset(get_copy = "pub")]
    pub evaluated_value: f64,
    #[getset(get = "pub")]
    pub name: Option<String>,
    #[getset(get = "pub")]
    pub subscripts: Vec<i64>,
    #[getset(get = "pub")]
    pub parameters: FnvHashMap<String, String>,
    #[getset(get = "pub")]
    pub description: Option<String>,
    #[getset(get = "pub")]
    used_decision_variable_ids: VariableIDSet,
}

/// Multiple sample evaluation results with deduplication
#[derive(Debug, Clone, PartialEq, Getters)]
pub struct SampledNamedFunction {
    #[getset(get = "pub")]
    id: NamedFunctionID,
    #[getset(get = "pub")]
    evaluated_values: Sampled<f64>,
    #[getset(get = "pub")]
    pub name: Option<String>,
    #[getset(get = "pub")]
    pub subscripts: Vec<i64>,
    #[getset(get = "pub")]
    pub parameters: FnvHashMap<String, String>,
    #[getset(get = "pub")]
    pub description: Option<String>,
    #[getset(get = "pub")]
    used_decision_variable_ids: VariableIDSet,
}

impl SampledNamedFunction {
    /// Get an evaluated named function for a specific sample ID
    pub fn get(&self, sample_id: SampleID) -> Result<EvaluatedNamedFunction, UnknownSampleIDError> {
        let evaluated_value = *self.evaluated_values.get(sample_id)?;

        Ok(EvaluatedNamedFunction {
            id: *self.id(),
            evaluated_value,
            name: self.name.clone(),
            subscripts: self.subscripts.clone(),
            parameters: self.parameters.clone(),
            description: self.description.clone(),
            used_decision_variable_ids: self.used_decision_variable_ids.clone(),
        })
    }
}
