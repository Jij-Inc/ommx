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

# Adapter Input Classes, Preparation, and Constraint Lowering

OMMX separates three concepts that were previously described together as adapter capabilities:

- An {class}`~ommx.InstanceClass` describes a set of exact `Instance` values. An adapter declares its structural input condition with `INPUT_CLASS`, then evaluates adapter-owned preconditions to determine applicability.
- {meth}`SolverAdapter.prepare() <ommx.adapter.SolverAdapter.prepare>` applies an Adapter-declared, policy-permitted Transform to an isolated source and returns an applicable {class}`~ommx.adapter.Preparation` input together with output decoding.
- {meth}`Instance.lower_special_constraints() <ommx.Instance.lower_special_constraints>` explicitly lowers selected special-constraint families on an instance. It does not declare an input class or establish adapter applicability.

This page covers:

- `InstanceClass` membership and adapter applicability
- {class}`~ommx.adapter.Preparation`, {class}`~ommx.adapter.PreparationPolicy`, and output decoding
- {class}`~ommx.SpecialConstraintKind` and {attr}`Instance.active_special_constraint_kinds <ommx.Instance.active_special_constraint_kinds>` as special-constraint family selectors
- {meth}`Instance.lower_special_constraints() <ommx.Instance.lower_special_constraints>` for explicit lowering
- Manual conversion APIs per constraint type
- Auditing conversion results

## Instance classes and adapter applicability

An `InstanceClass` is a finite union of complete, conjunctive {class}`~ommx.InstanceClassClause` values. Membership is evaluated against the exact input without mutating or preparing it.

```{code-cell} ipython3
from ommx import DegreeBound, InstanceClass, InstanceClassClause, Kind, Sense

binary_linear_with_one_hot = InstanceClass(
    [
        InstanceClassClause(
            label="binary-linear-with-one-hot",
            allowed_variable_kinds={Kind.Binary},
            objective_degree_bound=DegreeBound.at_most(1),
            allowed_senses={Sense.Maximize},
            allows_one_hot=True,
        )
    ]
)
```

Adapters declare this first applicability condition as `INPUT_CLASS`. Use `check_applicability()` for a structured result or `require_applicable()` to raise when membership or an adapter-owned precondition fails. Explicit preparation produces another input value, whose applicability must be checked again.

## Policy-driven Adapter preparation

{meth}`SolverAdapter.prepare() <ommx.adapter.SolverAdapter.prepare>` keeps direct Adapter applicability unchanged while providing an explicit preparation workflow. It first prefers identity when the source is already applicable. In the current common slice, it otherwise applies the canonical ordered special-constraint families declared by that Adapter and allowed by the caller's {class}`~ommx.adapter.PreparationPolicy`. Those selected active families are lowered together by the SDK; this tuple is not yet a general alternative-path search API. The source is never mutated, and the produced input is checked again before {class}`~ommx.adapter.Preparation` is returned.

HiGHS declares exact Indicator, OneHot, and SOS1 lowering candidates. A caller can therefore prepare a source with special constraints without manually choosing the active families:

```python
from ommx.adapter import PreparationPolicy
from ommx_highs_adapter import OMMXHighsAdapter

source = instance
preparation = OMMXHighsAdapter.prepare(source)
input_solution = OMMXHighsAdapter.solve(preparation.input)
source_solution = preparation.decode(input_solution)

# A policy can restrict, but never broaden, the Adapter's candidates.
no_automatic_lowering = PreparationPolicy(
    allowed_special_constraint_lowerings=frozenset()
)
```

`preparation.input` and `input_solution` belong to the transformed problem. For the current SDK special lowerings and the OpenJij pipeline, input evaluation retains or reconstructs every source variable. {meth}`Preparation.decode() <ommx.adapter.Preparation.decode>` can therefore select those source variables, remove input-only auxiliaries, and reevaluate the state on `preparation.source`; it does not promote regular constraints back into special constraints. `PreparationTransform` is an applied-transform audit receipt, not a formal proof or a general executable `encode`/`decode` object.

This slice transports successful `Solution` and `SampleSet` values. It does not yet provide a high-level invocation API that transports `InfeasibleDetected` or `UnboundedDetected` across a Preparation. Direct constructors, `solve()`, and `sample()` remain preparation-free and continue to reject non-applicable inputs.

## SpecialConstraintKind and active_special_constraint_kinds

{class}`~ommx.SpecialConstraintKind` enumerates the active special-constraint families that can be selected for explicit lowering to regular constraints. It is not an Adapter input declaration or a serialization feature.

| Kind | Constraint type |
|---|---|
| `SpecialConstraintKind.Indicator` | {class}`~ommx.IndicatorConstraint` |
| `SpecialConstraintKind.OneHot` | {class}`~ommx.OneHotConstraint` |
| `SpecialConstraintKind.Sos1` | {class}`~ommx.Sos1Constraint` |

