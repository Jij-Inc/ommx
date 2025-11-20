# Logical Memory Profiler Design

## Overview

The logical memory profiler provides memory profiling capabilities for OMMX optimization problem instances. It enables visualization and analysis of memory usage through a flamegraph-compatible format, making it easy to identify memory-intensive components and optimize memory efficiency.

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
- ⚠️ **Approximation**: Uses `len()` and `size_of::<T>()` for estimation of data actually present (ignored unused capacity)
- ⚠️ **Different from real allocations**: Actual heap profiling tools (like `jemalloc` or `valgrind`) will show different numbers

For precise memory profiling, use dedicated heap profilers. This tool is designed for:
- Understanding logical structure and proportions
- Flamegraph visualization of component relationships
- Development-time memory analysis

### Visitor Pattern

Each type only has the responsibility of communicating its logical structure to visitors.
Output formats and aggregation methods are delegated to visitor implementations.

## API Design

### Core Types

#### Path Management

```rust
/// Logical path for memory profiling
pub struct Path(Vec<&'static str>);

impl Path {
    /// Create a new path with a root name
    pub fn new(root: &'static str) -> Self;

    /// Get the path as a slice
    pub fn as_slice(&self) -> &[&'static str];

    /// Create a path guard that automatically pops on drop
    pub fn with(&mut self, name: &'static str) -> PathGuard<'_>;
}

/// RAII guard for path management that automatically pops on drop
pub struct PathGuard<'a> {
    path: &'a mut Path,
}
```

The `PathGuard` ensures that path push/pop operations are always paired, preventing bugs from forgetting to pop.

#### Trait Definitions

```rust
/// Types that provide logical memory profiling
pub trait LogicalMemoryProfile {
    /// Enumerate the "logical memory leaves" of this value.
    ///
    /// # Arguments
    /// - `path`: Logical path to the current node (managed with RAII guards)
    /// - `visitor`: Visitor that receives leaf node callbacks
    ///
    /// # Implementation Notes
    /// - Use `path.with("name")` to create RAII guards for automatic cleanup
    /// - At leaf nodes: `visitor.visit_leaf(&path.with("field"), bytes)`
    /// - For delegation: `self.field.visit_logical_memory(path.with("field").as_mut(), visitor)`
    fn visit_logical_memory<V: LogicalMemoryVisitor>(
        &self,
        path: &mut Path,
        visitor: &mut V,
    );
}

/// Visitor for logical memory leaf nodes
pub trait LogicalMemoryVisitor {
    /// Callback for a single "leaf node" (logical memory chunk)
    ///
    /// # Arguments
    /// - `path`: Logical path
    /// - `bytes`: Bytes used by this node
    fn visit_leaf(&mut self, path: &Path, bytes: usize);
}
```

### Helper Functions

```rust
/// Generate folded stack format for a value
pub fn logical_memory_to_folded<T: LogicalMemoryProfile>(
    root_name: &'static str,
    value: &T,
) -> String;

/// Calculate total bytes used by a value
pub fn logical_total_bytes<T: LogicalMemoryProfile>(
    root_name: &'static str,
    value: &T,
) -> usize;
```

## Implementation Patterns

### Critical Implementation Rule

**Never count struct size with `size_of::<Self>()`** - this causes double-counting when the struct contains other structs as fields.

Instead, count or delegate each field individually:
- Primitive types: count with `size_of::<T>()`
- Nested structs: delegate via `visit_logical_memory()`
- Collections: count stack overhead + elements separately

### Pattern 1: Using Path Guards

**Recommended**: Use RAII guards for automatic path management

```rust
impl LogicalMemoryProfile for DecisionVariable {
    fn visit_logical_memory<V: LogicalMemoryVisitor>(
        &self,
        path: &mut Path,
        visitor: &mut V,
    ) {
        // Count primitive fields using path guards
        visitor.visit_leaf(&path.with("id"), size_of::<VariableID>());
        visitor.visit_leaf(&path.with("kind"), size_of::<Kind>());
        visitor.visit_leaf(&path.with("bound"), size_of::<Bound>());

        // Delegate to nested struct
        self.metadata.visit_logical_memory(path.with("metadata").as_mut(), visitor);
    }
}
```

**Key benefits**:
- No manual push/pop needed
- Automatic cleanup on scope exit
- Prevents path management bugs

### Pattern 2: Collections with Nested Guards

For collections, use nested guards to manage complex hierarchies:

