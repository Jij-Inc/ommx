# Tutorial

A guided tour of the `ommx` crate's public API. Each submodule below
covers one piece of the API surface — start with
[`expressions`](tutorial/expressions/index.html) if this is your first
read, or jump straight into the topic you need.

## Building a problem

- [`expressions`](tutorial/expressions/index.html) — polynomial
  expressions via [`Linear`](crate::Linear), [`Quadratic`](crate::Quadratic),
  [`Polynomial`](crate::Polynomial), and [`Function`](crate::Function).
- [`decision_variables`](tutorial/decision_variables/index.html) —
  [`DecisionVariable`](crate::DecisionVariable) with
  [`Kind`](crate::Kind) and [`Bound`](crate::Bound).
- [`constraints`](tutorial/constraints/index.html) —
  [`Constraint`](crate::Constraint) and the stage-parameterized
  constraint type system.
- [`instance`](tutorial/instance/index.html) — assembling a complete
  [`Instance`](crate::Instance).

## Working with a problem

- [`evaluate`](tutorial/evaluate/index.html) — the
  [`Evaluate`](crate::Evaluate) trait.
- [`solution`](tutorial/solution/index.html) —
  [`Solution`](crate::Solution) and [`SampleSet`](crate::SampleSet).
- [`substitute`](tutorial/substitute/index.html) — the
  [`Substitute`](crate::Substitute) trait.

## Infrastructure

- [`error_handling`](tutorial/error_handling/index.html) —
  [`ommx::Result`](crate::Result), signal types, and the fail-site
  macros [`bail!`](crate::bail) / [`error!`](crate::error!) /
  [`ensure!`](crate::ensure).
