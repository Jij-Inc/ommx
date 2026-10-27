---
jupytext:
  text_representation:
    extension: .md
    format_name: myst
    format_version: 0.13
    jupytext_version: 1.19.1
kernelspec:
  display_name: Python 3 (ipykernel)
  language: python
  name: python3
---

# Evaluating a State to obtain a Solution

A {class}`~ommx.State` maps variable IDs to values; a {class}`~ommx.Solution` is the result of evaluating that assignment against a particular Instance. **An assignment alone does not tell you whether it is feasible for a problem.** Solver output and manually supplied candidates can be checked with the same {meth}`~ommx.Instance.evaluate`.

For example, supply $x=1, y=0$ to a problem in which two items cannot be selected together.

```{code-cell} ipython3
from ommx import Instance, Optimality, State

instance = Instance.maximize()
x = instance.new_binary("x")
y = instance.new_binary("y")
instance.objective = x + y
instance.add_constraint(x * y == 0, "exclusive")

state = State({x.id: 1, y.id: 0})
solution = instance.evaluate(state)

assert solution.state.entries == {x.id: 1.0, y.id: 0.0}
assert solution.feasible
assert solution.objective == 1
assert solution.optimality == Optimality.Unspecified
```

You can also pass a dictionary such as `{x.id: 1, y.id: 0}` directly instead of a `State`.

## Read feasibility, objective value, and optimality separately

| Information | What it tells you | What it does not tell you by itself |
|---|---|---|
| {attr}`~ommx.Solution.feasible` | Whether variable domains and constraints are satisfied | Whether a better candidate exists |
| {attr}`~ommx.Solution.objective` | This candidate's objective value | Whether the candidate is feasible or optimal |
| {attr}`~ommx.Solution.optimality` | The recorded optimality status | `evaluate` itself does not prove optimality |

The candidate above is feasible, but `evaluate` has not searched other candidates, so optimality is `Unspecified`. An Adapter can attach optimality information reported by a solver; evaluation alone does not produce `Optimal`.

An infeasible candidate still has an objective value when its objective can be evaluated.

```{code-cell} ipython3
infeasible = instance.evaluate({x.id: 1, y.id: 1})
assert infeasible.objective == 2
assert not infeasible.feasible
```

Although this is a maximization problem, an infeasible candidate with objective 2 cannot be selected as a better solution than a feasible candidate with objective 1. Feasibility includes variable kinds and bounds, not just constraints.

Floating-point evaluation uses a tolerance. Specify it with `evaluate(..., atol=...)` and retrieve the value used from {attr}`~ommx.Solution.feasibility_atol`. See {meth}`~ommx.Instance.populate_state` for exact normalization and validation rules.

## Find the conditions that failed

A Solution retains variable and constraint definitions and metadata from the Instance, together with evaluated values.

```{code-cell} ipython3
infeasible.decision_variables_df()
```

The decision-variable DataFrame includes the assigned `value` alongside `kind`, `lower`, and `upper`. The constraint DataFrame contains evaluated expression values and feasibility.

```{code-cell} ipython3
infeasible.constraints_df()
```

For an ordinary constraint, `value` is the left-hand side $f(x)$. The condition is zero for an equality or nonpositive for an inequality, so `value` is not always a violation measure. Use `constraints_df(kind=...)` for Indicator, OneHot, and SOS1 results. Their meanings and evaluation are explained in [Special constraints](./special_constraints.md).

(evaluating-transformed-instances)=
## Read results from a transformed problem

When an Instance was transformed for a solver, **evaluate the returned State against that transformed Instance.** It retains information for reconstructing original variable values, checking constraints, and computing the reported objective. Passing the solver's State directly to the original Instance may not interpret auxiliary variables or dependencies correctly.

During evaluation, recorded fixed values and dependency expressions reconstruct variable values. Variables with neither, and unused by the current problem, are irrelevant; omitted values are filled with defaults. {ref}`Variable roles <instance-variable-roles>` explains reconstruction versus defaulting.

Distinguish these three questions:

- Can values be reconstructed or filled?
- Is the candidate feasible, including the original constraints?
- Can the solver's objective value and optimality be interpreted as the reported problem's objective value and optimality?

Constraints imposed in the current problem are active; constraints excluded from it but retained for evaluation are removed. [Transformed problems and removed constraints](./removed_constraints.md) explains {attr}`~ommx.Solution.feasible`, which includes removed constraints, {attr}`~ommx.Solution.feasible_relaxed`, which uses only active constraints, and their relationship to the output objective.

Use [SampleSet](./sample_set.md) to evaluate several candidates under the same rules. “Best among the obtained candidates” is also different from global optimality for the problem.
