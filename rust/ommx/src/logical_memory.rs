//! Logical memory profiling for OMMX types.
//!
//! The public entry point is [`crate::Instance::logical_memory_profile`],
//! which returns a [`MemoryProfile`] that can be rendered as a folded-stack
//! string via its [`std::fmt::Display`] impl and consumed programmatically
//! through [`MemoryProfile::entries`] / [`MemoryProfile::total_bytes`].
//!
//! # Design philosophy
//!
//! - **Only leaf nodes emit byte counts.** Intermediate nodes delegate to
//!   their children; aggregation is the collector's job. This eliminates
//!   the inclusive/exclusive-bytes distinction and makes double-counting
//!   structurally impossible.
//! - **Visitor pattern.** Each type's only responsibility is to describe
//!   its logical structure to a visitor; output formats (folded stack,
//!   totals, ...) live in visitor implementations.
//! - **Flexible granularity.** Each type decides how deep to decompose
//!   itself — a struct may report every field, or collapse itself to one
//!   leaf (e.g. ID wrappers that are just `size_of::<T>()`).
//!
//! # Internal use only
//!
//! The [`LogicalMemoryProfile`] trait, [`Path`]/`PathGuard` helpers and
//! related free functions are `pub(crate)`: they are implementation details
//! used within the `ommx` crate and are not part of the public API.
//! External consumers should interact with [`MemoryProfile`] via the
//! method on [`crate::Instance`].
//!
//! # Implementation notes
//!
//! These conventions are enforced by `#[derive(LogicalMemoryProfile)]`
//! (from the `ommx-derive` crate) and by the declarative
//! `impl_logical_memory_profile!` macro. Hand-written impls for
//! generic / enum / foreign types should follow them too.
//!
//! - **Naming: `Type.field`.** Each field's frame is
//!   `"TypeName.field_name"`. Flamegraph frames then show both the
//!   owning type and the field name, which makes the hierarchy
//!   easy to read at a glance.
//!
//! - **Never write `size_of::<Self>()` at a struct leaf.** That would
//!   double-count: the struct's stack slot already includes every field
//!   by layout. Delegate to each field instead. Padding between fields
//!   is the only thing missed — an acceptable trade-off.
//!
//! - **Stack vs heap.** Primitives and POD structs (`Bound`, `Kind`, ...)
//!   emit a single leaf of `size_of::<T>()`. Collections emit a
//!   `Type[stack]` leaf for their header (`size_of::<Vec<T>>()` etc.)
//!   and then delegate to their elements; unused capacity is deliberately
//!   ignored. `String` emits `size_of::<String>() + len()` (heap
//!   bytes actually present).
//!
//! - **Aggregation.** Multiple visits to the same path accumulate in
//!   [`MemoryProfile`]. So profiling a `BTreeMap<Id, T>` with 1000
//!   entries produces one line per unique path, not 1000 duplicates.
//!
//! # Caveats
//!
//! This is a logical-structure estimation, not exact heap profiling.
//! Allocator overhead, internal fragmentation, and padding between
//! fields are not tracked. Unused `Vec` / `HashMap` capacity is
//! deliberately ignored — only bytes holding live data are counted.
//! For precise heap accounting use a dedicated profiler (jemalloc,
//! valgrind, heaptrack); this tool is for understanding proportions
//! and flamegraph visualization.

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
///
/// This trait is `pub` so it can appear in the bound of public types
/// (e.g. `ConstraintMetadataStore<ID>` where `ID: LogicalMemoryProfile`)
/// without triggering the `private_bounds` lint, and so the derive
/// macro can be used at every struct that participates in profiling
/// without having to fall back to a hand-written impl. The intended
/// user-facing entry points are still
/// [`crate::Instance::logical_memory_profile`] and
/// [`crate::MemoryProfile`]; the trait itself has no stability
/// guarantees beyond "exists, has the same shape across patch
/// versions".
pub trait LogicalMemoryProfile {
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
pub trait LogicalMemoryVisitor {
    /// Callback for a single "leaf node" (logical memory chunk).
    fn visit_leaf(&mut self, path: &Path, bytes: usize);
}

/// Logical memory profile of a value.
///
/// This is the output type of [`crate::Instance::logical_memory_profile`].
/// Internally it is a flat map from logical path (e.g.
/// `["Instance", "objective", ...]`) to the number of bytes attributed to
/// that leaf.
///
/// # Caveats
///
/// Reported bytes are a logical-structure estimation, not exact heap
/// profiling: allocator overhead, padding, and unused collection capacity
/// are deliberately ignored. See the module docs for details. Use for
/// proportions and flamegraph visualization, not for total-allocation
/// accounting.
///
/// # Flamegraph workflow
///
/// The [`std::fmt::Display`] impl produces the folded-stack format read
/// by `flamegraph.pl` and `inferno`:
///
/// ```bash
/// # in a Rust program / test / example
/// std::fs::write("profile.txt", instance.logical_memory_profile().to_string())?;
///
/// # then, in the shell:
/// flamegraph.pl profile.txt > memory.svg
/// # or with inferno:
/// inferno-flamegraph < profile.txt > memory.svg
/// ```
///
/// External tools:
/// - `flamegraph.pl`: <https://github.com/brendangregg/FlameGraph>
/// - `inferno` (Rust): <https://github.com/jonhoo/inferno>
#[derive(Debug, Clone, Default)]
pub struct MemoryProfile {
    // All frame names come from string literals and `concat!()` expansions,
    // so `&'static str` is sufficient. This avoids allocating a `String` per
    // path segment (and the segments account for the bulk of the work during
    // profiling) — only one `Vec` per leaf visit remains.
    entries: BTreeMap<Vec<&'static str>, usize>,
}

impl MemoryProfile {
    /// Total bytes across all leaves.
    pub fn total_bytes(&self) -> usize {
        self.entries.values().sum()
    }

    /// Iterate over `(path, bytes)` pairs.
    ///
    /// Each path is a slice of frame names like `["Instance", "objective", ...]`.
    /// The iteration order follows the natural ordering of paths.
    pub fn entries(&self) -> impl Iterator<Item = (&[&'static str], usize)> {
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
        *self.entries.entry(path.as_slice().to_vec()).or_insert(0) += bytes;
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
    let mut profile = MemoryProfile::default();
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

// Re-export the derive macro alongside the trait so downstream
// modules (and external crates, since both the trait and this
// re-export are `pub`) can write `use ommx::LogicalMemoryProfile;`
// and then `#[derive(LogicalMemoryProfile)]`. Promoting from
// `pub(crate)` to `pub` lets the derive replace every mechanical
// hand-written `impl` — the intended fix for "added a new field
// and forgot to update the impl".
pub use ommx_derive::LogicalMemoryProfile;

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
