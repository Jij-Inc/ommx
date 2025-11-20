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
//! use ommx::logical_memory::{LogicalMemoryProfile, logical_memory_to_folded};
//! use ommx::polynomial_base::Linear;
//!
//! let linear = Linear::default();
//! let folded = logical_memory_to_folded("Linear", &linear);
//! println!("{}", folded);
//! ```

/// Types that provide logical memory profiling.
///
/// Implementations should enumerate their "logical memory leaves" by calling
/// `visitor.visit_leaf(path, bytes)` for each leaf node, while intermediate
/// nodes should delegate to their children.
pub trait LogicalMemoryProfile {
    /// Enumerate the "logical memory leaves" of this value.
    ///
    /// # Arguments
    /// - `path`: Logical path to the current node (mutated with push/pop during recursion)
    /// - `visitor`: Visitor that receives leaf node callbacks
    ///
    /// # Implementation Notes
    /// - At leaf nodes, call `visitor.visit_leaf(path, bytes)`
    /// - Intermediate nodes delegate to children and manage path stack
    fn visit_logical_memory<V: LogicalMemoryVisitor>(
        &self,
        path: &mut Vec<&'static str>,
        visitor: &mut V,
    );
}

/// Visitor for logical memory leaf nodes.
pub trait LogicalMemoryVisitor {
    /// Callback for a single "leaf node" (logical memory chunk).
    ///
    /// # Arguments
    /// - `path`: Logical path (e.g., `["Instance", "objective", "terms"]`)
    /// - `bytes`: Bytes used by this node
    fn visit_leaf(&mut self, path: &[&'static str], bytes: usize);
}

/// Collector for generating folded stack format.
///
/// This format is compatible with flamegraph visualization tools like
/// `flamegraph.pl` and `inferno`.
#[derive(Debug, Default)]
pub struct FoldedCollector {
    lines: Vec<String>,
}

impl FoldedCollector {
    /// Create a new folded stack collector.
    pub fn new() -> Self {
        Self::default()
    }

    /// Finish collecting and return the folded stack output.
    ///
    /// Each line has format: `"frame1;frame2;...;frameN bytes"`
    pub fn finish(self) -> String {
        self.lines.join("\n")
    }
}

impl LogicalMemoryVisitor for FoldedCollector {
    fn visit_leaf(&mut self, path: &[&'static str], bytes: usize) {
        if bytes == 0 {
            return;
        }
        let frames = path.join(";");
        self.lines.push(format!("{frames} {bytes}"));
    }
}

/// Generate folded stack format for a value.
///
/// # Arguments
/// - `root_name`: Name of the root node in the flamegraph
/// - `value`: Value to profile
///
/// # Returns
/// Folded stack format string, with each line in format `"frame1;frame2;... bytes"`
///
/// # Example
///
/// ```rust
/// use ommx::logical_memory::logical_memory_to_folded;
/// use ommx::polynomial_base::Linear;
///
/// let linear = Linear::default();
/// let folded = logical_memory_to_folded("Linear", &linear);
/// // Output: "Linear;terms 0" (for empty polynomial)
/// ```
pub fn logical_memory_to_folded<T: LogicalMemoryProfile>(
    root_name: &'static str,
    value: &T,
) -> String {
    let mut path = vec![root_name];
    let mut collector = FoldedCollector::new();
    value.visit_logical_memory(&mut path, &mut collector);
    collector.finish()
}

/// Calculate total bytes used by a value.
///
/// # Arguments
/// - `root_name`: Name of the root node (not used in calculation, for consistency with other APIs)
/// - `value`: Value to profile
///
/// # Returns
/// Total bytes across all leaf nodes
///
/// # Example
///
/// ```rust
/// use ommx::logical_memory::logical_total_bytes;
/// use ommx::polynomial_base::Linear;
///
/// let linear = Linear::default();
/// let total = logical_total_bytes("Linear", &linear);
/// assert_eq!(total, 0); // Empty polynomial
/// ```
pub fn logical_total_bytes<T: LogicalMemoryProfile>(
    root_name: &'static str,
    value: &T,
) -> usize {
    struct Sum(usize);
    impl LogicalMemoryVisitor for Sum {
        fn visit_leaf(&mut self, _path: &[&'static str], bytes: usize) {
            self.0 += bytes;
        }
    }

    let mut path = vec![root_name];
    let mut sum = Sum(0);
    value.visit_logical_memory(&mut path, &mut sum);
    sum.0
}

#[cfg(test)]
mod tests;
