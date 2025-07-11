# Python SDK v1 to v2 Migration Guide

This document is a guide for migrating the OMMX Python SDK from Protocol Buffer-based (v1) to Rust-PyO3-based (v2).

## ⚠️ Important: Deprecation of `raw` Attributes

In v2, the `raw` attribute is deprecated across all migrated classes (`Instance`, `Solution`, `SampleSet`, etc.). Direct access to `.raw` should be avoided. Instead, use the methods directly available on the classes themselves:

**Examples:**
```python
# ❌ Deprecated: Don't access through raw
solution.raw.evaluated_constraints[0].evaluated_value
instance.raw.decision_variables
sample_set.raw.samples

# ✅ Recommended: Use direct properties and methods
solution.get_constraint_value(0)
instance.decision_variables  # Now a property returning a list
sample_set.get(sample_id)
```

These direct methods provide:
- Better performance through native Rust implementation
- Improved type safety
- Cleaner, more intuitive APIs

While `raw` attributes remain available for backward compatibility, they will be removed in future versions.

## Import Changes

**Old approach (v1)**:
```python
from ommx.v1.constraint_pb2 import Constraint, Equality
from ommx.v1.function_pb2 import Function
from ommx.v1.linear_pb2 import Linear
from ommx.v1 import Instance, DecisionVariable
```

**New approach (v2) - Recommended**:
```python
# Import everything uniformly from ommx.v1
from ommx.v1 import (
    Instance, DecisionVariable, Constraint,
    Function, Linear, Quadratic, Polynomial,
    Solution, State, SampleSet,
    # New evaluated types (v2.0.0rc3+)
    EvaluatedDecisionVariable, EvaluatedConstraint,
    SampledDecisionVariable, SampledConstraint
)
```

## DecisionVariable Factory Methods

**Still available**:
```python
# of_type method (still available)
DecisionVariable.of_type(
    DecisionVariable.BINARY, var.idx, 
    lower=var.lb, upper=var.ub, name=var.name
)
```

**Newly added methods**:
```python
# More concise type-specific factory methods
DecisionVariable.binary(var.idx, name=var.name)
DecisionVariable.integer(var.idx, lower=var.lb, upper=var.ub, name=var.name)  
DecisionVariable.continuous(var.idx, lower=var.lb, upper=var.ub, name=var.name)
```

## Function Creation

**Old approach**:
```python
# Direct Protocol Buffer creation
Function(constant=constant)
Function(linear=Linear(terms=terms, constant=constant))
```

**New approach**:
```python
# Unified constructor
Function(constant)  # Create from scalar value
Function(linear)    # Create from Linear object
Function(quadratic) # Create from Quadratic object

# Creating Linear objects
linear = Linear(terms=terms, constant=constant)
```

## Constraint Creation

**Old approach**:
```python
# Direct Protocol Buffer creation
Constraint(
    id=id,
    equality=Equality.EQUALITY_EQUAL_TO_ZERO,
    function=function,
    name=name,
)
```

**New approach**:
```python
# Direct constructor creation (using ommx.v1.Function)
constraint = Constraint(
    id=id,
    function=function,  # Use ommx.v1.Function
    equality=Constraint.EQUAL_TO_ZERO,  # Use Python SDK constants
    name=name,
)
```

## Function Inspection and Conversion

**Old approach**:
```python
# Protocol Buffer HasField check
if function.HasField("linear"):
    linear_terms = function.linear.terms
    constant = function.linear.constant
```

**New approach**:
```python
# Check polynomial degree using Function.degree() and use direct property access
degree = function.degree()
if degree == 0:
    # Constant function
    constant = function.constant_term
elif degree == 1:
    # Linear function - direct property access
    linear_terms = function.linear_terms      # dict[int, float]
    constant = function.constant_term         # float
elif degree == 2:
    # Quadratic function - direct property access
    quadratic_terms = function.quadratic_terms  # dict[tuple[int, int], float]
    linear_terms = function.linear_terms        # dict[int, float]
    constant = function.constant_term           # float

# Real adapter usage example (PySCIPOpt):
def _make_linear_expr(self, f: Function) -> pyscipopt.Expr:
    return (
        pyscipopt.quicksum(
            coeff * self.varname_map[str(id)]
            for id, coeff in f.linear_terms.items()
        )
        + f.constant_term
    )

def _make_quadratic_expr(self, f: Function) -> pyscipopt.Expr:
    # Quadratic terms
    quad_terms = pyscipopt.quicksum(
        self.varname_map[str(row)] * self.varname_map[str(col)] * coeff
        for (row, col), coeff in f.quadratic_terms.items()
    )
    # Linear terms
    linear_terms = pyscipopt.quicksum(
        coeff * self.varname_map[str(var_id)]
        for var_id, coeff in f.linear_terms.items()
    )
    return quad_terms + linear_terms + f.constant_term
```


