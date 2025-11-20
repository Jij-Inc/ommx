# Logical Memory Profiler Design

## Overview

Add memory profiling capabilities to `ommx::Instance` and related types to analyze logical memory usage of optimization problem instances. This enables visualization and tuning of memory efficiency.

## Motivation

### Background

Memory usage often becomes a bottleneck for large-scale optimization problems, particularly:

- **Polynomial data structures**: `PolynomialBase<M>` with `FnvHashMap<M, Coefficient>` for storing terms
- **Decision variables and constraints**: `BTreeMap` collections with metadata like names, bounds
- **Function representations**: `Linear`, `Quadratic`, `Polynomial` variants in objective and constraints

Understanding actual memory consumption of these structures enables optimization opportunities.

### Requirements

1. **Logical structure-based profiling**: Aggregate memory by user-understandable logical structures, not physical memory layout
2. **Flamegraph support**: Output in folded stack format for easy visualization tool integration
3. **Flexible granularity**: Each type can decide "how much to decompose"
4. **Python accessibility**: Easy to use from Python via PyO3

## Design Philosophy

### Core Principle

**"Only leaf nodes emit byte counts"**

This design provides:

- **No inclusive/exclusive calculation needed**: Only leaf nodes report memory, naturally avoiding double-counting
- **Simple Trait**: Unified with `visit` pattern only, no need for multiple methods like `size_bytes()`
- **Easy aggregation**: Derived information (total bytes, folded stack) can be added as visitor implementations

### Visitor Pattern

Each type only has the responsibility of communicating its logical structure to visitors.
Output formats and aggregation methods are delegated to visitor implementations.

## API Design

### Trait Definitions

```rust
/// Types that provide logical memory profiling
pub trait LogicalMemoryProfile {
    /// Enumerate the "logical memory leaves" of this value.
    ///
    /// # Arguments
    /// - `path`: Logical path to the current node (mutated with push/pop during recursion)
    /// - `visitor`: Visitor that receives leaf node callbacks
    ///
    /// # Implementation Notes
    /// - At leaf nodes, call `visitor.visit_leaf(path, bytes)`
    /// - Intermediate nodes delegate to children
    fn visit_logical_memory<V: LogicalMemoryVisitor>(
        &self,
        path: &mut Vec<&'static str>,
        visitor: &mut V
    );
}

/// Visitor for logical memory leaf nodes
pub trait LogicalMemoryVisitor {
    /// Callback for a single "leaf node" (logical memory chunk)
    ///
    /// # Arguments
    /// - `path`: Logical path (e.g., `["Model", "matrix", "values"]`)
    /// - `bytes`: Bytes used by this node
    fn visit_leaf(&mut self, path: &[&'static str], bytes: usize);
}
```

### Path Representation

- Use `&[&'static str]` (almost all names can be covered with literals)
- Separator is determined by visitor (`';'` for folded stack)
- Extension to allow `String` for dynamic names can be considered if needed

## Implementation Patterns

### Pattern 1: Leaf Node (No Logical Decomposition)

**Example**: CSR matrix `values` array

```rust
struct Values {
    data: Vec<f64>,
}

impl LogicalMemoryProfile for Values {
    fn visit_logical_memory<V: LogicalMemoryVisitor>(
        &self,
        path: &mut Vec<&'static str>,
        visitor: &mut V
    ) {
        path.push("values");
        let bytes = self.data.capacity() * std::mem::size_of::<f64>();
        visitor.visit_leaf(path, bytes);
        path.pop();
    }
}
```

**Key points**:
- Use `capacity()` (actual allocated memory, not `len()`)
- Get element size with `std::mem::size_of::<T>()`

### Pattern 2: Intermediate Node (Delegate to Children)

**Example**: Quadratic polynomial (function in objective or constraint)

```rust
// Quadratic = PolynomialBase<QuadraticMonomial>
// where PolynomialBase<M> has: terms: FnvHashMap<M, Coefficient>

impl<M: Monomial> LogicalMemoryProfile for PolynomialBase<M> {
    fn visit_logical_memory<V: LogicalMemoryVisitor>(
        &self,
        path: &mut Vec<&'static str>,
        visitor: &mut V
    ) {
        path.push("terms");

        // HashMap: keys (monomials) + values (coefficients) + overhead
        let map_overhead = std::mem::size_of::<FnvHashMap<M, Coefficient>>();
        let keys_bytes = self.terms.capacity() * std::mem::size_of::<M>();
        let values_bytes = self.terms.capacity() * std::mem::size_of::<Coefficient>();

        visitor.visit_leaf(path, map_overhead + keys_bytes + values_bytes);
        path.pop();
    }
}
```

**Emitted leaves**:
- `["...", "Quadratic", "terms"]` with total HashMap memory
- Or split into `["...", "keys"]`, `["...", "values"]`, `["...", "overhead"]` for finer granularity

### Pattern 3: Root Node

**Example**: Entire optimization problem instance

