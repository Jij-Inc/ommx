//! Logical memory profiling for OMMX types.
//!
//! This module provides a visitor-based approach to profile memory usage
//! of optimization problem instances. It generates folded stack format
//! output that can be visualized with flamegraph tools.
//!
//! # Design Philosophy
//!
//! - **Only leaf nodes emit byte counts**: This avoids inclusive/exclusive calculation complexity
//! - **Visitor pattern**: Output formats are delegated to visitor implementations
//! - **Flexible granularity**: Each type decides its own decomposition level
//!
//! # Example
//!
//! ```rust
//! use ommx::logical_memory::logical_memory_to_folded;
//! use ommx::Linear;
//!
//! let linear = Linear::default();
//! let folded = logical_memory_to_folded(&linear);
//! println!("{}", folded);
//! ```

mod collections;
mod path;
pub use path::{Path, PathGuard};

/// Types that provide logical memory profiling.
///
/// Implementations should enumerate their "logical memory leaves" by calling
/// `visitor.visit_leaf()` for each leaf node, while intermediate nodes should
/// delegate to their children.
///
/// # Recommended Implementation Pattern
///
/// Use [`Path::with()`] to create RAII guards that automatically manage path push/pop:
///
/// ```rust
/// use ommx::logical_memory::{LogicalMemoryProfile, LogicalMemoryVisitor, Path};
/// use std::mem::size_of;
///
/// struct MyStruct {
///     field1: u64,
///     field2: String,
/// }
///
/// impl LogicalMemoryProfile for MyStruct {
///     fn visit_logical_memory<V: LogicalMemoryVisitor>(
///         &self,
///         path: &mut Path,
///         visitor: &mut V,
///     ) {
///         // Count primitive fields using path guards
///         visitor.visit_leaf(&path.with("field1"), size_of::<u64>());
///
///         // Count String: stack + heap
///         let field2_bytes = size_of::<String>() + self.field2.len();
///         visitor.visit_leaf(&path.with("field2"), field2_bytes);
///     }
/// }
/// ```
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
    ///
    /// # Arguments
    /// - `path`: Logical path (e.g., `&Path::new("Instance").with("objective").with("terms")`)
    /// - `bytes`: Bytes used by this node
    fn visit_leaf(&mut self, path: &Path, bytes: usize);
}

/// Collector for generating folded stack format.
///
/// This format is compatible with flamegraph visualization tools like
/// `flamegraph.pl` and `inferno`.
///
/// The collector automatically aggregates multiple visits to the same path,
/// which is useful when profiling collections (e.g., multiple DecisionVariables
/// with the same metadata structure).
#[derive(Debug, Default)]
pub struct FoldedCollector {
    // Use HashMap to aggregate same paths
    aggregated: std::collections::HashMap<String, usize>,
}

impl FoldedCollector {
    /// Create a new folded stack collector.
    pub fn new() -> Self {
        Self::default()
    }

    /// Finish collecting and return the folded stack output.
    ///
    /// Each line has format: `"frame1;frame2;...;frameN bytes"`
    /// Lines are sorted for deterministic output.
    pub fn finish(self) -> String {
        let mut lines: Vec<_> = self
            .aggregated
            .into_iter()
            .map(|(path, bytes)| format!("{path} {bytes}"))
            .collect();
        lines.sort();
        lines.join("\n")
    }
}

impl LogicalMemoryVisitor for FoldedCollector {
    fn visit_leaf(&mut self, path: &Path, bytes: usize) {
        if bytes == 0 {
            return;
        }
        let frames = path.as_slice().join(";");
        *self.aggregated.entry(frames).or_insert(0) += bytes;
    }
}

/// Generate folded stack format for a value.
///
/// # Arguments
/// - `value`: Value to profile
///
/// # Returns
/// Folded stack format string, with each line in format `"frame1;frame2;... bytes"`
///
/// # Example
///
/// ```rust
/// use ommx::logical_memory::logical_memory_to_folded;
/// use ommx::Linear;
///
/// let linear = Linear::default();
/// let folded = logical_memory_to_folded(&linear);
/// // Output: "PolynomialBase.terms 32" (HashMap struct overhead)
/// assert_eq!(folded, "PolynomialBase.terms 32");
/// ```
pub fn logical_memory_to_folded<T: LogicalMemoryProfile>(value: &T) -> String {
    let mut path = Path::new();
    let mut collector = FoldedCollector::new();
    value.visit_logical_memory(&mut path, &mut collector);
    collector.finish()
}

/// Calculate total bytes used by a value.
///
/// # Arguments
/// - `value`: Value to profile
///
/// # Returns
/// Total bytes across all leaf nodes
///
/// # Example
///
/// ```rust
/// use ommx::logical_memory::logical_total_bytes;
/// use ommx::Linear;
///
/// let linear = Linear::default();
/// let total = logical_total_bytes(&linear);
/// // Empty polynomial has only struct overhead
/// assert!(total > 0);
/// ```
pub fn logical_total_bytes<T: LogicalMemoryProfile>(value: &T) -> usize {
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
#[macro_export]
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
                    self.$field.visit_logical_memory(
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
                    self.$field.visit_logical_memory(
                        path.with(concat!(stringify!($type_name), ".", stringify!($field))).as_mut(),
                        visitor,
                    );
                )*
            }
        }
    };
}

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
