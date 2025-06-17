# Python SDK v1 to v2 Migration Guide

This document is a guide for migrating the OMMX Python SDK from Protocol Buffer-based (v1) to Rust-PyO3-based (v2).

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
    Solution, State
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

## Recommended Implementation Patterns

```python
# Unified imports
from ommx.v1 import (
    Instance, DecisionVariable, Constraint,
    Function, Linear, Solution, State
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
- ⏳ **Planned**: `ommx.v1.Solution` and `ommx.v1.SampleSet` migrations

---

**Note**: v2 API migration is ongoing. PyO3 performance optimization in progress for core data structures.