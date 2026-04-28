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

In addition to regular constraints ({class}`~ommx.v1.Constraint` — an equality or inequality over a {class}`~ommx.v1.Function`), OMMX provides several constraint types frequently used in mathematical optimization as first-class citizens. This page introduces the following three special constraint types, their usage, and how to solve them with the PySCIPOpt Adapter.

- {class}`~ommx.v1.IndicatorConstraint`: a conditional constraint driven by a binary variable
- {class}`~ommx.v1.OneHotConstraint`: exactly one of a set of binary variables equals 1
- {class}`~ommx.v1.Sos1Constraint`: at most one of a set of variables is non-zero

The examples below use the PySCIPOpt Adapter, as in [Solving optimization problems with OMMX Adapter](../tutorial/solve_with_ommx_adapter.md). Install it first:

```
pip install ommx-pyscipopt-adapter
```

The PySCIPOpt Adapter declares support for Indicator and SOS1 constraints and passes them through to SCIP's `addConsIndicator` / `addConsSOS1` (equality indicators are split into two inequality indicators). OneHot is not declared as supported, so the adapter automatically converts it into a regular equality constraint before handing it to SCIP. For more on adapter capability declarations and conversions, see [Adapter Capability Model and Conversions](./capability_model.md).

## IndicatorConstraint

An **indicator constraint** enforces a constraint $f(x) \leq 0$ (or $f(x) = 0$) only when a binary variable $z = 1$. When $z = 0$, the constraint is unconditionally satisfied.

Create an {class}`~ommx.v1.IndicatorConstraint` from an existing {class}`~ommx.v1.Constraint` by calling {meth}`Constraint.with_indicator() <ommx.v1.Constraint.with_indicator>`.

```{code-cell} ipython3
from ommx.v1 import Instance, DecisionVariable, Equality

z = DecisionVariable.binary(0, name="z")
x = DecisionVariable.continuous(1, lower=0, upper=10, name="x")

# z = 1 => x <= 5
ic = (x <= 5).with_indicator(z)
assert ic.indicator_variable_id == 0
assert ic.equality == Equality.LessThanOrEqualToZero
```

Add it to an instance by passing a `dict[int, IndicatorConstraint]` to the `indicator_constraints=` argument of {meth}`Instance.from_components <ommx.v1.Instance.from_components>`.

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

The PySCIPOpt Adapter declares support for indicator constraints, so we can solve this directly.

```{code-cell} ipython3
from ommx_pyscipopt_adapter import OMMXPySCIPOptAdapter

solution = OMMXPySCIPOptAdapter.solve(instance)
# With z = 1, the constraint x <= 5 is active, so the maximum value of x is 5
assert abs(solution.objective - 5.0) < 1e-6
```

## OneHotConstraint

A **one-hot constraint** over a set of binary variables $\{x_1, \ldots, x_n\}$ requires $\sum_i x_i = 1$ — i.e. exactly one of them is 1.

```{code-cell} ipython3
from ommx.v1 import OneHotConstraint

xs = [DecisionVariable.binary(i, name="x", subscripts=[i]) for i in range(3)]
oh = OneHotConstraint(variables=[0, 1, 2])
assert oh.variables == [0, 1, 2]
```

The IDs passed to `variables` must correspond to binary variables that are in the instance's `decision_variables`. Mathematically the constraint is equivalent to the linear equality $x_0 + x_1 + x_2 - 1 = 0$, but holding it as a first-class constraint lets supporting solvers (many MIP solvers accept one-hot natively) handle it efficiently.

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

The PySCIPOpt Adapter does not declare OneHot support, so inside `solve` the constraint is automatically converted to the regular equality $x_0 + x_1 + x_2 - 1 = 0$ before being handed to SCIP.

```{code-cell} ipython3
solution = OMMXPySCIPOptAdapter.solve(instance_oh)
# Exactly one of the three is chosen, so x_1 with the largest value 10 is selected
assert abs(solution.objective - 10.0) < 1e-6
```

`instance_oh` is mutated in place by `solve`, so after the call the OneHot constraint is removed and a record of the conversion remains in `removed_one_hot_constraints`.

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
from ommx.v1 import Sos1Constraint