## Migration Steps

1. **Update imports**: Remove direct Protocol Buffer imports (`*_pb2`) and change to unified imports from `ommx.v1`
2. **Change Function inspection**: Replace `.HasField()` with `.degree()` checks and direct property access
3. **Utilize new methods**: More concise type-specific factory methods (`binary()`, `integer()`, `continuous()`) are available

## Common Issues and Solutions

- **`AttributeError: 'builtins.Function' object has no attribute 'HasField'`**: Use `.degree()` check followed by direct property access (`.linear_terms`, `.constant_term`, etc.)
- **`TypeError: 'float' object is not callable`**: Access `function.constant_term` as a property, not `function.constant_term()`
- **Using `.raw` attributes**: The `raw` attribute is deprecated. Use methods directly available on the classes (e.g., `solution.get_constraint_value()`, `instance.decision_variables`) for better performance and type safety

## Important Notes

- Import everything uniformly from `ommx.v1` and avoid direct Protocol Buffer imports
- When determining constraint types, check in order of increasing degree (constant → linear → quadratic)

## Newly Available Methods

### Function Class
```python
# Information retrieval
function.degree() -> int      # Function degree
function.num_terms() -> int   # Number of terms

# Direct property access (recommended)
function.constant_term      # float - constant term
function.linear_terms       # dict[int, float] - linear term coefficients
function.quadratic_terms    # dict[tuple[int, int], float] - quadratic term coefficients

# Evaluation
function.evaluate(state: State | dict[int, float]) -> float
function.partial_evaluate(state: State | dict[int, float]) -> Function
```

### Solution Class (v2.0.0rc3+)
```python
# New list-based properties (consistent with Instance)
solution.decision_variables  # list[EvaluatedDecisionVariable] - sorted by ID
solution.constraints        # list[EvaluatedConstraint] - sorted by ID

# New individual access methods
solution.get_decision_variable_by_id(variable_id: int) -> EvaluatedDecisionVariable
solution.get_constraint_by_id(constraint_id: int) -> EvaluatedConstraint
```

### SampleSet Class (v2.0.0rc3+)
```python
# New list-based properties (consistent with Instance)
sample_set.decision_variables # list[SampledDecisionVariable] - sorted by ID
sample_set.constraints       # list[SampledConstraint] - sorted by ID

# New individual access methods
sample_set.get_sample_by_id(sample_id: int) -> Solution  # Alias for get()
sample_set.get_decision_variable_by_id(variable_id: int) -> SampledDecisionVariable
sample_set.get_constraint_by_id(constraint_id: int) -> SampledConstraint
```

## Recommended Implementation Patterns

```python
# Unified imports (v2.0.0rc3+)
from ommx.v1 import (
    Instance, DecisionVariable, Constraint,
    Function, Linear, Solution, State, SampleSet,
    # New evaluated types for consistent API access
    EvaluatedDecisionVariable, EvaluatedConstraint,
    SampledDecisionVariable, SampledConstraint
)

# DecisionVariable creation (new factory methods)
var1 = DecisionVariable.binary(0, name="x1")
var2 = DecisionVariable.integer(1, lower=0, upper=10, name="x2")

# Function inspection (direct property access)
if objective.degree() == 1:
    terms = objective.linear_terms      # dict[int, float]
    constant = objective.constant_term  # float
elif objective.degree() == 2:
    linear_terms = objective.linear_terms        # dict[int, float]
    quadratic_terms = objective.quadratic_terms  # dict[tuple[int, int], float]
    constant = objective.constant_term           # float

# Consistent API patterns across all classes (v2.0.0rc3+)
# All three classes now follow the same pattern:

# Instance
for var in instance.decision_variables:  # list[DecisionVariable]
    process_variable(var.id, var)
var = instance.get_decision_variable_by_id(var_id)  # DecisionVariable

# Solution  
for var in solution.decision_variables:  # list[EvaluatedDecisionVariable]
    process_evaluated_variable(var.id, var.value)
var = solution.get_decision_variable_by_id(var_id)  # EvaluatedDecisionVariable

# SampleSet
for var in sample_set.decision_variables:  # list[SampledDecisionVariable] 
    process_sampled_variable(var.id, var.samples)
var = sample_set.get_decision_variable_by_id(var_id)  # SampledDecisionVariable
```

