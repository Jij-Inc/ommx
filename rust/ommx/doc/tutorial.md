# Tutorial

A guided tour of the `ommx` crate's public API. Each submodule below
covers one piece of the API surface — start with
[`expressions`](crate::doc::tutorial::expressions) if this is your first
read, or jump straight into the topic you need.

## Building a problem

- [`expressions`](crate::doc::tutorial::expressions) — polynomial
  expressions via [`Linear`](crate::Linear), [`Quadratic`](crate::Quadratic),
  [`Polynomial`](crate::Polynomial), and [`Function`](crate::Function).
- [`decision_variables`](crate::doc::tutorial::decision_variables) —
  [`DecisionVariable`](crate::DecisionVariable) with
  [`Kind`](crate::Kind) and [`Bound`](crate::Bound).
- [`constraints`](crate::doc::tutorial::constraints) —
  [`Constraint`](crate::Constraint) and the stage-parameterized
  constraint type system.
- [`instance`](crate::doc::tutorial::instance) — assembling a complete
  [`Instance`](crate::Instance).

## Working with a problem

- [`evaluate`](crate::doc::tutorial::evaluate) — the
  [`Evaluate`](crate::Evaluate) trait.
- [`solution`](crate::doc::tutorial::solution) —
  [`Solution`](crate::Solution) and [`SampleSet`](crate::SampleSet).
- [`substitute`](crate::doc::tutorial::substitute) — the
  [`Substitute`](crate::Substitute) trait.

## Infrastructure

- [`error_handling`](crate::doc::tutorial::error_handling) —
  [`ommx::Result`](crate::Result), signal types, and the fail-site
  macros [`bail!`](crate::bail) / [`error!`](crate::error!) /
  [`ensure!`](crate::ensure).
