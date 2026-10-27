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

(removed-constraints)=
# Transformed problems and removed constraints

Transforming an Instance changes the problem presented to a solver. Users may still want original variable values and objective values, and to know whether original constraints hold. OMMX separates **the current problem from retained information used to interpret results**.

Building on {ref}`Variable roles <instance-variable-roles>` and [State-to-Solution evaluation](./solution.md), this page explains feasibility and objective values after transformation. Operation-specific formulas and applicability conditions belong in the API Reference.

## Evaluate constraints excluded from the active problem

For example, temporarily remove $x\leq0$ from a problem maximizing $x\in\{0,1\}$.

```{code-cell} ipython3
from ommx import Instance

instance = Instance.maximize()
x = instance.new_binary("x")
instance.objective = x
limit = instance.add_constraint(x <= 0, "limit")
limit_id = limit.constraint_id

instance.relax_constraint(limit_id, "example.relax_limit")

assert limit_id not in instance.constraints
assert limit_id in instance.removed_constraints

solution = instance.evaluate({x.id: 1})
assert solution.feasible_relaxed
assert not solution.feasible
```

Constraints in `constraints` are active; those retained in `removed_constraints` are removed. Removed does not mean “never checked again.” The condition is no longer imposed on the solver's current problem, but OMMX can check whether a returned candidate satisfies it.

| Result | Conditions checked |
|---|---|
| {attr}`~ommx.Solution.feasible_relaxed` | Active constraints and variable kinds/bounds |
| {attr}`~ommx.Solution.feasible` | Both active and removed constraints, and variable kinds/bounds |

{attr}`~ommx.Solution.feasible_unrelaxed` is the same test as `feasible`. In this example, $x=1$ is allowed by the current problem but violates the retained $x\leq0$.

Values are reconstructed or filled according to the {ref}`Instance's rules <instance-variable-roles>`. If a variable appears only in removed constraints and is irrelevant, omitting its value checks the default assignment. To check a particular candidate against original constraints, supply that candidate's values explicitly.

## Removed does not necessarily mean relaxed

Several operations move constraints from active to removed.

| Example operation | Treatment in the current problem | Why retain the removed constraint? |
|---|---|---|
| Manual relaxation | Do not impose the condition | Check whether a candidate satisfies the excluded condition |
| Special-constraint lowering | Express the condition with ordinary constraints | Inspect results in terms of the original special constraint |
| Penalty method | Replace a constraint with an objective term | Check whether a penalty-based candidate satisfies the original constraint |
| Trivial-constraint elimination | Exclude an unnecessary active condition | Inspect which condition was removed |

The presence of removed constraints alone does not identify a transformation as equivalent or a relaxation, or establish preservation of optima. With a finite penalty weight, for example, a candidate violating an original constraint may be selected. Reconstructing values also does not imply original feasibility or optimality. Check each operation's guarantees, including domains and applicability conditions.

Indicator, OneHot, and SOS1 constraints also have active and removed collections.

| Constraint family | Active | Removed |
|---|---|---|
| Ordinary | {attr}`~ommx.Instance.constraints` | {attr}`~ommx.Instance.removed_constraints` |
| Indicator | {attr}`~ommx.Instance.indicator_constraints` | {attr}`~ommx.Instance.removed_indicator_constraints` |
| OneHot | {attr}`~ommx.Instance.one_hot_constraints` | {attr}`~ommx.Instance.removed_one_hot_constraints` |
| SOS1 | {attr}`~ommx.Instance.sos1_constraints` | {attr}`~ommx.Instance.removed_sos1_constraints` |

For example, {meth}`~ommx.Instance.convert_one_hot_to_constraint` replaces OneHot with an ordinary equality and retains the original OneHot as removed. Generated constraints carry provenance identifying their source. See each operation's API Reference for exact generated objects and removal-reason formats.

(active-and-output-objective)=
## Solver-facing and reported objective values

The solver-facing and user-facing objectives may also differ. For example, after negating a maximization objective for a solver that accepts only minimization, users still want values in the original maximization sense.

```{code-cell} ipython3
from ommx import Sense

objective_example = Instance.maximize()
reward = objective_example.new_binary("reward")
objective_example.objective = 3 * reward
objective_example.convert_active_objective(Sense.Minimize)

# 現在解くのは -3 * reward の最小化。
assert objective_example.sense == Sense.Minimize
assert objective_example.objective.evaluate({reward.id: 1}) == -3

# 評価結果は元の目的関数と最大化・最小化で返る。
reported = objective_example.evaluate({reward.id: 1})
assert reported.sense == Sense.Maximize
assert reported.objective == 3
```

{attr}`~ommx.Instance.objective` and {attr}`~ommx.Instance.sense` describe the active problem. When a transformation retains a reporting objective, {attr}`~ommx.Instance.output_objective` stores it and `evaluate` uses it. An ordinary newly constructed Instance has no separate output objective; evaluation uses the active objective.

Likewise, distinguish a penalty-inclusive objective minimized by the solver from the value reported in a Solution. A small `Solution.objective` alone does not establish a small penalty or satisfaction of original constraints. Check feasibility separately.

Optimality is separate information too. An equivalent objective-sign change and finite-weight penalization have different guarantees. {meth}`~ommx.Instance.map_active_optimality` defines how to interpret optimality reported for the active problem. Retaining an output objective is not itself an optimality proof.

## Inspect what was removed

On an Instance, `constraints_df(removed=True)` displays both active and removed constraints. `removed_reason` identifies the operation or caller-supplied reason for removal.

```{code-cell} ipython3
instance.constraints_df(removed=True)
```

In a Solution, both sets of constraints have been evaluated. Use `include=("removed_reason",)` to add the reason to removed rows.

```{code-cell} ipython3
solution.constraints_df(include=("removed_reason",))
```

Select special constraints with `kind="indicator"`, `kind="one_hot"`, or `kind="sos1"`. A {class}`~ommx.SampleSet` makes the same distinction for several candidates; `feasible` and `feasible_relaxed` return tests keyed by sample ID.

For manual relaxation and restoration of ordinary and Indicator constraints, see {meth}`~ommx.Instance.relax_constraint` / {meth}`~ommx.Instance.restore_constraint` and {meth}`~ommx.Instance.relax_indicator_constraint` / {meth}`~ommx.Instance.restore_indicator_constraint`. Restoration makes a constraint active again; it is not the inverse of the whole transformation and does not undo generated auxiliary variables, other constraints, or objective changes.
