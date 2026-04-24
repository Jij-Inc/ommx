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
    VariableID::from(1) => DecisionVariable::binary(VariableID::from(1)),
    VariableID::from(2) => DecisionVariable::continuous(VariableID::from(2)),
};

// Create objective function: minimize x1 + 2*x2
let objective = Function::from(linear!(1) + coeff!(2.0) * linear!(2));

// Create constraints
let constraints = btreemap! {
    // x1 + x2 = 1
    ConstraintID::from(1) => Constraint::equal_to_zero(
        Function::from(linear!(1) + linear!(2) + Linear::from(coeff!(-1.0)))
    ),
    // x2 <= 5
    ConstraintID::from(2) => Constraint::less_than_or_equal_to_zero(
        Function::from(linear!(2) + Linear::from(coeff!(-5.0)))
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