## State Constructor Changes (PyO3 Migration)

**Enhancement**: `State(entries=...)` constructor enhanced to accept both `dict[int, float]` and `Iterable[tuple[int, float]]`.

**Before (Protobuf)**:
```python
# These worked with protobuf State
state = State(entries=zip(variables, values))  # ✅ Worked
state = State(entries=[(1, 0.5), (2, 1.0)])   # ✅ Worked
```

**After (PyO3) - Enhanced Constructor**:
```python
# All these patterns now work with enhanced PyO3 State constructor
state = State(entries=zip(variables, values))        # ✅ Works with iterables
state = State(entries=[(1, 0.5), (2, 1.0)])         # ✅ Works with iterables  
state = State(entries=dict(zip(variables, values)))  # ✅ Works with dict
state = State(entries={1: 0.5, 2: 1.0})             # ✅ Works with dict
```

**Adapter Code Example**:
```python
# In adapter code (e.g., ommx-openjij-adapter)
def decode_to_samples(response: oj.Response) -> Samples:
    # Both patterns now work with enhanced PyO3 State:
    state = State(entries=zip(response.variables, sample))           # ✅ Works directly
    # OR
    state = State(entries=dict(zip(response.variables, sample)))     # ✅ Also works
```

**Migration Status**:
- ✅ **Completed**: `ommx.v1.State` migrated to PyO3 `_ommx_rust.State`
- ✅ **Completed**: Enhanced State constructor to accept both dict and iterables
- ✅ **Completed**: Adapter compatibility fixes for State constructor changes
  - ✅ OpenJij adapter: Compatible with both `zip()` and `dict(zip())` patterns
  - ✅ PyScipOpt adapter: Enhanced `to_state()` function for protobuf/PyO3 compatibility
  - ✅ Enhanced `ToState` type alias to include legacy protobuf State
- ✅ **Completed**: `ommx.v1.Solution` migrated to PyO3 `_ommx_rust.Solution`
- ✅ **Completed**: `ommx.v1.SampleSet` migrated to PyO3 `_ommx_rust.SampleSet`

## Solution API Changes

**Enhancement**: `Solution` now provides direct methods for accessing constraints and dual variables.

### Accessing Constraint Values

**Before (Protobuf)**:
```python
# Direct access to evaluated_constraints list
solution.raw.evaluated_constraints[0].evaluated_value  # ❌ No longer available
```

**After (PyO3)**:
```python
# Use new getter methods
solution.get_constraint_value(0)  # ✅ Get constraint value by ID
```

### Managing Dual Variables

**Before (Protobuf)**:
```python
# Direct modification of constraint objects
for constraint in solution.raw.evaluated_constraints:
    constraint.dual_variable = dual_variables[constraint.id]  # ❌ No longer available
```

**After (PyO3)**:
```python
# Use new setter/getter methods
solution.set_dual_variable(constraint_id, dual_value)  # ✅ Set dual variable by ID
solution.get_dual_variable(constraint_id)             # ✅ Get dual variable by ID
```

### Accessing Constraint IDs

**Before (Protobuf)**:
```python
# Iterate through evaluated_constraints
for constraint in solution.raw.evaluated_constraints:
    id = constraint.id  # ❌ No longer available
```

**After (PyO3)**:
```python
# Use constraint_ids property
for constraint_id in solution.constraint_ids:  # ✅ Returns set of constraint IDs
    value = solution.get_constraint_value(constraint_id)
```

### Adapter Implementation Examples

**HiGHS Adapter**:
```python
# Old approach
for constraint in solution.raw.evaluated_constraints:
    if constraint.id < row_dual_len:
        constraint.dual_variable = row_dual[constraint.id]

# New approach
for constraint_id in solution.constraint_ids:
    if constraint_id < row_dual_len:
        solution.set_dual_variable(constraint_id, row_dual[constraint_id])
```