ys = [DecisionVariable.continuous(i, lower=0, upper=10, name="y", subscripts=[i]) for i in range(3, 6)]
s1 = Sos1Constraint(variables=[3, 4, 5])
assert s1.variables == [3, 4, 5]
```

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

The PySCIPOpt Adapter declares support for SOS1, so we can solve this directly.

```{code-cell} ipython3
solution = OMMXPySCIPOptAdapter.solve(instance_s1)
# Only one variable may be non-zero, so one is set to its upper bound 10 and the others to 0
assert abs(solution.objective - 10.0) < 1e-6
```

## Independent ID spaces per constraint type

In OMMX, each of the four constraint collections — regular / Indicator / OneHot / SOS1 — has an **independent ID space**. The four dicts passed to {meth}`Instance.from_components <ommx.v1.Instance.from_components>` are keyed independently, so using the same integer ID across different constraint types does not cause a collision.

For example, "regular constraint ID=1" and "Indicator constraint ID=1" coexist as distinct constraints.

```{code-cell} ipython3
z2 = DecisionVariable.binary(10, name="z2")
x2 = DecisionVariable.continuous(11, lower=0, upper=10, name="x2")

instance_mix = Instance.from_components(
    decision_variables=[z2, x2] + xs + ys,
    objective=x2,
    constraints={1: z2 == 1},                                        # regular ID=1
    indicator_constraints={1: (x2 <= 5).with_indicator(z2)},         # Indicator ID=1
    one_hot_constraints={1: OneHotConstraint(variables=[0, 1, 2])},  # OneHot ID=1
    sos1_constraints={1: Sos1Constraint(variables=[3, 4, 5])},       # SOS1 ID=1
    sense=Instance.MAXIMIZE,
)

# Each of the four dicts holds its own ID=1 constraint independently
assert set(instance_mix.constraints.keys()) == {1}
assert set(instance_mix.indicator_constraints.keys()) == {1}
assert set(instance_mix.one_hot_constraints.keys()) == {1}
assert set(instance_mix.sos1_constraints.keys()) == {1}
```

When a special constraint is converted to a regular constraint (see [Capability Model and Conversions](./capability_model.md)), the generated regular constraint is allocated from the `Constraint` ID space. Only regular constraint IDs can collide after conversion.

## Accessing evaluation results

The {class}`~ommx.v1.Solution` or {class}`~ommx.v1.SampleSet` obtained after solving exposes a single {meth}`~ommx.v1.Solution.constraints_df` method that dispatches on `kind=`:

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

`removed_reason` is no longer a default column of {meth}`~ommx.v1.Solution.constraints_df`. Pass `"removed_reason"` in `include=` to fold the reason name and the `removed_reason.{key}` parameter columns back in (rows whose constraint was not removed before evaluation get NA in those columns):

```python
df = solution.constraints_df(
    include=("metadata", "parameters", "removed_reason"),
)
```

The same applies to Indicator, OneHot, and SOS1: pass the corresponding `kind=` together with `"removed_reason"` in `include=`. The long-format {meth}`~ommx.v1.Solution.constraint_removed_reasons_df` sidecar (also `kind=`-dispatched) remains the right surface when you want one row per (constraint id, parameter key) pair for joins or aggregation.

## Relax / Restore

{class}`~ommx.v1.IndicatorConstraint` supports the same relax / restore workflow as regular constraints.

- {meth}`Instance.relax_indicator_constraint() <ommx.v1.Instance.relax_indicator_constraint>`: relax (deactivate) an indicator constraint and record a reason string. The relaxed constraint is moved into `removed_indicator_constraints`.
- {meth}`Instance.restore_indicator_constraint() <ommx.v1.Instance.restore_indicator_constraint>`: restore a previously relaxed indicator constraint. Fails if the indicator variable has already been substituted or fixed.

For OneHot and SOS1, movement into `removed_one_hot_constraints` / `removed_sos1_constraints` happens via the conversion APIs covered in [Capability Model and Conversions](./capability_model.md).
