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

# Adapter Exact Inputs and Instance Preparation

The standard Adapter workflow starts from the exact `INPUT_CLASS` declared by the Adapter and its recommended policy. The caller edits that policy when application-specific choices are needed, applies it to the caller-owned {class}`~ommx.Instance`, and passes that same `Instance` to a strict Adapter API that never prepares input implicitly.

```{important}
There is one fact to remember: **Adapters never transform an input implicitly to make it acceptable. The caller chooses and executes preparation.**
```

## Standard workflow

The following example prepares a model with a OneHot constraint for the HiGHS Adapter.

```{code-cell} ipython3
from ommx import DecisionVariable, Instance, OneHotConstraint
from ommx_highs_adapter import OMMXHighsAdapter

xs = [DecisionVariable.binary(i, name="x", subscripts=[i]) for i in range(3)]
instance = Instance.from_components(
    decision_variables=xs,
    objective=sum((i + 1) * x for i, x in enumerate(xs)),
    constraints={},
    one_hot_constraints={0: OneHotConstraint(variables=xs)},
    sense=Instance.MAXIMIZE,
)

input_class = OMMXHighsAdapter.INPUT_CLASS
assert input_class is not None

policy = OMMXHighsAdapter.recommended_preparation_policy()
# Edit public policy fields here when the application needs different choices.
instance.prepare(input_class, policy)

# Successful preparation guarantees INPUT_CLASS membership.
assert input_class.contains(instance)
# Check Adapter-owned preconditions through the normal applicability path.
OMMXHighsAdapter.require_applicable(instance)
solution = OMMXHighsAdapter.solve(instance)
assert abs(solution.objective - 3.0) < 1e-8
```

{meth}`~ommx.Instance.prepare` mutates `instance` in place and returns `None`. See [Prepare an Instance for an Adapter](../tutorial/prepare_instance_for_adapter.md) for a complete example.

## Exact INPUT_CLASS and Adapter applicability

An {class}`~ommx.InstanceClass` is the **set of exact `Instance` values an Adapter can receive without transformation**. Each {class}`~ommx.InstanceClassClause` is a conjunction of conditions, and an `InstanceClass` is their finite union. Membership examines the value as supplied, including facts such as used variable kinds, objective and constraint degrees, constraint relations, active special-constraint families, and optimization sense.

- `input_class.contains(instance)` returns membership as a `bool`.
- `input_class.check_membership(instance)` returns structured mismatches for every clause.
- Neither operation mutates `instance` or performs preparation such as lowering or encoding.

`INPUT_CLASS` membership and Adapter applicability are separate conditions.

| Condition | Owner | Inspection API |
|---|---|---|
| Exact input structure defined by OMMX | `InstanceClass` | `contains()` / `check_membership()` |
| Backend-specific limits and conversion preconditions | Adapter | `check_applicability()` / `require_applicable()` |

{meth}`~ommx.adapter.SolverAdapter.check_applicability` checks `INPUT_CLASS` membership first and evaluates Adapter-owned preconditions only for a member. For example, limits on Backend IDs or finiteness after converting coefficients to a Backend format are Adapter preconditions. {meth}`~ommx.adapter.SolverAdapter.require_applicable` uses the same report and raises {class}`~ommx.adapter.AdapterNotApplicableError` when the input is not applicable.

Direct `solve()` / `sample()` calls and Adapter constructors are strict. They do not transform an input to make it applicable and do not mutate the caller's `Instance`.

## Recommendation and execution are separate

{meth}`~ommx.adapter.SolverAdapter.recommended_preparation_policy` returns a {class}`~ommx.PreparationPolicy` containing transformations that are commonly useful for reaching that Adapter's `INPUT_CLASS`.

- Every call returns a fresh policy that the caller can safely edit.
- It does not inspect or mutate an `Instance`.
- It does not execute preparation.
- It does not guarantee that a particular `Instance` can be prepared or that Adapter-owned preconditions will hold.

`INPUT_CLASS` and the policy are independent values. The caller passes both to `instance.prepare(input_class, policy)`. `prepare()` is target-directed. On success, the following postcondition holds:

```python
assert input_class.contains(instance)
```

This postcondition does not include Adapter-owned preconditions. Follow it with `require_applicable()` or the normal applicability check performed by `solve()` / `sample()`.

## Native input and prepared input

Different Adapters can choose different exact input forms for the same special constraint.

| Adapter | Exact input | Example recommended preparation |
|---|---|---|
| PySCIPOpt | Accepts linear Indicator and SOS1 natively | Lower only OneHot to a regular constraint |
| HiGHS / Python-MIP | Accept regular linear constraints | Lower Indicator, OneHot, and SOS1 to regular constraints |
| OpenJij | Accepts unconstrained Binary minimization | Lower special constraints, normalize sense, introduce Integer slack, and encode used Integers. The caller chooses fixed penalty magnitudes |

A linear Indicator or SOS1 that PySCIPOpt accepts natively therefore does not need to be transformed merely for uniformity. The same source model can instead be prepared as a different exact input using the HiGHS `INPUT_CLASS` and recommended policy. See [Special Constraints](./special_constraints.md) for their mathematical definitions, native support, and individual conversion APIs.

If the pre-transformation model must remain available, copy it explicitly before calling `prepare()`. The same prepared `Instance` retains variable dependencies and removed constraints and becomes the evaluation owner for the {class}`~ommx.Solution` or {class}`~ommx.SampleSet` returned by the Adapter.

## Advanced: phase order and failure state

Each `PreparationPolicy` field selects an optional phase backed by an existing `Instance` owner operation. `Instance.prepare()` applies each selected phase at most once in this fixed order:

1. Special-constraint lowering
2. Optimization-sense normalization
3. Integer slack introduction
4. Used-Integer encoding
5. Fixed-weight penalty

`prepare()` evaluates target `InstanceClass` membership before the first phase and after every selected phase, stopping as soon as the target contains the instance. If the instance is already a member, preparation is an exact identity and no policy phase runs.

There is no global transaction around preparation. Each delegated owner operation retains its own validation and mutation contract, but a failure in a later phase does not roll back changes committed by earlier phases. Some owner operations also mutate multiple targets sequentially within one phase and can therefore fail after changing only part of that phase. Existing owner-operation exceptions propagate unchanged.

If all configured phases finish without reaching the target, {class}`~ommx.PreparationTargetNotReachedError` is raised. Its `report` attribute contains the {class}`~ommx.InstanceClassMembershipReport` for the final, partially prepared state.

## Summary

| Goal | API |
|---|---|
| Declare the exact input accepted directly by an Adapter | `INPUT_CLASS` / {class}`~ommx.InstanceClass` |
| Inspect exact membership | `contains()` / `check_membership()` |
| Include Adapter-owned preconditions in the check | `check_applicability()` / `require_applicable()` |
| Get the Adapter's preparation recommendation | `recommended_preparation_policy()` |
| Apply a caller-owned policy toward a target | {meth}`~ommx.Instance.prepare` |
| Pass prepared input to a strict Adapter | `solve()` / `sample()` / Adapter constructor |
