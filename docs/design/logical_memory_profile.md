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

### Important Limitations

**This is logical memory estimation, not exact heap profiling**

The reported byte counts are:
- ✅ **Useful for**: Comparing relative sizes, identifying large data structures, tracking growth trends
- ⚠️ **Not exact**: Does not account for allocator overhead, padding, internal fragmentation
- ⚠️ **Approximation**: Uses `capacity()` and `size_of::<T>()` for estimation
- ⚠️ **Different from real allocations**: Actual heap profiling tools (like `jemalloc` or `valgrind`) will show different numbers

For precise memory profiling, use dedicated heap profilers. This tool is designed for:
- Understanding logical structure and proportions
- Flamegraph visualization of component relationships
- Development-time memory analysis

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

### Critical Implementation Rule

**Never count struct size with `size_of::<Self>()`** - this causes double-counting when the struct contains other structs as fields.

Instead, count or delegate each field individually:
- Primitive types: count with `size_of::<T>()`
- Nested structs: delegate via `visit_logical_memory()`
- Collections: count stack overhead + elements separately

### Pattern 1: Struct with Primitive Fields

**Example**: Simple struct with primitive fields

```rust
struct Point {
    x: f64,
    y: f64,
}

impl LogicalMemoryProfile for Point {
    fn visit_logical_memory<V: LogicalMemoryVisitor>(
        &self,
        path: &mut Vec<&'static str>,
        visitor: &mut V
    ) {
        // Count each field individually
        path.push("x");
        visitor.visit_leaf(path, std::mem::size_of::<f64>());
        path.pop();

        path.push("y");
        visitor.visit_leaf(path, std::mem::size_of::<f64>());
        path.pop();
    }
}
```

**Key points**:
- Count each primitive field separately
- Do NOT use `size_of::<Point>()` - this would include padding

### Pattern 2: Struct with Nested Struct Fields

**Example**: Struct containing another struct

```rust
struct DecisionVariable {
    id: VariableID,           // u64 wrapper
    kind: Kind,               // enum
    bound: Bound,             // struct with two f64s
    metadata: Metadata,       // nested struct
}

impl LogicalMemoryProfile for DecisionVariable {
    fn visit_logical_memory<V: LogicalMemoryVisitor>(
        &self,
        path: &mut Vec<&'static str>,
        visitor: &mut V
    ) {
        // Count primitive/simple fields
        path.push("id");
        visitor.visit_leaf(path, std::mem::size_of::<VariableID>());
        path.pop();

        path.push("kind");
        visitor.visit_leaf(path, std::mem::size_of::<Kind>());
        path.pop();

        path.push("bound");
        visitor.visit_leaf(path, std::mem::size_of::<Bound>());
        path.pop();

        // Delegate to nested struct
        path.push("metadata");
        self.metadata.visit_logical_memory(path, visitor);
        path.pop();
    }
}
```

**Key points**:
- Simple structs (like `Bound` with just two f64s) can be counted directly
- Complex structs with heap allocations should be delegated
- Never use `size_of::<DecisionVariable>()` - would double-count metadata

### Pattern 3: Collections (Vec, HashMap, BTreeMap)

**Example**: Struct with collection fields

```rust
struct Metadata {
    name: Option<String>,
    subscripts: Vec<i64>,
    parameters: HashMap<String, String>,
}

impl LogicalMemoryProfile for Metadata {
    fn visit_logical_memory<V: LogicalMemoryVisitor>(
        &self,
        path: &mut Vec<&'static str>,
        visitor: &mut V
    ) {
        // Option<String>: count stack + heap
        path.push("name");
        let name_bytes = std::mem::size_of::<Option<String>>()
            + self.name.as_ref().map_or(0, |s| s.capacity());
        visitor.visit_leaf(path, name_bytes);
        path.pop();

        // Vec<i64>: count stack + heap
        path.push("subscripts");
        let vec_bytes = std::mem::size_of::<Vec<i64>>()
            + self.subscripts.capacity() * std::mem::size_of::<i64>();
        visitor.visit_leaf(path, vec_bytes);
        path.pop();

        // HashMap: count stack + entries
        path.push("parameters");
        let map_overhead = std::mem::size_of::<HashMap<String, String>>();
        let mut entries_bytes = 0;
        for (k, v) in &self.parameters {
            entries_bytes += std::mem::size_of::<(String, String)>();
            entries_bytes += k.capacity() + v.capacity();
        }
        visitor.visit_leaf(path, map_overhead + entries_bytes);
        path.pop();
    }
}
```

