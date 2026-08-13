---
jupytext:
  text_representation:
    extension: .md
    format_name: myst
    format_version: 0.13
    jupytext_version: 1.19.1
kernelspec:
  display_name: ommx-update-books (3.9.23)
  language: python
  name: python3
---

# Special Constraints

In addition to regular constraints ({class}`~ommx.Constraint` — an equality or inequality over a {class}`~ommx.Function`), OMMX provides several constraint types frequently used in mathematical optimization as first-class citizens. This page introduces the following three special constraint types, their usage, and how to solve them with the PySCIPOpt Adapter.

- {class}`~ommx.IndicatorConstraint`: a conditional constraint driven by a binary variable
- {class}`~ommx.OneHotConstraint`: exactly one of a set of binary variables equals 1
- {class}`~ommx.Sos1Constraint`: at most one of a set of variables is non-zero

The examples below use the PySCIPOpt Adapter, as in [Solve a 0-1 Knapsack Problem with the PySCIPOpt Adapter](../tutorial/solve_with_ommx_adapter.md). Install it first:

```
pip install ommx-pyscipopt-adapter
```

The PySCIPOpt Adapter passes linear Indicator and SOS1 constraints through to SCIP's `addConsIndicator` / `addConsSOS1` (equality indicators are split into two inequality indicators). It does not accept OneHot directly. In the standard workflow, the caller gets the PySCIPOpt Adapter's recommended policy and uses `Instance.prepare()` to lower OneHot to a regular equality. See [Adapter Exact Inputs and Instance Preparation](./capability_model.md).

In addition to each first-class representation, the sections below describe the individual lowering APIs. Use the recommended policy with `Instance.prepare()` in the standard Adapter workflow; call these APIs directly when you need to choose an individual conversion explicitly or inspect its result in detail.

## IndicatorConstraint

An **indicator constraint** enforces a constraint $f(x) \leq 0$ (or $f(x) = 0$) only when a binary variable $z = 1$. When $z = 0$, the constraint is unconditionally satisfied.

Create an {class}`~ommx.IndicatorConstraint` from an existing {class}`~ommx.Constraint` by calling {meth}`Constraint.with_indicator() <ommx.Constraint.with_indicator>`. The indicator argument accepts a variable ID, a standalone {class}`~ommx.DecisionVariable`, or an {class}`~ommx.AttachedDecisionVariable`.

```{code-cell} ipython3
from ommx import Instance, DecisionVariable, Equality

z = DecisionVariable.binary(0, name="z")
x = DecisionVariable.continuous(1, lower=0, upper=10, name="x")

# z = 1 => x <= 5
ic = (x <= 5).with_indicator(z)
assert ic.indicator_variable_id == 0
assert ic.equality == Equality.LessThanOrEqualToZero
```

Add it to an instance by passing a `dict[int, IndicatorConstraint]` to the `indicator_constraints=` argument of {meth}`Instance.from_components <ommx.Instance.from_components>`.

```{code-cell} ipython3
instance = Instance.from_components(
    decision_variables=[z, x],
    objective=x,
    constraints={0: z == 1},       # fix z = 1
    indicator_constraints={0: ic}, # z = 1 => x <= 5
    sense=Instance.MAXIMIZE,
)
assert set(instance.indicator_constraints.keys()) == {0}
```

The PySCIPOpt Adapter accepts this linear Indicator constraint and passes it directly to SCIP.

```{code-cell} ipython3
from ommx_pyscipopt_adapter import OMMXPySCIPOptAdapter

solution = OMMXPySCIPOptAdapter.solve(instance)
# With z = 1, the constraint x <= 5 is active, so the maximum value of x is 5
assert abs(solution.objective - 5.0) < 1e-6
```

### Lowering Indicator to Big-M constraints

{meth}`Instance.convert_indicator_to_constraint(indicator_id) <ommx.Instance.convert_indicator_to_constraint>` rewrites an Indicator constraint $y = 1 \Rightarrow f(x) \leq 0$ as regular constraints using Big-M values computed from the bounds of $f(x)$. Unlike SOS1 lowering, it introduces no new indicator variable; it uses the variable already held by the `IndicatorConstraint` as $y$.

$$
f(x) + u y - u \leq 0, \qquad -f(x) - l y + l \leq 0
$$

where $u \geq \sup f(x)$ and $l \leq \inf f(x)$.

