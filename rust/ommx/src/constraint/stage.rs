use fnv::FnvHashMap;

use super::Constraint;

/// Trait that defines the stage-specific data a constraint carries at each lifecycle phase.
///
/// The type parameter `C` is the constraint struct itself (e.g. `Constraint<S>`),
/// allowing different constraint types to have different stage data for the same stage.
pub trait Stage<C> {
    type Data;
}

/// The constraint as defined in the problem, before evaluation.
#[derive(Debug, Clone, PartialEq)]
pub struct Created;

/// The constraint has been removed (relaxed) from the active set.
#[derive(Debug, Clone, PartialEq)]
pub struct Removed;

// ===== Stage data types for regular Constraint =====

/// Data carried by a regular constraint in the Created stage.
#[derive(Debug, Clone, PartialEq)]
pub struct CreatedData {
    pub function: crate::Function,
}

/// Data carried by a regular constraint in the Removed stage.
#[derive(Debug, Clone, PartialEq)]
pub struct RemovedData {
    pub function: crate::Function,
    pub removed_reason: String,
    pub removed_reason_parameters: FnvHashMap<String, String>,
}

// ===== Stage implementations for Constraint =====

impl Stage<Constraint<Created>> for Created {
    type Data = CreatedData;
}

impl Stage<Constraint<Removed>> for Removed {
    type Data = RemovedData;
}