**Python-MIP Adapter**:
```python
# Old approach
for constraint in solution.raw.evaluated_constraints:
    id = constraint.id
    if id in dual_variables:
        constraint.dual_variable = dual_variables[id]

# New approach
for constraint_id, dual_value in dual_variables.items():
    solution.set_dual_variable(constraint_id, dual_value)
```

### Complete Solution API Reference

```python
# Properties
solution.objective           # float - objective value
solution.constraint_ids      # set[int] - all constraint IDs
solution.decision_variable_ids  # set[int] - all decision variable IDs
solution.feasible           # bool - feasibility status
solution.feasible_relaxed   # bool - relaxed feasibility status

# New list-based properties (v2.0.0rc3+)
solution.decision_variables  # list[EvaluatedDecisionVariable] - sorted by ID
solution.constraints        # list[EvaluatedConstraint] - sorted by ID

# Methods
solution.get_constraint_value(constraint_id: int) -> float
solution.get_dual_variable(constraint_id: int) -> Optional[float]
solution.set_dual_variable(constraint_id: int, value: Optional[float]) -> None
solution.extract_decision_variables(name: str) -> dict[tuple[int, ...], float]
solution.extract_constraints(name: str) -> dict[tuple[int, ...], float]

# New individual access methods (v2.0.0rc3+)
solution.get_decision_variable_by_id(variable_id: int) -> EvaluatedDecisionVariable
solution.get_constraint_by_id(constraint_id: int) -> EvaluatedConstraint

# State access (backward compatible)
solution.state              # State object with variable values
solution.state.entries      # dict[int, float] - variable ID to value mapping
```

### Solution API Consistency Improvements (v2.0.0rc3+)

**Enhancement**: Solution API now follows the same patterns as Instance for consistency.

**New Properties**:
```python
# List-based access (consistent with Instance)
solution.decision_variables  # list[EvaluatedDecisionVariable] - sorted by ID
solution.constraints        # list[EvaluatedConstraint] - sorted by ID

# Individual access by ID (consistent with Instance)
solution.get_decision_variable_by_id(variable_id: int) -> EvaluatedDecisionVariable
solution.get_constraint_by_id(constraint_id: int) -> EvaluatedConstraint
```

**Migration Pattern**:
```python
# Before: Direct access to constraint values
solution.get_constraint_value(constraint_id)
solution.get_dual_variable(constraint_id)

# After: Access through constraint objects (alternative pattern)
constraint = solution.get_constraint_by_id(constraint_id)
value = constraint.evaluated_value
dual_var = constraint.dual_variable

# Both patterns are supported for backward compatibility
```

## SampleSet API Reference

The `SampleSet` class now provides direct methods for accessing samples and extracting data:

```python
# Properties
sample_set.sample_ids         # set[int] - all sample IDs
sample_set.feasible_ids       # set[int] - feasible sample IDs
sample_set.best_feasible_id   # Optional[int] - best feasible sample ID
sample_set.best_feasible      # Optional[Solution] - best feasible solution

# New list-based properties (v2.0.0rc3+)
sample_set.decision_variables # list[SampledDecisionVariable] - sorted by ID
sample_set.constraints       # list[SampledConstraint] - sorted by ID

# Methods
sample_set.get(sample_id: int) -> Solution
sample_set.extract_decision_variables(name: str, sample_id: int) -> dict[tuple[int, ...], float]
sample_set.extract_constraints(name: str, sample_id: int) -> dict[tuple[int, ...], float]

# New individual access methods (v2.0.0rc3+)
sample_set.get_sample_by_id(sample_id: int) -> Solution  # Alias for get()
sample_set.get_decision_variable_by_id(variable_id: int) -> SampledDecisionVariable
sample_set.get_constraint_by_id(constraint_id: int) -> SampledConstraint
```

### SampleSet API Consistency Improvements (v2.0.0rc3+)

**Enhancement**: SampleSet API now follows the same patterns as Instance and Solution for consistency.

**New Properties**:
```python
# List-based access (consistent with Instance and Solution)
sample_set.decision_variables # list[SampledDecisionVariable] - sorted by ID
sample_set.constraints       # list[SampledConstraint] - sorted by ID

# Individual access by ID (consistent with Instance and Solution)
sample_set.get_sample_by_id(sample_id: int) -> Solution  # Alias for existing get()
sample_set.get_decision_variable_by_id(variable_id: int) -> SampledDecisionVariable
sample_set.get_constraint_by_id(constraint_id: int) -> SampledConstraint
```