- For inequality ($\leq$) Indicators, only the upper side is considered and is emitted only when $u > 0$. If $u \leq 0$, the variable bounds already imply the original constraint, so no regular constraint is emitted.
- For equality ($= 0$) Indicators, the two sides are considered independently: the upper side is emitted when $u > 0$, and the lower side when $l < 0$.

```{code-cell} ipython3
indicator_constraint_ids = instance.convert_indicator_to_constraint(0)
assert len(indicator_constraint_ids) == 1
assert instance.indicator_constraints == {}
assert set(instance.removed_indicator_constraints.keys()) == {0}
```

Use {meth}`~ommx.Instance.convert_all_indicators_to_constraints` to convert every active Indicator constraint. Conversion fails before mutation if a required bound of $f(x)$ is non-finite or if $f(x)$ references a semi-continuous / semi-integer variable. The bulk API validates every target first, so if any target cannot be converted, it leaves the entire `Instance` unchanged.

## OneHotConstraint

A **one-hot constraint** over a set of binary variables $\{x_1, \ldots, x_n\}$ requires $\sum_i x_i = 1$ — i.e. exactly one of them is 1.

```{code-cell} ipython3
from ommx import OneHotConstraint

xs = [DecisionVariable.binary(i, name="x", subscripts=[i]) for i in range(3)]
oh = OneHotConstraint(variables=xs)
assert oh.variables == [0, 1, 2]
```

Each entry passed to `variables` may be a variable ID, a standalone {class}`~ommx.DecisionVariable`, or an {class}`~ommx.AttachedDecisionVariable`. This input is exposed as the `VariableIDLike` type alias because only the variable identity is consumed. When the constraint is inserted, the enclosing instance validates the referenced IDs and any constraint-specific requirement against its own decision variables. For OneHot, the referenced variables must exist and be binary. The constraint stores their IDs, which are available through `oh.variables`. Mathematically the constraint is equivalent to the linear equality $x_0 + x_1 + x_2 - 1 = 0$, but holding it as a first-class constraint lets supporting solvers (many MIP solvers accept one-hot natively) handle it efficiently.

```{code-cell} ipython3
values = [5.0, 10.0, 3.0]
instance_oh = Instance.from_components(
    decision_variables=xs,
    objective=sum(v * x for v, x in zip(values, xs)),
    constraints={},
    one_hot_constraints={0: oh},
    sense=Instance.MAXIMIZE,
)
assert set(instance_oh.one_hot_constraints.keys()) == {0}
```

### Lowering OneHot to a regular equality

{meth}`Instance.convert_one_hot_to_constraint(one_hot_id) <ommx.Instance.convert_one_hot_to_constraint>` rewrites the selected OneHot as the mathematically equivalent regular equality $x_0 + x_1 + x_2 - 1 = 0$ and returns the ID of the generated regular constraint. To demonstrate the individual API, this example calls it directly before passing the instance to the PySCIPOpt Adapter. Because the conversion produces a different exact input, check its applicability before solving.

```{code-cell} ipython3
one_hot_constraint_id = instance_oh.convert_one_hot_to_constraint(0)
assert isinstance(one_hot_constraint_id, int)
assert OMMXPySCIPOptAdapter.check_applicability(instance_oh).is_applicable
solution = OMMXPySCIPOptAdapter.solve(instance_oh)
# Exactly one of the three is chosen, so x_1 with the largest value 10 is selected
assert abs(solution.objective - 10.0) < 1e-6
```

The conversion mutates `instance_oh` in place. After explicit lowering, the OneHot constraint is removed and its conversion record remains in `removed_one_hot_constraints`. Solving does not perform this conversion. Use {meth}`~ommx.Instance.convert_all_one_hots_to_constraints` to convert every active OneHot constraint in one call.

```{code-cell} ipython3
assert instance_oh.one_hot_constraints == {}
assert len(instance_oh.constraints) == 1
assert set(instance_oh.removed_one_hot_constraints.keys()) == {0}
```

## Sos1Constraint

An **SOS1 (Special Ordered Set type 1)** constraint over a set of variables $\{x_1, \ldots, x_n\}$ requires that **at most one** of them be non-zero. It differs from one-hot in the following ways:

- One-hot requires $\sum x_i = 1$, so exactly one variable is non-zero.
- SOS1 permits up to one variable to be non-zero (zero variables non-zero is also allowed).
- SOS1 variables are not necessarily binary — continuous variables work too.