```rust
impl LogicalMemoryProfile for ConstraintHints {
    fn visit_logical_memory<V: LogicalMemoryVisitor>(
        &self,
        path: &mut Path,
        visitor: &mut V,
    ) {
        // one_hot_constraints: Vec<OneHot>
        {
            let mut guard = path.with("one_hot_constraints");
            let vec_overhead = size_of::<Vec<OneHot>>();
            visitor.visit_leaf(&guard, vec_overhead);

            for one_hot in &self.one_hot_constraints {
                one_hot.visit_logical_memory(guard.with("OneHot").as_mut(), visitor);
            }
        } // guard automatically pops here

        // sos1_constraints: Vec<Sos1>
        {
            let mut guard = path.with("sos1_constraints");
            let vec_overhead = size_of::<Vec<Sos1>>();
            visitor.visit_leaf(&guard, vec_overhead);

            for sos1 in &self.sos1_constraints {
                sos1.visit_logical_memory(guard.with("Sos1").as_mut(), visitor);
            }
        }
    }
}
```

### Pattern 3: Key-Value Separation in Collections

For `HashMap<K, V>` and `BTreeMap<K, V>`, separate keys and values:

```rust
impl LogicalMemoryProfile for Instance {
    fn visit_logical_memory<V: LogicalMemoryVisitor>(
        &self,
        path: &mut Path,
        visitor: &mut V,
    ) {
        // decision_variables: BTreeMap<VariableID, DecisionVariable>
        {
            let mut guard = path.with("decision_variables");

            // BTreeMap stack overhead
            let map_overhead = size_of::<BTreeMap<VariableID, DecisionVariable>>();
            visitor.visit_leaf(&guard, map_overhead);

            // Keys (VariableID)
            let key_size = size_of::<VariableID>();
            let keys_bytes = self.decision_variables().len() * key_size;
            visitor.visit_leaf(&guard.with("keys"), keys_bytes);

            // Delegate to each DecisionVariable
            for dv in self.decision_variables().values() {
                dv.visit_logical_memory(guard.with("DecisionVariable").as_mut(), visitor);
            }
        }
    }
}
```

**Output example** (2 DecisionVariables):
```
Instance;decision_variables 24                              # BTreeMap overhead
Instance;decision_variables;keys 16                         # 2 × 8 bytes (VariableID)
Instance;decision_variables;DecisionVariable;id 16          # Aggregated IDs
Instance;decision_variables;DecisionVariable;bound 32       # Aggregated bounds
...
```