**API Consistency Achievement**:
All three core classes now follow the same pattern:
- **Instance**: `decision_variables` → `list[DecisionVariable]`, `get_decision_variable_by_id()` → `DecisionVariable`
- **Solution**: `decision_variables` → `list[EvaluatedDecisionVariable]`, `get_decision_variable_by_id()` → `EvaluatedDecisionVariable`  
- **SampleSet**: `decision_variables` → `list[SampledDecisionVariable]`, `get_decision_variable_by_id()` → `SampledDecisionVariable`

Same patterns apply to constraints and other access methods.

## Adapter API Changes (v2.0-rc.4)

**Breaking Change**: Instance API methods changed from methods to properties and return types changed from dictionaries to lists.

### Instance API Changes

**Before**:
```python
# Methods returning dictionaries
for var_id, var in instance.decision_variables().items():
    process_variable(var_id, var)

for constraint_id, constraint in instance.constraints().items():
    process_constraint(constraint_id, constraint)

# Raw access for iteration
for var_id, var in instance.raw.decision_variables.items():
    process_variable(var_id, var)
```

**After**:
```python
# Properties returning lists (ID-sorted)
for var in instance.decision_variables:
    process_variable(var.id, var)

for constraint in instance.constraints:
    process_constraint(constraint.id, constraint)

# No raw access needed - use properties directly
for var in instance.decision_variables:
    process_variable(var.id, var)
```

### Instance Sense Access

**Before**:
```python
if instance.raw.sense == Instance.MAXIMIZE:
    # Handle maximize
elif instance.raw.sense == Instance.MINIMIZE:
    # Handle minimize
```

**After**:
```python
if instance.sense == Instance.MAXIMIZE:
    # Handle maximize
elif instance.sense == Instance.MINIMIZE:
    # Handle minimize
```

### Adapter Implementation Updates

**Python-MIP Adapter**:
```python
# Before
def _set_decision_variables(self):
    for var_id, var in self.instance.raw.decision_variables.items():
        # Process variable

# After
def _set_decision_variables(self):
    for var in self.instance.decision_variables:
        # Process variable using var.id and var properties
```

**State Creation Pattern**:
```python
# Before
return State(entries={
    var_id: data.var_by_name(str(var_id)).x
    for var_id, var in self.instance.raw.decision_variables.items()
})

# After
return State(entries={
    var.id: data.var_by_name(str(var.id)).x
    for var in self.instance.decision_variables
})
```

### New Instance Methods

**Individual Access**:
```python
# Get specific items by ID (with KeyError on missing ID)
var = instance.get_decision_variable_by_id(variable_id)  # Individual variable access
constraint = instance.get_constraint_by_id(constraint_id)  # Individual constraint access
removed_constraint = instance.get_removed_constraint(constraint_id)  # Individual removed constraint access
```

### Required Adapter Changes

1. **Replace `.raw` access**: Use direct properties instead of `.raw.decision_variables.items()`
2. **Update iteration patterns**: Change from `dict.items()` to direct list iteration
3. **Access individual IDs**: Use `.id` property on each object instead of dict keys
4. **Update sense access**: Use `instance.sense` instead of `instance.raw.sense`
5. **Use new individual access methods**: Use `get_decision_variable_by_id()` and `get_constraint_by_id()` for individual item access

### Migration Checklist for Adapters

- [ ] Replace `instance.raw.decision_variables.items()` → `instance.decision_variables`
- [ ] Replace `instance.raw.constraints.items()` → `instance.constraints`
- [ ] Replace `instance.raw.sense` → `instance.sense`
- [ ] Update variable access from `(var_id, var)` → `var` (use `var.id`)
- [ ] Update constraint access from `(constraint_id, constraint)` → `constraint` (use `constraint.id`)
- [ ] Update test assertions from `len(instance.decision_variables())` → `len(instance.decision_variables)`
- [ ] Use `instance.get_decision_variable_by_id(id)` for individual variable access
- [ ] Use `instance.get_constraint_by_id(id)` for individual constraint access

---

**Note**: v2 API migration is complete. All core data structures now use PyO3 for improved performance.