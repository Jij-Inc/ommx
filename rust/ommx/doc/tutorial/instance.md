# Instance

The [`Instance`](crate::Instance) type represents a complete optimization problem with objective, variables,
and constraints. All variables used in the objective and constraints must be defined in the
decision variables map.

```rust
use ommx::{Instance, DecisionVariable, VariableID, Constraint, ConstraintID, Function, Sense, Linear, linear, coeff};
use maplit::btreemap;
use std::collections::BTreeMap;

// Create decision variables
let decision_variables = btreemap! {
    VariableID::from(1) => DecisionVariable::binary(),
    VariableID::from(2) => DecisionVariable::continuous(),
};

// Create objective function: minimize x1 + 2*x2
let objective = Function::from((linear!(1) + (coeff!(2.0) * linear!(2))?)?);

// Create constraints
let constraints = btreemap! {
    // x1 + x2 = 1
    ConstraintID::from(1) => Constraint::equal_to_zero(
        Function::from(((linear!(1) + linear!(2))? + Linear::from(coeff!(-1.0)))?)
    ),
    // x2 <= 5
    ConstraintID::from(2) => Constraint::less_than_or_equal_to_zero(
        Function::from((linear!(2) + Linear::from(coeff!(-5.0)))?)
    ),
};

// Create the instance
let instance = Instance::new(
    Sense::Minimize,
    objective,
    decision_variables,
    constraints,
)?;

assert_eq!(instance.sense(), Sense::Minimize);
assert_eq!(instance.decision_variables().len(), 2);
assert_eq!(instance.constraints().len(), 2);
# Ok::<(), Box<dyn std::error::Error>>(())
```

The `new` method validates that all variable IDs used in the objective function and
constraints are defined in the decision variables map, returning an error if any
undefined variables are referenced.

## Incremental decision-variable construction

When the instance should allocate variable IDs, use its typed `new_*` methods.
Each method receives the complete bound, modeling label, optional fixed value,
and tolerance before changing the instance:

```rust
use ommx::{ATol, Bound, DecisionVariableLabel, Instance, Kind, VariableID};

let mut instance = Instance::default();
let id = instance.new_integer(
    Bound::new(0.2, 3.8)?,
    DecisionVariableLabel {
        name: Some("count".to_string()),
        ..Default::default()
    },
    Some(2.0),
    ATol::default(),
)?;

assert_eq!(id, VariableID::from(0));
assert_eq!(instance.decision_variables()[&id].kind(), Kind::Integer);
assert_eq!(
    instance.decision_variables()[&id].bound(),
    Bound::new(1.0, 3.0)?,
);
assert_eq!(instance.variable_labels().name(id), Some("count"));
assert_eq!(instance.fixed_decision_variable_value(id), Some(2.0));
# Ok::<(), Box<dyn std::error::Error>>(())
```

For `Integer` and `SemiInteger`, each finite bound side becomes the least or
greatest integer satisfying the requested bound under `atol`; an infinite side
remains unbounded. Bound membership uses the same one-sided
residual-feasibility rule as inequality constraints. If no integer satisfies
the bound, `new_integer` returns an error, while `new_semi_integer` uses `[0,
0]` to preserve the semi-integer zero alternative.

The normalized row, label, and fixed value are committed together under the
returned ID. Invalid bounds, inconsistent fixed values, and a maximum existing
decision-variable ID of `u64::MAX` that prevents assignment of a larger
automatic ID return [`DecisionVariableError`](crate::DecisionVariableError)
without leaving a partially created variable in the instance. Use
[`Instance::new_decision_variable`](crate::Instance::new_decision_variable)
when the kind is selected dynamically.