{attr}`Instance.active_special_constraint_kinds <ommx.Instance.active_special_constraint_kinds>` returns the set of `SpecialConstraintKind` values corresponding to the **active special constraints the instance currently holds**. When the instance uses only regular constraints the set is empty.

```{code-cell} ipython3
from ommx import Instance, DecisionVariable, OneHotConstraint, SpecialConstraintKind

xs = [DecisionVariable.binary(i, name="x", subscripts=[i]) for i in range(3)]

instance = Instance.from_components(
    decision_variables=xs,
    objective=sum(xs),
    constraints={},
    one_hot_constraints={0: OneHotConstraint(variables=xs)},
    sense=Instance.MAXIMIZE,
)
assert instance.active_special_constraint_kinds == {SpecialConstraintKind.OneHot}
assert binary_linear_with_one_hot.contains(instance)
```

## Explicit lowering via lower_special_constraints

{meth}`Instance.lower_special_constraints() <ommx.Instance.lower_special_constraints>` is an explicit, mutating operation. For each family selected in `kinds_to_lower`, the corresponding conversion API (see below) is invoked when that family is active. Families omitted from the set remain active, and an empty set is a no-op.

```{code-cell} ipython3
lowered = instance.lower_special_constraints({SpecialConstraintKind.OneHot})
assert lowered == {SpecialConstraintKind.OneHot}
```

```{code-cell} ipython3
assert instance.active_special_constraint_kinds == set()
assert instance.one_hot_constraints == {}
assert len(instance.constraints) == 1
```

The OneHot constraint has been removed and a regular equality $x_0 + x_1 + x_2 - 1 = 0$ has been added in its place. `lower_special_constraints` mutates the instance in place and returns only the selected families that were active and actually lowered. The method returns an empty set when no selected family was active. Recheck `INPUT_CLASS` membership or adapter applicability on this resulting value.

## Manual conversion APIs

`lower_special_constraints` is implemented by composing the per-type conversion APIs below. You can call these directly as well.

### One-hot → equality constraint

{meth}`Instance.convert_one_hot_to_constraint(one_hot_id) <ommx.Instance.convert_one_hot_to_constraint>` rewrites a OneHot constraint as the mathematically equivalent linear equality $x_1 + \ldots + x_n - 1 = 0$.

```{code-cell} ipython3
instance2 = Instance.from_components(
    decision_variables=xs,
    objective=sum(xs),
    constraints={},
    one_hot_constraints={1: OneHotConstraint(variables=xs)},
    sense=Instance.MAXIMIZE,
)
new_id = instance2.convert_one_hot_to_constraint(1)
assert isinstance(new_id, int)
assert set(instance2.constraints.keys()) == {new_id}
assert instance2.one_hot_constraints == {}
```

Use {meth}`~ommx.Instance.convert_all_one_hots_to_constraints` to convert every active OneHot constraint in one call.

### SOS1 → Big-M constraints

{meth}`Instance.convert_sos1_to_constraints(sos1_id) <ommx.Instance.convert_sos1_to_constraints>` rewrites an SOS1 constraint into regular constraints via the Big-M method. For each variable $x_i \in [l_i, u_i]$:

1. If $x_i$ is binary with bounds $[0, 1]$, it is reused directly as its own indicator.
2. Otherwise a fresh binary indicator $y_i$ is introduced, and the pair $x_i - u_i y_i \leq 0$ and $l_i y_i - x_i \leq 0$ is emitted (trivial sides with $u_i = 0$ or $l_i = 0$ are skipped).
3. Finally, the cardinality constraint $\sum_i y_i - 1 \leq 0$ is added.

```{code-cell} ipython3
from ommx import Sos1Constraint

ys = [DecisionVariable.binary(i, name="y", subscripts=[i]) for i in range(3)]
instance3 = Instance.from_components(
    decision_variables=ys,
    objective=sum(ys),
    constraints={},
    sos1_constraints={1: Sos1Constraint(variables=ys)},
    sense=Instance.MAXIMIZE,
)
new_ids = instance3.convert_sos1_to_constraints(1)
# An all-binary SOS1 collapses to a single cardinality constraint sum(x_i) - 1 <= 0
assert len(new_ids) == 1
assert set(instance3.constraints.keys()) == set(new_ids)
assert instance3.sos1_constraints == {}
```

Use {meth}`~ommx.Instance.convert_all_sos1_to_constraints` to convert every SOS1 constraint in one call. If a variable has a non-finite bound or a domain that excludes 0, conversion fails before any mutation occurs and the instance is left unchanged.

### Indicator → Big-M constraints

