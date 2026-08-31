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

Each OMMX Adapter must declare the set of {class}`~ommx.Instance` values its strict conversion path accepts, without further Preparation, as a non-optional `INPUT_CLASS`. This is not a broad capability model that includes problems the Adapter could handle after conversion. It is the exact condition on the solver-facing `Instance` delivered to `solve_without_preparation()` or `sample_without_preparation()`.

```{important}
Checking `INPUT_CLASS` does not transform an Instance. The easy `solve()` and `sample()` APIs may accept a source Instance outside this class because they prepare an isolated copy first. Preparation-free APIs require the value they receive to be a member already.
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
assert not input_class.contains(instance)

report = input_class.check_membership(instance)
assert not report.is_member
print(report)
```

See the API Reference for {class}`~ommx.InstanceClassMismatch` and {class}`~ommx.InstanceClassClauseReport` for the mismatch types and attributes returned by `check_membership()`.

## Membership Uses Active Mathematical Content

Membership is determined from the objective and the decision variables actually used by the four active constraint families: regular, Indicator, OneHot, and SOS1. The following variables are not included in the variable-kind check because they are not part of {attr}`~ommx.Instance.used_decision_variables`.

- Fixed, dependent, or irrelevant variables
- Variables referenced only by the output objective
- Variables referenced only by removed constraints
- Variables referenced only by named functions

Likewise, removed constraints are excluded when checking relations, degrees, and the presence of special constraints. An Instance whose special constraints have been lowered to regular constraints can therefore belong to an `INPUT_CLASS` that disallows those special constraints while still retaining the source constraints as removed entries. See [Removed Constraints and Feasibility](./removed_constraints.md) for the distinction between active and removed constraints.

## `INPUT_CLASS` Is Adapter Applicability

`INPUT_CLASS` membership is the complete applicability condition. {meth}`~ommx.adapter.SolverAdapter.check_applicability` returns the same structured membership report as `INPUT_CLASS.check_membership()`. {meth}`~ommx.adapter.SolverAdapter.require_applicable` returns that report for a member and raises {class}`~ommx.adapter.AdapterNotApplicableError` with it for a non-member. There is no second Adapter-owned precondition layer.

Converter-local representation checks and Backend limits may still fail after an Instance has passed applicability. Such a failure belongs to solver-input construction or Backend execution; it does not retroactively make the Instance a non-member of `INPUT_CLASS`.

These checks do not perform any of the following operations.

- Preparation such as lowering, encoding, or a penalty method
- Backend execution
- A guarantee that the Backend will return a solution or samples

The easy `solve()` and `sample()` APIs copy their source Instance, apply the Adapter's recommended Policy to that copy, and call the corresponding preparation-free API. The source is not modified. In contrast, `solve_without_preparation()`, `sample_without_preparation()`, and direct Adapter constructors require an exact member and do not run Preparation. If the application needs different Preparation choices, it prepares a copy explicitly and passes that exact value to the preparation-free API. See [Instance Preparation and `PreparationPolicy`](./preparation_policy.md) and [Prepare an Instance for an Adapter](../tutorial/prepare_instance_for_adapter.md).