```rust
impl LogicalMemoryProfile for Instance {
    fn visit_logical_memory<V: LogicalMemoryVisitor>(
        &self,
        path: &mut Vec<&'static str>,
        visitor: &mut V
    ) {
        path.push("Instance");

        // Objective function
        path.push("objective");
        self.objective.visit_logical_memory(path, visitor);
        path.pop();

        // Decision variables (BTreeMap<VariableID, DecisionVariable>)
        path.push("decision_variables");
        // ... visit BTreeMap structure
        path.pop();

        // Constraints (BTreeMap<ConstraintID, Constraint>)
        path.push("constraints");
        // ... visit each constraint's function
        path.pop();

        // Removed constraints
        path.push("removed_constraints");
        // ... visit BTreeMap structure
        path.pop();

        path.pop(); // "Instance"
    }
}
```

**Key point**: Root node doesn't emit leaves itself, delegates everything to children

## Use Cases

### 1. Folded Stack Generation

Generate folded stack format for flamegraph visualization tools:

```rust
pub struct FoldedCollector {
    lines: Vec<String>,
}

impl FoldedCollector {
    pub fn new() -> Self {
        Self { lines: Vec::new() }
    }

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

// Helper function
pub fn logical_memory_to_folded<T: LogicalMemoryProfile>(
    root_name: &'static str,
    value: &T
) -> String {
    let mut path = vec![root_name];
    let mut collector = FoldedCollector::new();
    value.visit_logical_memory(&mut path, &mut collector);
    collector.finish()
}
```

**Usage**:
```rust
let instance: Instance = ...;
let folded = logical_memory_to_folded("Instance", &instance);
// Pass to flamegraph.pl or similar tools
```

### 2. Total Bytes Calculation

```rust
pub fn logical_total_bytes<T: LogicalMemoryProfile>(
    root_name: &'static str,
    value: &T
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
```

### 3. JSON/YAML Tree Output

Hierarchical JSON or YAML output can also be implemented as visitors:

```json
{
  "Instance": {
    "objective": {
      "Quadratic": {
        "terms": 16384
      }
    },
    "decision_variables": {
      "btree_overhead": 512,
      "entries": 2048
    },
    "constraints": {
      "btree_overhead": 1024,
      "entries": {
        "functions": 8192
      }
    }
  }
}
```

### 4. ASCII Tree Output

For CLI tool display:

```
Instance
├─ objective (Quadratic)
│  └─ terms: 16.0 KB
├─ decision_variables
│  ├─ btree_overhead: 512 B
│  └─ entries: 2.0 KB
└─ constraints
   ├─ btree_overhead: 1.0 KB
   └─ entries: 8.0 KB
```

## Python API Design

Make it accessible from Python via PyO3:

```python
from ommx.v1 import Instance

instance = Instance.from_file("problem.ommx")

# Get folded stack
folded = instance.logical_memory_profile()
print(folded)

# Total bytes
total = instance.logical_memory_bytes()
print(f"Total: {total / 1024 / 1024:.2f} MB")

# Visualization in Jupyter
instance.show_memory_flamegraph()  # Display SVG inline
```

## Implementation Plan

### Phase 1: Core Traits and Utilities

1. Create `rust/ommx/src/logical_memory.rs`
   - `LogicalMemoryProfile` trait
   - `LogicalMemoryVisitor` trait
   - `FoldedCollector`
   - `logical_memory_to_folded()`
   - `logical_total_bytes()`

2. Add generic implementations
   - Implementation for `Vec<T>`
   - Implementation for `HashMap<K, V>`
   - Implementation for primitive types

### Phase 2: Domain Type Implementations

3. Implement `LogicalMemoryProfile` for major types
   - `Instance` (root node, delegates to all fields)
   - `Function` enum variants (`Linear`, `Quadratic`, `Polynomial`)
   - `PolynomialBase<M>` (generic implementation for all polynomial types)
   - `Constraint` (delegates to embedded `Function`)
   - Collections: `BTreeMap<K, V>`, `FnvHashMap<K, V>`

4. Add tests
   - `rust/ommx/tests/logical_memory_test.rs`
   - Integration tests for flamegraph generation
   - Verify memory accounting accuracy

### Phase 3: Python Bindings

5. Add PyO3 methods
   - `Instance.logical_memory_profile() -> str`
   - `Instance.logical_memory_bytes() -> int`

6. Python-side utilities
   - Flamegraph SVG generation helper
   - Display functions for Jupyter

### Phase 4: Documentation and Samples

7. API documentation
8. Add tutorials to Jupyter Book
9. Sample code and benchmarks

## Benefits and Trade-offs

### Benefits

✅ **Simple Trait**: Only `visit` method needed, concise implementation
✅ **Flexibility**: Each type can decide decomposition granularity freely
✅ **Extensibility**: New output formats just need new visitor implementations
✅ **Avoid double-counting**: No inclusive/exclusive calculation needed
✅ **Easy Python integration**: Folded stack can be passed as string

### Trade-offs

⚠️ **Dynamic dispatch overhead**: Visitor pattern has slight overhead (acceptable for profiling use case)
⚠️ **Path management**: Need to correctly push/pop `Vec<&'static str>` (addressed with docs and samples)
⚠️ **Dynamic names**: Paths with indices or keys can't be represented with `&'static str` (future extension with `Cow<'static, str>` can be considered)

## Related Resources

- [flamegraph.pl](https://github.com/brendangregg/FlameGraph)
- [inferno (Rust flamegraph tool)](https://github.com/jonhoo/inferno)
- [Python memory_profiler](https://pypi.org/project/memory-profiler/)

## Revision History

- 2025-11-20: Initial version (visitor-based design)
