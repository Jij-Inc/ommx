# `Solution` and `SampleSet`

[`Solution`](crate::Solution) represents a single result of optimization
(variable values, objective value, per-constraint evaluation, and
feasibility). [`SampleSet`](crate::SampleSet) represents a family of
such results for stochastic / sampling methods, keyed by
[`SampleID`](crate::SampleID).

Both are produced through the [`Evaluate`](crate::Evaluate) trait — the
same trait that covers functions and constraints — so you can feed an
[`Instance`](crate::Instance) either a single `State` or a
[`Sampled<State>`](crate::Sampled) and get the appropriate output back.

## Single state → `Solution`

```rust
use ommx::{Instance, DecisionVariable, VariableID, Constraint, ConstraintID, Function, Sense, Linear, Evaluate, ATol, linear, coeff};
use ommx::v1::State;
use maplit::btreemap;
use std::collections::{BTreeMap, HashMap};

// Create an instance with variables and constraints
let decision_variables = btreemap! {
    VariableID::from(1) => DecisionVariable::continuous(VariableID::from(1)),
    VariableID::from(2) => DecisionVariable::continuous(VariableID::from(2)),
};

let objective = Function::from(linear!(1) + coeff!(2.0) * linear!(2));

let constraints = btreemap! {
    // x1 + x2 <= 10
    ConstraintID::from(1) => Constraint::less_than_or_equal_to_zero(
        Function::from(linear!(1) + linear!(2) + Linear::from(coeff!(-10.0)))
    ),
    // x1 >= 1 (as -x1 + 1 <= 0)
    ConstraintID::from(2) => Constraint::less_than_or_equal_to_zero(
        Function::from(coeff!(-1.0) * linear!(1) + Linear::from(coeff!(1.0)))
    ),
};

let instance = Instance::new(
    Sense::Minimize,
    objective,
    decision_variables,
    constraints,
)?;

// Create a state with variable values that satisfy constraints
let state = State::from(HashMap::from([(1, 3.0), (2, 4.0)]));

// Evaluate the instance to get a solution
let solution = instance.evaluate(&state, ATol::default())?;

// Access solution properties
assert_eq!(*solution.objective(), 11.0); // 3 + 2*4 = 11
assert!(solution.feasible()); // All constraints satisfied

// Check evaluated constraints
let evaluated_constraints = solution.evaluated_constraints();
assert_eq!(evaluated_constraints.len(), 2);

// Constraint 1: x1 + x2 - 10 <= 0, evaluated to 3 + 4 - 10 = -3
let constraint1 = &evaluated_constraints[&ConstraintID::from(1)];
assert_eq!(constraint1.stage.evaluated_value, -3.0);
assert!(constraint1.stage.feasible); // -3 <= 0 ✓

// Constraint 2: -x1 + 1 <= 0, evaluated to -3 + 1 = -2
let constraint2 = &evaluated_constraints[&ConstraintID::from(2)];
assert_eq!(constraint2.stage.evaluated_value, -2.0);
assert!(constraint2.stage.feasible); // -2 <= 0 ✓
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Many states → `SampleSet`

For sampling-based solvers, evaluate the same `Instance` against a
[`Sampled<State>`](crate::Sampled) via
[`Instance::evaluate_samples`](crate::Evaluate::evaluate_samples). The
result is a [`SampleSet`](crate::SampleSet) that groups per-sample
objective values, evaluated constraints, and feasibility flags under
[`SampleID`](crate::SampleID) keys.

```rust
use ommx::{Instance, DecisionVariable, VariableID, Constraint, ConstraintID, Function, Sense, Linear, Evaluate, ATol, SampleID, Sampled, linear, coeff};
use ommx::v1::State;
use maplit::btreemap;
use std::collections::HashMap;

let decision_variables = btreemap! {
    VariableID::from(1) => DecisionVariable::continuous(VariableID::from(1)),
    VariableID::from(2) => DecisionVariable::continuous(VariableID::from(2)),
};
let objective = Function::from(linear!(1) + coeff!(2.0) * linear!(2));
let constraints = btreemap! {
    // x1 + x2 <= 10
    ConstraintID::from(1) => Constraint::less_than_or_equal_to_zero(
        Function::from(linear!(1) + linear!(2) + Linear::from(coeff!(-10.0)))
    ),
};
let instance = Instance::new(Sense::Minimize, objective, decision_variables, constraints)?;

// Three samples: the first two are feasible, the third violates x1 + x2 <= 10.
let samples: Sampled<State> = Sampled::new(
    [
        vec![SampleID::from(0)],
        vec![SampleID::from(1)],
        vec![SampleID::from(2)],
    ],
    vec![
        State::from(HashMap::from([(1, 3.0), (2, 4.0)])), // obj = 11
        State::from(HashMap::from([(1, 1.0), (2, 2.0)])), // obj = 5
        State::from(HashMap::from([(1, 8.0), (2, 5.0)])), // obj = 18, infeasible
    ],
)?;

let sample_set: ommx::SampleSet = instance.evaluate_samples(&samples, ATol::default())?;

// Per-sample feasibility and the best feasible sample.
assert_eq!(sample_set.is_sample_feasible(SampleID::from(0)), Some(true));
assert_eq!(sample_set.is_sample_feasible(SampleID::from(2)), Some(false));

let best = sample_set.best_feasible()?;   // Solution for the minimum-objective feasible sample
assert_eq!(*best.objective(), 5.0);
# Ok::<(), Box<dyn std::error::Error>>(())
```