```{code-cell} ipython3
from ommx import Sos1Constraint

ys = [DecisionVariable.continuous(i, lower=0, upper=10, name="y", subscripts=[i]) for i in range(3, 6)]
s1 = Sos1Constraint(variables=ys)
assert s1.variables == [3, 4, 5]
```

As with `OneHotConstraint`, each `variables` entry accepts `VariableIDLike`: a variable ID, a standalone decision variable, or an attached decision variable. The SOS1 constraint stores their IDs, which are available through `s1.variables`.

```{code-cell} ipython3
instance_s1 = Instance.from_components(
    decision_variables=ys,
    objective=sum(ys),
    constraints={},
    sos1_constraints={0: s1},
    sense=Instance.MAXIMIZE,
)
assert set(instance_s1.sos1_constraints.keys()) == {0}
```

The PySCIPOpt Adapter accepts SOS1 constraints and passes them directly to SCIP.

```{code-cell} ipython3
solution = OMMXPySCIPOptAdapter.solve(instance_s1)
# Only one variable may be non-zero, so one is set to its upper bound 10 and the others to 0
assert abs(solution.objective - 10.0) < 1e-6
```

### Lowering SOS1 to Big-M constraints

{meth}`Instance.convert_sos1_to_constraints(sos1_id) <ommx.Instance.convert_sos1_to_constraints>` converts an SOS1 constraint to regular constraints using the Big-M method. For each variable $x_i \in [l_i, u_i]$, it applies these rules:

1. If $x_i$ is Binary with bounds $[0, 1]$, reuse the variable itself as its indicator.
2. Otherwise introduce a new Binary variable $y_i$ and emit $x_i - u_i y_i \leq 0$ and $l_i y_i - x_i \leq 0$. Trivial sides with $u_i = 0$ or $l_i = 0$ are omitted.
3. Finally, emit the cardinality constraint $\sum_i y_i - 1 \leq 0$.

```{code-cell} ipython3
sos1_constraint_ids = instance_s1.convert_sos1_to_constraints(0)
# Three upper Big-M constraints and one cardinality constraint
assert len(sos1_constraint_ids) == 4
assert instance_s1.sos1_constraints == {}
assert set(instance_s1.removed_sos1_constraints.keys()) == {0}
```

Use {meth}`~ommx.Instance.convert_all_sos1_to_constraints` to convert every active SOS1 constraint. Conversion fails before mutation if a non-Binary variable has a non-finite bound, its domain excludes $0$, or its kind is semi-continuous / semi-integer. The bulk API validates every target first, so if any target cannot be converted, it leaves the entire `Instance` unchanged.

## Independent ID spaces per constraint type

In OMMX, each of the four constraint collections — regular / Indicator / OneHot / SOS1 — has an **independent ID space**. The four dicts passed to {meth}`Instance.from_components <ommx.Instance.from_components>` are keyed independently, so using the same integer ID across different constraint types does not cause a collision.

For example, "regular constraint ID=1" and "Indicator constraint ID=1" coexist as distinct constraints.

```{code-cell} ipython3
z2 = DecisionVariable.binary(10, name="z2")
x2 = DecisionVariable.continuous(11, lower=0, upper=10, name="x2")

instance_mix = Instance.from_components(
    decision_variables=[z2, x2] + xs + ys,
    objective=x2,
    constraints={1: z2 == 1},                                        # regular ID=1
    indicator_constraints={1: (x2 <= 5).with_indicator(z2)},         # Indicator ID=1
    one_hot_constraints={1: OneHotConstraint(variables=xs)},         # OneHot ID=1
    sos1_constraints={1: Sos1Constraint(variables=ys)},              # SOS1 ID=1
    sense=Instance.MAXIMIZE,
)

# Each of the four dicts holds its own ID=1 constraint independently
assert set(instance_mix.constraints.keys()) == {1}
assert set(instance_mix.indicator_constraints.keys()) == {1}
assert set(instance_mix.one_hot_constraints.keys()) == {1}
assert set(instance_mix.sos1_constraints.keys()) == {1}
```

When a special constraint is lowered to a regular constraint, the generated regular constraint is allocated from the `Constraint` ID space. Only regular constraint IDs can collide after conversion.

## Auditing lowering results

The source special constraints are not discarded. They remain as removed entries in the following `removed_*_constraints` dicts.

