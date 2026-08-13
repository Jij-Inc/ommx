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

# Adapter Exact Inputs (`INPUT_CLASS`)

Each OMMX Adapter declares the set of {class}`~ommx.Instance` values it accepts directly, without conversion, as `INPUT_CLASS`. This is not a broad capability model that includes problems the Adapter could handle after conversion. It is an exact condition on **the value about to be passed to the Adapter**.

```{important}
Checking `INPUT_CLASS` does not transform an Instance. The Instance checked before Preparation and the Instance produced by Preparation are different inputs, so check membership for the value you actually pass to the Adapter.
```

## The Set Represented by InstanceClass

The type of `INPUT_CLASS`, {class}`~ommx.InstanceClass`, represents a finite union of {class}`~ommx.InstanceClassClause` values: an Instance is accepted if it satisfies at least one clause. Within one clause, all of the following conditions must hold at the same time.

- Kinds of used decision variables
- Degree of the objective function
- Relations and degrees of regular constraints
- Relations and degrees of the bodies of Indicator constraints
- Whether active OneHot and SOS1 constraints are allowed
- Optimization sense

For example, if one clause describes continuous quadratic programs and another describes linear programs that may use integer variables, the clauses do not combine to accept mixed-integer quadratic programs.

## Checking Membership

{meth}`~ommx.InstanceClass.contains` returns membership as a `bool`. When you also need to know why an Instance is not accepted, {meth}`~ommx.InstanceClass.check_membership` returns an {class}`~ommx.InstanceClassMembershipReport` with a report for each clause. Both operations are side-effect free.

The following Instance has an active OneHot constraint, so it is not in the PySCIPOpt Adapter's `INPUT_CLASS`, which does not accept OneHot constraints directly.

```{code-cell} ipython3
from ommx import DecisionVariable, Instance, OneHotConstraint
from ommx_pyscipopt_adapter import OMMXPySCIPOptAdapter

xs = [DecisionVariable.binary(i) for i in range(3)]
instance = Instance.from_components(
    decision_variables=xs,
    objective=sum(xs),
    constraints={},
    one_hot_constraints={0: OneHotConstraint(variables=xs)},
    sense=Instance.MAXIMIZE,
)

input_class = OMMXPySCIPOptAdapter.INPUT_CLASS
assert input_class is not None
assert not input_class.contains(instance)

report = input_class.check_membership(instance)
assert not report.is_member
print(report)
```

See the API Reference for {class}`~ommx.InstanceClassMismatch` and {class}`~ommx.InstanceClassClauseReport` for the mismatch types and attributes returned by `check_membership()`.

## Membership Uses Active Mathematical Content

Membership is determined from the objective and the decision variables actually used by the four active constraint families: regular, Indicator, OneHot, and SOS1. The following variables are not included in the variable-kind check because they are not part of {attr}`~ommx.Instance.used_decision_variables`.

- Fixed, dependent, or irrelevant variables
- Variables referenced only by removed constraints
- Variables referenced only by named functions

Likewise, removed constraints are excluded when checking relations, degrees, and the presence of special constraints. An Instance whose special constraints have been lowered to regular constraints can therefore belong to an `INPUT_CLASS` that disallows those special constraints while still retaining the source constraints as removed entries. See [Removed Constraints and Feasibility](./removed_constraints.md) for the distinction between active and removed constraints.

## `INPUT_CLASS` and Adapter Applicability

`INPUT_CLASS` membership is the first condition for Adapter applicability, but the two are not identical.

| Check | Owner | API |
|---|---|---|
| Exact input structure represented by OMMX | `InstanceClass` | `contains()` / `check_membership()` |
| Applicability including Backend-specific conditions | Adapter | `check_applicability()` / `require_applicable()` |

{meth}`~ommx.adapter.SolverAdapter.check_applicability` first checks `INPUT_CLASS` membership. Only for a member does it check Adapter-specific preconditions, such as the range of IDs accepted by the Backend. {meth}`~ommx.adapter.SolverAdapter.require_applicable` performs the same checks and raises {class}`~ommx.adapter.AdapterNotApplicableError` when the Instance is not applicable.

These checks do not perform any of the following operations.

- Preparation such as lowering, encoding, or a penalty method
- Backend execution
- A guarantee that the Backend will return a solution or samples

Neither `solve()`, `sample()`, nor an Adapter constructor prepares its input implicitly. If an Instance is not in `INPUT_CLASS`, the caller prepares a different input by following [Instance Preparation and `PreparationPolicy`](./preparation_policy.md). See [Prepare an Instance for an Adapter](../tutorial/prepare_instance_for_adapter.md) for the complete workflow.