### Avoiding Double-Counting

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
    fn visit_logical_memory<V: LogicalMemoryVisitor>(
        &self,
        path: &mut Path,
        visitor: &mut V,
    ) {
        // Count primitive field
        visitor.visit_leaf(&path.with("field1"), size_of::<u64>());

        // Delegate to nested struct (don't count it!)
        self.field2.visit_logical_memory(path.with("field2").as_mut(), visitor);
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

- **Collections**: Count stack overhead + heap content separately (ignore unused capacity)
  - Vec: `size_of::<Vec<T>>()` + `len() * size_of::<T>()`
  - HashMap: `size_of::<HashMap<K,V>>()` + `len()`-based entry bytes
  - String: `size_of::<String>()` + `len()`

**Trade-off**: Padding between fields is not tracked, but this prevents double-counting.

### Automatic Aggregation

`FoldedCollector` automatically aggregates multiple visits to the same path:

```rust
// Multiple DecisionVariables report to same path
for dv in decision_variables.values() {
    dv.visit_logical_memory(guard.with("DecisionVariable").as_mut(), visitor);
}

// FoldedCollector aggregates automatically:
// "Instance;decision_variables;DecisionVariable;id 24" (3 × 8 bytes)
```

**Implementation**:
```rust
impl LogicalMemoryVisitor for FoldedCollector {
    fn visit_leaf(&mut self, path: &Path, bytes: usize) {
        if bytes == 0 {
            return;
        }
        let frames = path.as_slice().join(";");
        *self.aggregated.entry(frames).or_insert(0) += bytes;
    }
}
```

**Rationale**: Flamegraphs naturally aggregate same stack frames; this provides clean visualization without manual aggregation logic in each type's implementation.

## Python API

The profiler is accessible from Python via PyO3:

```python
from ommx.v1 import Instance, DecisionVariable

# Create instance
x = [DecisionVariable.binary(i) for i in range(3)]
instance = Instance.from_components(
    decision_variables=x,
    objective=x[0] + x[1],
    constraints=[],
    sense=Instance.MAXIMIZE,
)

# Get folded stack format
profile = instance.logical_memory_profile()
print(profile)
```

**Output**:
```
Instance;constraint_hints;one_hot_constraints 24
Instance;constraint_hints;sos1_constraints 24
Instance;constraints 24
Instance;decision_variable_dependency;assignments 32
Instance;decision_variable_dependency;dependency 144
Instance;decision_variables 24
Instance;decision_variables;DecisionVariable;bound 48
Instance;decision_variables;DecisionVariable;id 24
Instance;decision_variables;DecisionVariable;kind 3
Instance;decision_variables;DecisionVariable;metadata;description 72
Instance;decision_variables;DecisionVariable;metadata;name 72
Instance;decision_variables;DecisionVariable;metadata;parameters 96
Instance;decision_variables;DecisionVariable;metadata;subscripts 72
Instance;decision_variables;DecisionVariable;substituted_value 48
Instance;decision_variables;keys 24
Instance;objective;Linear;terms 104
Instance;removed_constraints 24
Instance;sense 1
```

### Visualization with Flamegraph

To create a flamegraph visualization:

1. Save the output to a file:
   ```python
   with open("profile.txt", "w") as f:
       f.write(instance.logical_memory_profile())
   ```

2. Generate SVG using `flamegraph.pl`:
   ```bash
   flamegraph.pl profile.txt > memory.svg
   ```

3. Open `memory.svg` in a browser to visualize the memory hierarchy

## Implementation Status

### Implemented Components

✅ **Core Infrastructure** (`rust/ommx/src/logical_memory.rs`):
- `Path` type with RAII guards
- `PathGuard` for automatic path management
- `LogicalMemoryProfile` trait
- `LogicalMemoryVisitor` trait
- `FoldedCollector` for folded stack generation
- Helper functions: `logical_memory_to_folded()`, `logical_total_bytes()`

✅ **Domain Type Implementations**:
- `Instance` - Root type covering major components (sense, objective, decision_variables, constraints, removed_constraints, decision_variable_dependency, constraint_hints, parameters, description)
- `v1::Parameters` and `v1::instance::Description` - Protobuf metadata fields
- `Function` enum (Linear/Quadratic/Polynomial/Zero)
- `PolynomialBase<M>` - Generic polynomial implementation
- `DecisionVariable` - Variables with metadata
- `Constraint` and `RemovedConstraint` - Constraint types
- `ConstraintHints`, `OneHot`, `Sos1` - Constraint hints
- `AcyclicAssignments` - Variable dependencies

✅ **Python Bindings** (`python/ommx/src/instance.rs`):
- `Instance.logical_memory_profile()` method
- Type stubs generation
- Comprehensive documentation with examples

✅ **Testing**:
- 30 Rust unit tests with snapshot testing
- 115 Python tests including doctests
- All tests passing

## Benefits and Trade-offs

### Benefits

✅ **Simple implementation**: RAII guards make path management automatic and bug-free
✅ **Type safety**: `Path` type prevents incorrect path manipulation
✅ **Flexibility**: Each type decides decomposition granularity
✅ **Extensibility**: New output formats just need new visitor implementations
✅ **Avoid double-counting**: Field-by-field counting prevents errors
✅ **Easy Python integration**: Folded stack passed as string

### Trade-offs

⚠️ **Approximation**: Not exact heap profiling, uses `len()` and `size_of::<T>()` (unused capacity is intentionally ignored)
⚠️ **Padding not tracked**: Field-by-field counting omits padding between fields
⚠️ **Static names only**: Paths use `&'static str`, can't include dynamic indices (can be extended with `String` if needed)

## Related Resources

- [flamegraph.pl](https://github.com/brendangregg/FlameGraph)
- [inferno (Rust flamegraph tool)](https://github.com/jonhoo/inferno)
- [Brendan Gregg's Blog on Flamegraphs](https://www.brendangregg.com/flamegraphs.html)

## Revision History

- 2025-11-20: Initial design with visitor-based approach
- 2025-11-20: Fixed double-counting by eliminating `size_of::<Self>()` usage
- 2025-11-20: Introduced `Path` and `PathGuard` RAII pattern for automatic path management
- 2025-11-20: Completed implementation with Python API and comprehensive testing
