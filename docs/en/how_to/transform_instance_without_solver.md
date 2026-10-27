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

# Transform an Instance and inspect the result

To transform a problem into a particular representation, **specify a target InstanceClass and a Policy of allowed transformations, then call `prepare()`.** Lower a OneHot constraint to an ordinary linear equality, then evaluate candidates before and after.

## Environment

Complete {ref}`Python SDK installation <install-python-sdk>`. No Adapter or solver is needed. This page does not depend on another tutorial's runtime state.

## 1. Prepare the problem and comparison results

Choose exactly one of two Binary variables, with value `3 * x[0] + 2 * x[1]`. Save evaluation results for a candidate selecting one and a candidate selecting both. `evaluate()` checks a supplied candidate; it does not solve the problem.

```{code-cell} ipython3
from ommx import Instance, OneHotConstraint

instance = Instance.maximize()
x = [instance.new_binary("x", subscripts=[i]) for i in range(2)]
instance.objective = 3 * x[0] + 2 * x[1]
instance.add_one_hot_constraint(OneHotConstraint(variables=x))

state = {x[0].id: 1, x[1].id: 0}
both = {x[0].id: 1, x[1].id: 1}
before = instance.evaluate(state)
before_both = instance.evaluate(both)
```

## 2. Specify the target representation

Allow only Binary variables, a linear objective, and ordinary linear equalities. A native OneHot constraint does not satisfy this class.

```{code-cell} ipython3
from ommx import (
    Equality, InstanceClass, InstanceClassClause, Kind,
    PolynomialRequirement, Sense,
)

linear = PolynomialRequirement.at_most(1)
target = InstanceClass([
    InstanceClassClause(
        label="binary-linear-equality",
        allowed_variable_kinds={Kind.Binary},
        objective_polynomial_requirement=linear,
        regular_constraint_polynomial_requirements={Equality.EqualToZero: linear},
        allowed_senses={Sense.Maximize},
    )
])

assert not target.contains(instance)
```

`contains()` only checks the current representation; it does not transform it. See [InstanceClass](../user_guide/instance_class.md) for combinations of conditions and representation requirements.

## 3. Choose a Policy and transform the Instance

Allow lowering only OneHot. `prepare()` changes the Instance in place. The earlier evaluation results are already saved, so preserving the original Instance itself is unnecessary.

```{code-cell} ipython3
from ommx import (
    PreparationPolicy, SpecialConstraintKind, SpecialConstraintPreparation,
)

policy = PreparationPolicy(
    special_constraints=SpecialConstraintPreparation.lower_special_constraints(
        kinds={SpecialConstraintKind.OneHot}
    )
)

instance.prepare(target, policy)

assert target.contains(instance)
```

This replaces the OneHot condition, exactly one variable equal to 1, with `x[0] + x[1] == 1`. The Policy selects operations `prepare()` may use; it is not itself the transforming function.

## 4. Inspect active and removed constraints

Active constraints are imposed on the current problem. Removed constraints are no longer imposed there but retained for evaluation. There is now one active ordinary constraint and one retained removed OneHot constraint.

```{code-cell} ipython3
assert len(instance.constraints) == 1
assert not instance.one_hot_constraints
assert len(instance.removed_one_hot_constraints) == 1

instance.constraints_df()
```

Removed does not mean never evaluated. The candidate can still be checked against the original OneHot, separately from the ordinary active constraint. See [Transformed problems and removed constraints](../user_guide/removed_constraints.md).

## 5. Evaluate the supplied candidates

Evaluate the same assignment, `x[0] = 1` and `x[1] = 0`, against the transformed Instance. Compare feasibility and objective values with the saved evaluation.

```{code-cell} ipython3
after = instance.evaluate(state)

assert before.feasible and after.feasible
assert before.objective == after.objective == 3
print(after.objective)
```

The candidate selecting both remains infeasible:

```{code-cell} ipython3
assert not before_both.feasible
assert not instance.evaluate(both).feasible
```

You have transformed an Instance into the requested representation and compared candidate evaluations. `target.contains(instance)` establishes **the requested representation conditions**. Mathematical correspondence with the source depends on the selected operation's guarantees and applicability conditions. Agreement for these candidates does not establish equivalence or optimality for arbitrary transformations.

See [PreparationPolicy](../user_guide/preparation_policy.md) for target, Policy, and execution roles and {meth}`~ommx.Instance.convert_one_hot_to_constraint` for the exact OneHot contract. To execute an Adapter with your own Policy, continue to [Choose which constraint families Preparation transforms](./customize_preparation.md).
