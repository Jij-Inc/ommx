//! Logical memory profiling for OMMX types.
//!
//! This module provides a visitor-based approach to profile memory usage
//! of optimization problem instances. The public entry point is
//! [`crate::Instance::logical_memory_profile`], which returns a
//! [`MemoryProfile`] that can be rendered as a folded-stack string via
//! its [`std::fmt::Display`] impl and consumed programmatically through
//! [`MemoryProfile::entries`] / [`MemoryProfile::total_bytes`].
//!
//! # Design Philosophy
//!
//! - **Only leaf nodes emit byte counts**: avoids inclusive/exclusive calculation complexity
//! - **Visitor pattern**: output formats are delegated to visitor implementations
//! - **Flexible granularity**: each type decides its own decomposition level
//!
//! # Internal use only
//!
//! The `LogicalMemoryProfile` trait, `Path`/`PathGuard` helpers and related
//! free functions are `pub(crate)`: they are implementation details used within
//! the `ommx` crate and are not part of the public API. External consumers
//! should interact with [`MemoryProfile`] via the method on [`crate::Instance`].

mod collections;
mod path;
pub(crate) use path::Path;

use std::collections::BTreeMap;
use std::fmt;

/// Types that provide logical memory profiling.
///
/// Implementations should enumerate their "logical memory leaves" by calling
/// `visitor.visit_leaf()` for each leaf node, while intermediate nodes should
/// delegate to their children.
pub(crate) trait LogicalMemoryProfile {
    /// Enumerate the "logical memory leaves" of this value.
    ///
    /// # Arguments
    /// - `path`: Logical path to the current node (mutated during recursion)
    /// - `visitor`: Visitor that receives leaf node callbacks
    ///
    /// # Implementation Notes
    /// - Use `path.with("name")` to create RAII guards for automatic cleanup
    /// - At leaf nodes: `visitor.visit_leaf(path.with("field"), bytes)`
    /// - For delegation: `self.field.visit_logical_memory(path.with("field").as_mut(), visitor)`
    fn visit_logical_memory<V: LogicalMemoryVisitor>(&self, path: &mut Path, visitor: &mut V);
}

/// Visitor for logical memory leaf nodes.
pub(crate) trait LogicalMemoryVisitor {
    /// Callback for a single "leaf node" (logical memory chunk).
    fn visit_leaf(&mut self, path: &Path, bytes: usize);
}

/// Logical memory profile of a value.
///
/// This is the public output type of [`crate::Instance::logical_memory_profile`].
/// Internally it is a flat map from logical path (e.g. `["Instance", "objective", ...]`)
/// to the number of bytes attributed to that leaf.
///
/// Render as a folded-stack string with [`ToString::to_string`] (via the
/// [`std::fmt::Display`] impl) to feed into flamegraph tools such as
/// `flamegraph.pl` or `inferno`.
#[derive(Debug, Clone, Default)]
pub struct MemoryProfile {
    entries: BTreeMap<Vec<String>, usize>,
}

impl MemoryProfile {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Total bytes across all leaves.
    pub fn total_bytes(&self) -> usize {
        self.entries.values().sum()
    }

    /// Iterate over `(path, bytes)` pairs.
    ///
    /// Each path is a slice of frame names like `["Instance", "objective", ...]`.
    /// The iteration order follows the natural ordering of paths.
    pub fn entries(&self) -> impl Iterator<Item = (&[String], usize)> {
        self.entries
            .iter()
            .map(|(path, bytes)| (path.as_slice(), *bytes))
    }

    /// Number of distinct leaf paths recorded.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether no leaves were recorded.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl LogicalMemoryVisitor for MemoryProfile {
    fn visit_leaf(&mut self, path: &Path, bytes: usize) {
        if bytes == 0 {
            return;
        }
        let key: Vec<String> = path.as_slice().iter().map(|s| (*s).to_string()).collect();
        *self.entries.entry(key).or_insert(0) += bytes;
    }
}

/// Renders the profile as folded stack format: each leaf on its own line as
/// `"frame1;frame2;...;frameN bytes"`, lines sorted for deterministic output.
impl fmt::Display for MemoryProfile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut first = true;
        for (path, bytes) in &self.entries {
            if !first {
                writeln!(f)?;
            }
            first = false;
            let frames = path.join(";");
            write!(f, "{frames} {bytes}")?;
        }
        Ok(())
    }
}

