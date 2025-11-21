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
pub struct Path(Vec<String>);

impl Path {
    /// Create a new empty path
    pub fn new() -> Self;

    /// Get the path as a slice
    pub fn as_slice(&self) -> &[String];

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
pub fn logical_memory_to_folded<T: LogicalMemoryProfile>(value: &T) -> String;

/// Calculate total bytes used by a value
pub fn logical_total_bytes<T: LogicalMemoryProfile>(value: &T) -> usize;
```

## Implementation Patterns

### Critical Implementation Rule

**Never count struct size with `size_of::<Self>()`** - this causes double-counting when the struct contains other structs as fields.

Instead, count or delegate each field individually:
- Primitive types: count with `size_of::<T>()`
- Nested structs: delegate via `visit_logical_memory()`
- Collections: count stack overhead + elements separately

### Pattern 0: Using the Macro (Recommended)

**Most common case**: For structs that simply delegate to their fields, use the macro:

```rust
// Simple type
crate::impl_logical_memory_profile! {
    DecisionVariableMetadata {
        name,
        subscripts,
        parameters,
        description,
    }
}

// Type with path (e.g., protobuf types)
crate::impl_logical_memory_profile! {
    v1::Parameters as "Parameters" {
        entries,
    }
}
```

**Macro features**:
- Automatically uses `Type.field` naming convention
- Delegates each field to its `LogicalMemoryProfile` implementation
- Two variants: simple types and types with path (using `as "TypeName"`)
- Eliminates boilerplate and ensures consistency

**When NOT to use the macro**:
- Type has fields that need manual counting (primitives)
- Type needs custom logic (e.g., computed values)
- Type is an enum with match pattern

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

### Pattern 2: Generic Collection Implementations

Collections have generic implementations that handle stack overhead and element delegation:

**Available generic implementations**:
- `Vec<T>` - Reports `Vec[stack]` + delegates to each element
- `HashMap<K, V>` - Reports `HashMap[stack]` + delegates to keys (`HashMap[key]`) and values
- `BTreeMap<K, V>` - Reports `BTreeMap[stack]` + delegates to keys (`BTreeMap[key]`) and values
- `FnvHashMap<K, V>` - Reports `FnvHashMap[stack]` + delegates to keys and values
- `BTreeSet<T>` - Reports `BTreeSet[stack]` + delegates to each element
- `Option<T>` - Reports `Option[additional stack]` (Some) or `Option[stack]` (None)
- `String` - Reports stack + heap length

**Example**: Using macro with collections (automatic):
```rust
// ConstraintHints now uses the macro
crate::impl_logical_memory_profile! {
    ConstraintHints {
        one_hot_constraints,  // Vec<OneHot> - automatically handled
        sos1_constraints,     // Vec<Sos1> - automatically handled
    }
}
```

**Output format** (note the `[stack]` suffix):
```
ConstraintHints.one_hot_constraints;Vec[stack] 24
ConstraintHints.sos1_constraints;Vec[stack] 24
```

### Pattern 3: Naming Convention - Type.field

All implementations use the `Type.field` naming convention for clarity:

```rust
// DecisionVariable delegates to each field with Type.field naming
visitor.visit_leaf(&path.with("DecisionVariable.id"), size_of::<VariableID>());
visitor.visit_leaf(&path.with("DecisionVariable.kind"), size_of::<Kind>());
self.metadata.visit_logical_memory(
    path.with("DecisionVariable.metadata").as_mut(),
    visitor
);
```

**Output format**:
```
DecisionVariable.id 8
DecisionVariable.kind 1
DecisionVariable.metadata;DecisionVariableMetadata.name 26
```

**Benefits**:
- Flamegraph frames show both type and field names
- Easier to understand hierarchy at a glance
- Consistent across all implementations

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

- **Collections**: Generic implementations handle stack (`[stack]`) + heap content (ignore unused capacity)
  - Vec: Reports `Vec[stack]` + delegates to elements
  - HashMap/BTreeMap: Reports `Map[stack]` + `Map[key]` + delegates to values
  - String: Reports stack + heap (`size_of::<String>() + len()`)

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
Instance.constraint_hints;ConstraintHints.one_hot_constraints;Vec[stack] 24
Instance.constraint_hints;ConstraintHints.sos1_constraints;Vec[stack] 24
Instance.constraints;BTreeMap[stack] 24
Instance.decision_variable_dependency;AcyclicAssignments.assignments;FnvHashMap[stack] 32
Instance.decision_variable_dependency;AcyclicAssignments.dependency 144
Instance.decision_variables;BTreeMap[key] 24
Instance.decision_variables;BTreeMap[stack] 24
Instance.decision_variables;DecisionVariable.bound 48
Instance.decision_variables;DecisionVariable.id 24
Instance.decision_variables;DecisionVariable.kind 3
Instance.decision_variables;DecisionVariable.metadata;DecisionVariableMetadata.description;Option[stack] 72
Instance.decision_variables;DecisionVariable.metadata;DecisionVariableMetadata.name;Option[stack] 72
Instance.decision_variables;DecisionVariable.metadata;DecisionVariableMetadata.parameters;FnvHashMap[stack] 96
Instance.decision_variables;DecisionVariable.metadata;DecisionVariableMetadata.subscripts;Vec[stack] 72
Instance.decision_variables;DecisionVariable.substituted_value;Option[stack] 48
Instance.description;Option[stack] 96
Instance.objective;Linear;PolynomialBase.terms 80
Instance.parameters;Option[stack] 48
Instance.removed_constraints;BTreeMap[stack] 24
Instance.sense 1
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
- `Path` type with RAII guards (no root name, starts empty)
- `PathGuard` for automatic path management
- `LogicalMemoryProfile` trait
- `LogicalMemoryVisitor` trait
- `FoldedCollector` for folded stack generation
- Helper functions: `logical_memory_to_folded()`, `logical_total_bytes()`
- **`impl_logical_memory_profile!` macro** - Auto-generates implementations for field delegation
  - Simple variant: `Type { fields... }`
  - Path variant: `path::to::Type as "TypeName" { fields... }`

✅ **Generic Collection Implementations** (`rust/ommx/src/logical_memory/collections.rs`):
- `String` - Stack + heap length
- `Option<T>` - `[additional stack]` (Some) or `[stack]` (None)
- `Vec<T>` - `Vec[stack]` + element delegation
- `HashMap<K, V>`, `BTreeMap<K, V>`, `FnvHashMap<K, V>` - `Map[stack]` + `Map[key]` + value delegation
- `BTreeSet<T>` - `BTreeSet[stack]` + element delegation

✅ **Domain Type Implementations** (all using macro where possible):
- `Instance` - Root type with 9 fields (uses macro)
- `v1::Parameters`, `v1::instance::Description` - Protobuf types (uses macro with path variant)
- `Function` enum (Linear/Quadratic/Polynomial/Zero) - Custom implementation
- `PolynomialBase<M>` - Generic polynomial implementation
- `DecisionVariable`, `DecisionVariableMetadata` - Variables (uses macro)
- `Constraint`, `RemovedConstraint` - Constraint types (uses macro)
- `ConstraintHints`, `OneHot`, `Sos1` - Constraint hints (uses macro)
- `AcyclicAssignments` - Variable dependencies (partial macro use)
- **Basic types**: `Sense`, `Equality`, `Kind`, `Bound`, `ConstraintID`, `VariableID` - Primitives

✅ **Python Bindings** (`python/ommx/src/instance.rs`):
- `Instance.logical_memory_profile()` method
- Type stubs generation
- Comprehensive documentation with examples

✅ **Testing**:
- 35 Rust unit tests with snapshot testing
- 390 Python tests including doctests
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
⚠️ **Stack vs heap**: Stack memory shown as `[stack]` suffix, actual heap profilers may show different allocations

## Related Resources

- [flamegraph.pl](https://github.com/brendangregg/FlameGraph)
- [inferno (Rust flamegraph tool)](https://github.com/jonhoo/inferno)
- [Brendan Gregg's Blog on Flamegraphs](https://www.brendangregg.com/flamegraphs.html)

## Revision History

- 2025-11-20: Initial design with visitor-based approach
- 2025-11-20: Fixed double-counting by eliminating `size_of::<Self>()` usage
- 2025-11-20: Introduced `Path` and `PathGuard` RAII pattern for automatic path management
- 2025-11-20: Completed implementation with Python API and comprehensive testing
- 2025-11-21: Removed root_name from Path::new(), now starts with empty path
- 2025-11-21: Renamed `[overhead]` → `[stack]` for clarity
- 2025-11-21: Improved Option<T> to show `[additional stack]` (Some) vs `[stack]` (None)
- 2025-11-21: Introduced `impl_logical_memory_profile!` macro for boilerplate reduction
- 2025-11-21: Extended macro to support type paths (`v1::Parameters as "Parameters"`)
- 2025-11-21: Implemented generic collection types (Vec, HashMap, BTreeMap, BTreeSet, Option, String)
- 2025-11-21: Adopted `Type.field` naming convention throughout
- 2025-11-21: Converted 13 types to use macro, added 8 basic type implementations