| Original type | Removed dict | DataFrame |
|---|---|---|
| OneHotConstraint | {attr}`~ommx.Instance.removed_one_hot_constraints` | `instance.constraints_df(kind="one_hot", removed=True)` |
| Sos1Constraint | {attr}`~ommx.Instance.removed_sos1_constraints` | `instance.constraints_df(kind="sos1", removed=True)` |
| IndicatorConstraint | {attr}`~ommx.Instance.removed_indicator_constraints` | `instance.constraints_df(kind="indicator", removed=True)` |

With `removed=True`, the DataFrame includes both active and removed rows. It also automatically includes the `removed_reason` and `removed_reason.{key}` columns so that converted constraints remain distinguishable from active constraints.

Each removed entry ({class}`~ommx.RemovedOneHotConstraint` / {class}`~ommx.RemovedSos1Constraint` / {class}`~ommx.RemovedIndicatorConstraint`) records `removed_reason` and holds the generated regular-constraint IDs in `removed_reason_parameters`.

- **OneHot**: a single ID under `constraint_id`
- **SOS1**: a comma-separated list of IDs under `constraint_ids`
- **Indicator**: a comma-separated list of IDs under `constraint_ids`, or an empty string if both Big-M sides were redundant

```{code-cell} ipython3
removed = instance_oh.removed_one_hot_constraints
assert set(removed.keys()) == {0}
assert removed[0].removed_reason == "ommx.Instance.convert_one_hot_to_constraint"
```

Each generated regular constraint also keeps a source reference in {attr}`Constraint.provenance <ommx.Constraint.provenance>`. Every {class}`~ommx.Provenance` records the source {attr}`~ommx.Provenance.kind` and {attr}`~ommx.Provenance.original_id`, so you can trace a regular constraint back to its special-constraint type and ID.

```{code-cell} ipython3
from ommx import ProvenanceKind

generated = instance_oh.constraints[one_hot_constraint_id]
assert any(
    p.kind == ProvenanceKind.OneHotConstraint and p.original_id == 0
    for p in generated.provenance
)
```

## Accessing evaluation results

The {class}`~ommx.Solution` or {class}`~ommx.SampleSet` obtained after solving exposes a single {meth}`~ommx.Solution.constraints_df` method that dispatches on `kind=`:

| Constraint type | `kind=` value |
|---|---|
| Regular | `"regular"` (default) |
| Indicator | `"indicator"` |
| OneHot | `"one_hot"` |
| SOS1 | `"sos1"` |

```python
solution.constraints_df()                  # regular (default)
solution.constraints_df(kind="indicator")  # indicator
sample_set.constraints_df(kind="one_hot")  # one-hot
```

The DataFrame is indexed by the kind-qualified id column (`regular_constraint_id`, `indicator_constraint_id`, `one_hot_constraint_id`, `sos1_constraint_id`) — accidental cross-id-space `df.join()` mistakes surface in `df.head()` and friends.

The Indicator DataFrame includes an `indicator_active` column that disambiguates "the indicator was OFF (constraint trivially satisfied)" from "the indicator was ON and the constraint was actually satisfied". Indicator constraints do not carry a dual variable — a dual value is not well-defined for a conditional constraint — so `dual_variable` is omitted.

### Removed reason columns via `include=`

`removed_reason` is no longer a default column of {meth}`~ommx.Solution.constraints_df`. Pass `"removed_reason"` in `include=` to fold the reason name and the `removed_reason.{key}` parameter columns back in (rows whose constraint was not removed before evaluation get NA in those columns):

```python
df = solution.constraints_df(
    include=("label", "parameters", "removed_reason"),
)
```

The same applies to Indicator, OneHot, and SOS1: pass the corresponding `kind=` together with `"removed_reason"` in `include=`. The long-format {meth}`~ommx.Solution.constraint_removed_reasons_df` sidecar (also `kind=`-dispatched) remains the right surface when you want one row per (constraint id, parameter key) pair for joins or aggregation.

## Relax / Restore

{class}`~ommx.IndicatorConstraint` supports the same relax / restore workflow as regular constraints.

- {meth}`Instance.relax_indicator_constraint() <ommx.Instance.relax_indicator_constraint>`: relax (deactivate) an indicator constraint and record a reason string. The relaxed constraint is moved into `removed_indicator_constraints`.
- {meth}`Instance.restore_indicator_constraint() <ommx.Instance.restore_indicator_constraint>`: restore a previously relaxed indicator constraint. Fails if the indicator variable has already been substituted or fixed.

For OneHot and SOS1, movement into `removed_one_hot_constraints` / `removed_sos1_constraints` happens through the individual lowering APIs covered on this page.
