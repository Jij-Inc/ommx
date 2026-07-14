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

# Adapter Capability Model and Explicit Preparation

An adapter capability answers one question: **which complete model shapes can
this adapter translate directly to its backend solver?** It includes the kinds
of variables actually used by the solver input, polynomial-degree limits for
the objective and each constraint family, supported constraint relations, and
the optimization sense. Regular constraints are therefore not assumed to be
universally supported.

Three related APIs have deliberately separate responsibilities:

| API | Responsibility |
|---|---|
| {class}`~ommx.AdapterCapabilities` / {class}`~ommx.CapabilityProfile` | Declare native adapter input and compare it with a model |
| {class}`~ommx.SpecialConstraintKind` / {meth}`~ommx.Instance.lower_special_constraints` | Select an explicit special-constraint lowering operation |
| `ommx.v2.Feature` / `required_features` | Decide whether serialized semantics are safe for a reader to deserialize |

Lowering a constraint, encoding an Integer as Binary variables, reversing a
sense, or adding finite penalties may make a model acceptable, but those are
preparation steps rather than native capabilities. Likewise,
`ommx.v2.Feature` is a wire-format forward-compatibility mechanism; it does not
say whether a solver can optimize the deserialized model.

## Deriving the complete model requirements

{meth}`Instance.solver_requirements() <ommx.Instance.solver_requirements>`
derives the active solver-facing shape of an instance. It includes:

- used variable IDs grouped by {class}`~ommx.Kind`;
- the objective degree and optimization {class}`~ommx.Sense`;
- every active regular constraint's relation and degree;
- every active Indicator constraint's relation and body degree; and
- the active OneHot and SOS1 constraint IDs.

Fixed, dependent, irrelevant, removed-constraint-only, and
named-function-only variables do not restrict an adapter profile. Requirements
are derived again on every call, so they reflect an explicit preparation that
mutated a working copy.

```{code-cell} ipython3
from ommx import DecisionVariable, Instance, OneHotConstraint

xs = [DecisionVariable.binary(i, name="x", subscripts=[i]) for i in range(3)]
instance = Instance.from_components(
    decision_variables=xs,
    objective=sum(xs),
    constraints={},
    one_hot_constraints={0: OneHotConstraint(variables=xs)},
    sense=Instance.MAXIMIZE,
)

requirements = instance.solver_requirements()
assert requirements.used_variable_ids == {0, 1, 2}
assert requirements.objective_degree == 1
assert requirements.one_hot_constraint_ids == {0}
```

## Declaring coherent native profiles

An adapter declares `CAPABILITIES` as one or more complete
{class}`~ommx.CapabilityProfile` values. Every field in one profile is a
conjunction. Multiple profiles are alternatives, not fields that are unioned
together; for example, separate continuous-QP and MILP profiles do not
accidentally claim MIQP support.

```{code-cell} ipython3
from ommx import (
    AdapterCapabilities,
    CapabilityProfile,
    DegreeLimit,
    Equality,
    Kind,
    Sense,
)
from ommx.adapter import SolverAdapter

linear_profile = CapabilityProfile(
    name="binary-linear",
    variable_kinds={Kind.Binary},
    objective_degree=DegreeLimit.at_most(1),
    regular_constraints={
        Equality.EqualToZero: DegreeLimit.at_most(1),
        Equality.LessThanOrEqualToZero: DegreeLimit.at_most(1),
    },
    senses={Sense.Minimize, Sense.Maximize},
)

class MyLinearAdapter(SolverAdapter):
    CAPABILITIES = AdapterCapabilities([linear_profile])
```

Omitting a constraint family means that the profile supports no active
constraint of that family. `DegreeLimit.at_most(n)` is cumulative and accepts
degrees from 0 through `n`; `DegreeLimit.any()` accepts every polynomial degree
representable by OMMX. An adapter-specific numerical restriction that cannot be
expressed portably belongs in the adapter's `_check_preconditions` hook, not in
the profile.

## Side-effect-free compatibility checks

{meth}`~ommx.adapter.SolverAdapter.check_compatibility` compares the complete
requirements against each native profile and then checks adapter-specific
preconditions. It does not lower, encode, relax, or otherwise mutate the input.
{meth}`~ommx.adapter.SolverAdapter.require_compatible` returns the same report
on success and raises {class}`~ommx.adapter.AdapterCompatibilityError` on
failure.

```{code-cell} ipython3
from ommx import SpecialConstraintKind

report = MyLinearAdapter.check_compatibility(instance)
assert not report.compatible
assert instance.active_special_constraint_kinds == {SpecialConstraintKind.OneHot}
```

This instance fails because the profile does not claim native OneHot support.
The check leaves the source instance unchanged.

## Explicit special-constraint lowering and recheck

{attr}`Instance.active_special_constraint_kinds <ommx.Instance.active_special_constraint_kinds>`
is an inventory of active special-constraint families. It is not an adapter
support declaration. To lower selected families, prepare a working copy and
pass those {class}`~ommx.SpecialConstraintKind` values to
{meth}`Instance.lower_special_constraints <ommx.Instance.lower_special_constraints>`.

```{code-cell} ipython3
import copy

prepared = copy.deepcopy(instance)
lowered = prepared.lower_special_constraints({SpecialConstraintKind.OneHot})
assert lowered == {SpecialConstraintKind.OneHot}
assert prepared.active_special_constraint_kinds == set()
assert len(prepared.constraints) == 1

# Preparation changed the model shape, so derive requirements and check again.
MyLinearAdapter.require_compatible(prepared)

# The source model was not used as transformation workspace.
assert instance.active_special_constraint_kinds == {SpecialConstraintKind.OneHot}
```

`lower_special_constraints` mutates the selected instance in place and returns
the families it actually lowered. Direct selection avoids interpreting the set
as a declaration of what some adapter supports. The operation composes the
per-family conversion APIs below and preserves the removed constraints and
provenance for audit.

More general preparation can include exact reformulation, approximation,
relaxation, or finite-penalty conversion. Such a workflow should record its
semantics explicitly and must check the resulting solver model against the
native profile again. Adapter-owned conditions such as a backend integer-width
limit are checked in addition to the portable profile; they are not new OMMX
capability fields and are unrelated to `ommx.v2.Feature`.

## Per-family conversion APIs

You can also call the individual conversion APIs directly.

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
| Derive the active solver-facing model shape | {meth}`Instance.solver_requirements <ommx.Instance.solver_requirements>` |
| Declare direct translator support | `SolverAdapter.CAPABILITIES` with {class}`~ommx.AdapterCapabilities` |
| Check without mutating the input | `check_compatibility` / `require_compatible` |
| Inspect active special-constraint families | {attr}`Instance.active_special_constraint_kinds <ommx.Instance.active_special_constraint_kinds>` |
| Explicitly lower selected special constraints | {meth}`Instance.lower_special_constraints <ommx.Instance.lower_special_constraints>` |
| Convert individually to regular constraints | `convert_*_to_constraint(s)` / `convert_all_*_to_constraints` |
| Audit conversion history | `instance.constraints_df(kind=..., removed=True)` / `solution.constraints_df(kind=..., include=("...","removed_reason"))` |
| Check serialized forward compatibility | `ommx.v2.Feature` / `required_features` |
