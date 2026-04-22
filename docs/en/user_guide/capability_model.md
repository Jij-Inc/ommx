---
kernelspec:
  display_name: ommx-update-books (3.9.23)
  language: python
  name: python3
---

# Adapter Capability Model and Conversions

OMMX treats [special constraints](./special_constraints.md) — `IndicatorConstraint`, `OneHotConstraint`, `Sos1Constraint` — as first-class citizens, but not every solver accepts them directly. To handle the differences uniformly, OMMX provides an **Adapter Capability Model**.

This page covers:

- {class}`~ommx.v1.AdditionalCapability` and {attr}`Instance.required_capabilities <ommx.v1.Instance.required_capabilities>` for describing what an instance requires
- How an adapter declares its supported capabilities via `ADDITIONAL_CAPABILITIES`
- {meth}`Instance.reduce_capabilities() <ommx.v1.Instance.reduce_capabilities>` for automatic conversion
- Manual conversion APIs per constraint type
- Auditing conversion results

## AdditionalCapability and required_capabilities

{class}`~ommx.v1.AdditionalCapability` is the enumeration of "extra constraint types" beyond regular constraints.

| Capability | Constraint type |
|---|---|
| `AdditionalCapability.Indicator` | {class}`~ommx.v1.IndicatorConstraint` |
| `AdditionalCapability.OneHot` | {class}`~ommx.v1.OneHotConstraint` |
| `AdditionalCapability.Sos1` | {class}`~ommx.v1.Sos1Constraint` |

{attr}`Instance.required_capabilities <ommx.v1.Instance.required_capabilities>` returns the set of `AdditionalCapability` values corresponding to the **special constraints the instance currently holds**. When the instance uses only regular constraints the set is empty.

```{code-cell} ipython3
from ommx.v1 import Instance, DecisionVariable, OneHotConstraint, AdditionalCapability

xs = [DecisionVariable.binary(i, name="x", subscripts=[i]) for i in range(3)]

instance = Instance.from_components(
    decision_variables=xs,
    objective=sum(xs),
    constraints={},
    one_hot_constraints={0: OneHotConstraint(variables=[0, 1, 2])},
    sense=Instance.MAXIMIZE,
)
assert instance.required_capabilities == {AdditionalCapability.OneHot}
```

## Adapter-side declaration

Each OMMX Adapter declares which capabilities it supports via the `ADDITIONAL_CAPABILITIES` class attribute.

```python
from ommx.v1 import AdditionalCapability
from ommx.adapter import SolverAdapter

class MySolverAdapter(SolverAdapter):
    ADDITIONAL_CAPABILITIES = frozenset({AdditionalCapability.Indicator})
```

When the adapter's constructor calls `super().__init__(instance)`, **any constraint type not in `ADDITIONAL_CAPABILITIES` is automatically converted into regular constraints.** In other words, the adapter author only needs to handle the types declared plus regular constraints; any instance can be accepted.

By default `ADDITIONAL_CAPABILITIES = frozenset()`, so every special constraint type is auto-converted. Adapters may also declare full support (for example, the PySCIPOpt Adapter currently declares Indicator and SOS1 support).

## Automatic conversion via reduce_capabilities

Inside `super().__init__`, {meth}`Instance.reduce_capabilities() <ommx.v1.Instance.reduce_capabilities>` is called. For each capability in `required_capabilities` that is not in `supported`, the corresponding conversion API (see below) is invoked to turn that special constraint into regular constraints.

```{code-cell} ipython3
converted = instance.reduce_capabilities(supported=set())
assert converted == {AdditionalCapability.OneHot}
```

```{code-cell} ipython3
assert instance.required_capabilities == set()
assert instance.one_hot_constraints == {}
assert len(instance.constraints) == 1
```

The OneHot constraint has been removed and a regular equality $x_0 + x_1 + x_2 - 1 = 0$ has been added in its place. `reduce_capabilities` mutates the instance in place. On success, `required_capabilities` becomes a subset of `supported`. The method returns an empty set when no conversion was needed.

## Manual conversion APIs

`reduce_capabilities` is implemented by composing the per-type conversion APIs below. You can call these directly as well.

### One-hot → equality constraint

{meth}`Instance.convert_one_hot_to_constraint(one_hot_id) <ommx.v1.Instance.convert_one_hot_to_constraint>` rewrites a OneHot constraint as the mathematically equivalent linear equality $x_1 + \ldots + x_n - 1 = 0$.

```{code-cell} ipython3
instance2 = Instance.from_components(
    decision_variables=xs,
    objective=sum(xs),
    constraints={},
    one_hot_constraints={1: OneHotConstraint(variables=[0, 1, 2])},
    sense=Instance.MAXIMIZE,
)
new_id = instance2.convert_one_hot_to_constraint(1)
assert isinstance(new_id, int)
assert set(instance2.constraints.keys()) == {new_id}
assert instance2.one_hot_constraints == {}
```

Use {meth}`~ommx.v1.Instance.convert_all_one_hots_to_constraints` to convert every active OneHot constraint in one call.

### SOS1 → Big-M constraints

{meth}`Instance.convert_sos1_to_constraints(sos1_id) <ommx.v1.Instance.convert_sos1_to_constraints>` rewrites a SOS1 constraint into regular constraints via the Big-M method. For each variable $x_i \in [l_i, u_i]$:

1. If $x_i$ is binary with bounds $[0, 1]$, it is reused directly as its own indicator.
2. Otherwise a fresh binary indicator $y_i$ is introduced, and the pair $x_i - u_i y_i \leq 0$ and $l_i y_i - x_i \leq 0$ is emitted (trivial sides with $u_i = 0$ or $l_i = 0$ are skipped).
3. Finally, the cardinality constraint $\sum_i y_i - 1 \leq 0$ is added.

```{code-cell} ipython3
from ommx.v1 import Sos1Constraint

ys = [DecisionVariable.binary(i, name="y", subscripts=[i]) for i in range(3)]
instance3 = Instance.from_components(
    decision_variables=ys,
    objective=sum(ys),
    constraints={},
    sos1_constraints={1: Sos1Constraint(variables=[0, 1, 2])},
    sense=Instance.MAXIMIZE,
)
new_ids = instance3.convert_sos1_to_constraints(1)
# An all-binary SOS1 collapses to a single cardinality constraint sum(x_i) - 1 <= 0
assert len(new_ids) == 1
assert set(instance3.constraints.keys()) == set(new_ids)
assert instance3.sos1_constraints == {}
```

Use {meth}`~ommx.v1.Instance.convert_all_sos1_to_constraints` to convert every SOS1 constraint in one call. If a variable has a non-finite bound or a domain that excludes 0, conversion fails before any mutation occurs and the instance is left unchanged.

### Indicator → Big-M constraints

{meth}`Instance.convert_indicator_to_constraint(indicator_id) <ommx.v1.Instance.convert_indicator_to_constraint>` rewrites an indicator constraint $y = 1 \Rightarrow f(x) \leq 0$ using the upper and lower bounds of $f(x)$ as the Big-M values. Unlike SOS1, no new indicator variable is introduced; the `IndicatorConstraint`'s existing indicator variable is used as $y$.

$$
f(x) + u y - u \leq 0, \qquad -f(x) - l y + l \leq 0
$$

where $u \geq \sup f(x)$ and $l \leq \inf f(x)$.

- For inequality ($\leq$) indicators, only the upper side is considered and is emitted only when $u > 0$ (when $u \leq 0$, the constraint is already implied by the variable bounds, so nothing is emitted).
- For equality ($= 0$) indicators, the upper and lower sides are considered independently: the upper is emitted if $u > 0$, and the lower is emitted if $l < 0$.

Use {meth}`~ommx.v1.Instance.convert_all_indicators_to_constraints` to convert every indicator constraint in one call. If a required bound on $f(x)$ is non-finite, or if $f(x)$ references a semi-continuous / semi-integer variable, conversion fails before any mutation occurs.

## Auditing conversion results

The original special constraints are not discarded; they are kept as "removed" entries in the following `removed_*_constraints` dicts.

| Original type | Removed dict | DataFrame |
|---|---|---|
| OneHotConstraint | {attr}`~ommx.v1.Instance.removed_one_hot_constraints` | {attr}`~ommx.v1.Instance.removed_one_hot_constraints_df` |
| Sos1Constraint | {attr}`~ommx.v1.Instance.removed_sos1_constraints` | {attr}`~ommx.v1.Instance.removed_sos1_constraints_df` |
| IndicatorConstraint | {attr}`~ommx.v1.Instance.removed_indicator_constraints` | {attr}`~ommx.v1.Instance.removed_indicator_constraints_df` |

Each entry ({class}`~ommx.v1.RemovedOneHotConstraint` / {class}`~ommx.v1.RemovedSos1Constraint` / {class}`~ommx.v1.RemovedIndicatorConstraint`) records a `removed_reason` string (for example, `"ommx.Instance.convert_one_hot_to_constraint"`) and stores the generated regular-constraint IDs in `removed_reason_parameters`. The key name and shape differ by constraint type:

- **OneHot**: a single ID under the `constraint_id` key
- **SOS1**: a comma-separated list of IDs under the `constraint_ids` key
- **Indicator**: a comma-separated list of IDs under the `constraint_ids` key (empty when both Big-M sides are redundant)

```{code-cell} ipython3
removed = instance2.removed_one_hot_constraints
assert set(removed.keys()) == {1}
```

In addition, each generated regular constraint retains a reference back to its origin via the {attr}`Constraint.provenance <ommx.v1.Constraint.provenance>` property. Each {class}`~ommx.v1.Provenance` entry records the origin kind ({attr}`~ommx.v1.Provenance.kind`, a {class}`~ommx.v1.ProvenanceKind`) and the original ID ({attr}`~ommx.v1.Provenance.original_id`), letting you trace which regular constraint was generated from which specific special constraint.

```{code-cell} ipython3
from ommx.v1 import ProvenanceKind

# Walk the regular constraints generated earlier by convert_one_hot_to_constraint(1)
for cid, c in instance2.constraints.items():
    for p in c.provenance:
        assert p.kind == ProvenanceKind.OneHotConstraint
        assert p.original_id == 1
```

## Summary

| What you want to do | API |
|---|---|
| Inspect which capabilities an instance requires | {attr}`Instance.required_capabilities <ommx.v1.Instance.required_capabilities>` |
| Declare supported capabilities on an adapter | The `ADDITIONAL_CAPABILITIES` class attribute |
| Auto-convert every unsupported special constraint | {meth}`Instance.reduce_capabilities <ommx.v1.Instance.reduce_capabilities>` |
| Convert individually to regular constraints | `convert_*_to_constraint(s)` / `convert_all_*_to_constraints` |
| Audit conversion history | `Instance.removed_*_constraints(_df)` / `Solution.*_removed_reasons_df` |