**Key points**:
- Always count collection stack overhead (Vec header, HashMap header, etc.)
- Use `capacity()` for heap-allocated sizes, not `len()`
- For String, count both `size_of::<String>()` and `capacity()`

## Implementation Guidelines

### Avoiding Double-Counting

**Critical Rule**: Never use `size_of::<Self>()` to count the entire struct size.

**Problem**: When struct A contains struct B as a field:
```rust
struct A {
    field1: u64,
    field2: B,  // B's stack space is included in size_of::<A>()
}
```

If you count both `size_of::<A>()` and then delegate to `B.visit_logical_memory()`, the stack space of B gets counted twice.

**Solution**: Count or delegate each field individually:

```rust
impl LogicalMemoryProfile for A {
    fn visit_logical_memory<V: LogicalMemoryVisitor>(...) {
        // Count field1
        path.push("field1");
        visitor.visit_leaf(path, size_of::<u64>());
        path.pop();

        // Delegate to field2 (don't count it!)
        path.push("field2");
        self.field2.visit_logical_memory(path, visitor);
        path.pop();
    }
}
```

### Stack vs Heap Counting

- **Primitive types**: Count with `size_of::<T>()`
  - Examples: `u64`, `f64`, `bool`, enums

- **Simple structs** (no heap allocations): Count with `size_of::<T>()`
  - Example: `Bound` (just two f64s)

- **Complex structs** (with heap allocations): Delegate via `visit_logical_memory()`
  - Examples: `Metadata`, `Constraint`, `DecisionVariable`

- **Collections**: Count stack overhead + heap separately
  - Vec: `size_of::<Vec<T>>()` + `capacity() * size_of::<T>()`
  - HashMap: `size_of::<HashMap<K,V>>()` + entry bytes
  - String: `size_of::<String>()` + `capacity()`

**Trade-off**: Padding between fields is not tracked, but this prevents double-counting.

### Key-Value Separation in Collections

For `HashMap<K, V>` and `BTreeMap<K, V>`, separate keys and values into distinct paths:

```rust
// BTreeMap overhead
let map_overhead = size_of::<BTreeMap<K, V>>();
visitor.visit_leaf(path, map_overhead);

// Keys
path.push("keys");
let keys_bytes = len * size_of::<K>();
visitor.visit_leaf(path, keys_bytes);
path.pop();

// Delegate to each value
for value in values() {
    path.push("ValueType");  // Add type name
    value.visit_logical_memory(path, visitor);
    path.pop();
}
```

**Output example** (2 DecisionVariables):
```
Instance;decision_variables 24                              # BTreeMap overhead
Instance;decision_variables;keys 16                         # 2 × 8 bytes (VariableID)
Instance;decision_variables;DecisionVariable 304            # 2 × 152 bytes (aggregated)
Instance;decision_variables;DecisionVariable;metadata;name 95  # metadata heap allocations
```

**Rationale**: Provides clear breakdown of collection structure vs content in flamegraphs.

### Type Names in Delegation Paths

When delegating from a collection to value types, add the type name to the path:

```rust
// ❌ Bad: Values reported directly under collection path
for constraint in constraints.values() {
    constraint.visit_logical_memory(path, visitor);
}

// ✅ Good: Values reported under type-specific subpath
for constraint in constraints.values() {
    path.push("Constraint");
    constraint.visit_logical_memory(path, visitor);
    path.pop();
}
```

**Rationale**: Creates hierarchical flamegraph structure that distinguishes collection metadata from element content.

### Automatic Aggregation

`FoldedCollector` automatically aggregates multiple visits to the same path:

```rust
// Multiple DecisionVariables report to same path
for dv in decision_variables.values() {
    path.push("DecisionVariable");
    dv.visit_logical_memory(path, visitor);  // Each reports 152 bytes
    path.pop();
}

// FoldedCollector aggregates: "Instance;decision_variables;DecisionVariable 304"
```

**Implementation** (in `FoldedCollector`):
```rust
impl LogicalMemoryVisitor for FoldedCollector {
    fn visit_leaf(&mut self, path: &[&'static str], bytes: usize) {
        let frames = path.join(";");
        *self.aggregated.entry(frames).or_insert(0) += bytes;
    }
}
```

**Rationale**: Flamegraphs naturally aggregate same stack frames; this provides clean visualization without manual aggregation logic in each type's implementation.

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
- 2025-11-20: Fixed double-counting issue by eliminating `size_of::<Self>()` usage
  - Changed to count each field individually instead of entire struct size
  - Prevents double-counting when structs contain other structs as fields
  - Trade-off: padding between fields is no longer tracked
  - Updated all implementation patterns and guidelines