/// Build a [`MemoryProfile`] for a value.
pub(crate) fn build_profile<T: LogicalMemoryProfile>(value: &T) -> MemoryProfile {
    let mut path = Path::new();
    let mut profile = MemoryProfile::new();
    value.visit_logical_memory(&mut path, &mut profile);
    profile
}

/// Convenience wrapper returning the folded-stack string directly.
///
/// Equivalent to `build_profile(value).to_string()`.
#[cfg(test)]
pub(crate) fn logical_memory_to_folded<T: LogicalMemoryProfile>(value: &T) -> String {
    build_profile(value).to_string()
}

/// Total bytes used by a value.
#[cfg(test)]
pub(crate) fn logical_total_bytes<T: LogicalMemoryProfile>(value: &T) -> usize {
    struct Sum(usize);
    impl LogicalMemoryVisitor for Sum {
        fn visit_leaf(&mut self, _path: &Path, bytes: usize) {
            self.0 += bytes;
        }
    }

    let mut path = Path::new();
    let mut sum = Sum(0);
    value.visit_logical_memory(&mut path, &mut sum);
    sum.0
}

// Macro to implement LogicalMemoryProfile for structs with fields
/// Generates a LogicalMemoryProfile implementation that delegates to each field.
///
/// Kept for types that cannot use `#[derive(LogicalMemoryProfile)]`, e.g. types
/// defined in external modules where we need an explicit type-name override.
///
/// # Example
/// ```ignore
/// impl_logical_memory_profile! {
///     RemovedConstraint {
///         constraint,
///         removed_reason,
///         removed_reason_parameters,
///     }
/// }
///
/// // For types with path (e.g., v1::Parameters), specify type name explicitly:
/// impl_logical_memory_profile! {
///     v1::Parameters as "Parameters" {
///         entries,
///     }
/// }
/// ```
macro_rules! impl_logical_memory_profile {
    // For types with explicit name (e.g., v1::Parameters as "Parameters")
    ($type_path:path as $type_name:literal { $($field:ident),* $(,)? }) => {
        impl $crate::logical_memory::LogicalMemoryProfile for $type_path {
            fn visit_logical_memory<V: $crate::logical_memory::LogicalMemoryVisitor>(
                &self,
                path: &mut $crate::logical_memory::Path,
                visitor: &mut V,
            ) {
                $(
                    $crate::logical_memory::LogicalMemoryProfile::visit_logical_memory(
                        &self.$field,
                        path.with(concat!($type_name, ".", stringify!($field))).as_mut(),
                        visitor,
                    );
                )*
            }
        }
    };
    // For simple types (e.g., RemovedConstraint)
    ($type_name:ident { $($field:ident),* $(,)? }) => {
        impl $crate::logical_memory::LogicalMemoryProfile for $type_name {
            fn visit_logical_memory<V: $crate::logical_memory::LogicalMemoryVisitor>(
                &self,
                path: &mut $crate::logical_memory::Path,
                visitor: &mut V,
            ) {
                $(
                    $crate::logical_memory::LogicalMemoryProfile::visit_logical_memory(
                        &self.$field,
                        path.with(concat!(stringify!($type_name), ".", stringify!($field))).as_mut(),
                        visitor,
                    );
                )*
            }
        }
    };
}
pub(crate) use impl_logical_memory_profile;

// Re-export the derive macro so downstream modules can write
// `use crate::logical_memory::LogicalMemoryProfile;` and then
// `#[derive(LogicalMemoryProfile)]`.
pub(crate) use ommx_derive::LogicalMemoryProfile;

// Generic implementations for primitive types

macro_rules! impl_logical_memory_profile_for_primitive {
    ($($ty:ty),*) => {
        $(
            impl LogicalMemoryProfile for $ty {
                fn visit_logical_memory<V: LogicalMemoryVisitor>(&self, path: &mut Path, visitor: &mut V) {
                    use std::mem::size_of;
                    visitor.visit_leaf(path, size_of::<$ty>());
                }
            }
        )*
    };
}

impl_logical_memory_profile_for_primitive!(
    u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64
);

#[cfg(test)]
mod tests;