{meth}`Instance.convert_indicator_to_constraint(indicator_id) <ommx.Instance.convert_indicator_to_constraint>` rewrites an indicator constraint $y = 1 \Rightarrow f(x) \leq 0$ using the upper and lower bounds of $f(x)$ as the Big-M values. Unlike SOS1, no new indicator variable is introduced; the `IndicatorConstraint`'s existing indicator variable is used as $y$.

$$
f(x) + u y - u \leq 0, \qquad -f(x) - l y + l \leq 0
$$

where $u \geq \sup f(x)$ and $l \leq \inf f(x)$.

- For inequality ($\leq$) indicators, only the upper side is considered and is emitted only when $u > 0$ (when $u \leq 0$, the constraint is already implied by the variable bounds, so nothing is emitted).
- For equality ($= 0$) indicators, the upper and lower sides are considered independently: the upper is emitted if $u > 0$, and the lower is emitted if $l < 0$.

Use {meth}`~ommx.Instance.convert_all_indicators_to_constraints` to convert every indicator constraint in one call. If a required bound on $f(x)$ is non-finite, or if $f(x)$ references a semi-continuous / semi-integer variable, conversion fails before any mutation occurs.

## Auditing conversion results

The original special constraints are not discarded; they are kept as "removed" entries in the following `removed_*_constraints` dicts.

| Original type | Removed dict | DataFrame |
|---|---|---|
| OneHotConstraint | {attr}`~ommx.Instance.removed_one_hot_constraints` | `instance.constraints_df(kind="one_hot", removed=True)` |
| Sos1Constraint | {attr}`~ommx.Instance.removed_sos1_constraints` | `instance.constraints_df(kind="sos1", removed=True)` |
| IndicatorConstraint | {attr}`~ommx.Instance.removed_indicator_constraints` | `instance.constraints_df(kind="indicator", removed=True)` |

`removed=True` returns active + removed rows in the same DataFrame and auto-adds the `removed_reason` / `removed_reason.{key}` columns so removed rows are distinguishable from active ones.

Each entry ({class}`~ommx.RemovedOneHotConstraint` / {class}`~ommx.RemovedSos1Constraint` / {class}`~ommx.RemovedIndicatorConstraint`) records a `removed_reason` string (for example, `"ommx.Instance.convert_one_hot_to_constraint"`) and stores the generated regular-constraint IDs in `removed_reason_parameters`. The key name and shape differ by constraint type:

- **OneHot**: a single ID under the `constraint_id` key
- **SOS1**: a comma-separated list of IDs under the `constraint_ids` key
- **Indicator**: a comma-separated list of IDs under the `constraint_ids` key (empty when both Big-M sides are redundant)

```{code-cell} ipython3
removed = instance2.removed_one_hot_constraints
assert set(removed.keys()) == {1}
```

In addition, each generated regular constraint retains a reference back to its origin via the {attr}`Constraint.provenance <ommx.Constraint.provenance>` property. Each {class}`~ommx.Provenance` entry records the origin kind ({attr}`~ommx.Provenance.kind`, a {class}`~ommx.ProvenanceKind`) and the original ID ({attr}`~ommx.Provenance.original_id`), letting you trace which regular constraint was generated from which specific special constraint.

```{code-cell} ipython3
from ommx import ProvenanceKind

# Walk the regular constraints generated earlier by convert_one_hot_to_constraint(1)
for cid, c in instance2.constraints.items():
    for p in c.provenance:
        assert p.kind == ProvenanceKind.OneHotConstraint
        assert p.original_id == 1
```

## Summary

| What you want to do | API |
|---|---|
| Describe a structural set of adapter inputs | {class}`~ommx.InstanceClass` |
| Declare the first adapter applicability condition | `INPUT_CLASS` |
| Check membership plus adapter-owned preconditions | `check_applicability()` / `require_applicable()` |
| Prepare an isolated applicable Adapter input | {meth}`SolverAdapter.prepare <ommx.adapter.SolverAdapter.prepare>` with {class}`~ommx.adapter.PreparationPolicy` |
| Decode and reevaluate Adapter output on the source | {meth}`Preparation.decode <ommx.adapter.Preparation.decode>` |
| Inspect active special-constraint families | {attr}`Instance.active_special_constraint_kinds <ommx.Instance.active_special_constraint_kinds>` |
| Explicitly lower selected special constraints | {meth}`Instance.lower_special_constraints <ommx.Instance.lower_special_constraints>` |
| Convert individually to regular constraints | `convert_*_to_constraint(s)` / `convert_all_*_to_constraints` |
| Audit conversion history | `instance.constraints_df(kind=..., removed=True)` / `solution.constraints_df(kind=..., include=("...","removed_reason"))` |
