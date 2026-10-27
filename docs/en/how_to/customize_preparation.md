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

# Choose which constraint families Preparation transforms

This page follows the [automatic Preparation tutorial](../tutorial/prepare_instance_for_adapter.md). **To explicitly choose the transformations your application allows, configure a Policy, prepare the Instance yourself, and call the strict execution API.** For ordinary solving, start with `solve()`.

Here we lower only OneHot to an ordinary constraint and pass SOS1 natively. The PySCIPOpt Adapter accepts SOS1 directly but not OneHot. Its current recommendation makes the same choice; this example explicitly fixes the allowed family rather than relying on that recommendation.

## Build the model

Use the v3 environment from [Install the Python SDK and Adapters](../getting_started/install.md). OneHot means exactly one variable is 1; SOS1 means at most one is nonzero.

```{code-cell} ipython3
from ommx import Instance, OneHotConstraint, Sos1Constraint

instance = Instance.maximize()
primary = [instance.new_binary("primary", subscripts=[i]) for i in range(2)]
optional = [instance.new_binary("optional", subscripts=[i]) for i in range(2)]
instance.objective = (
    10 * primary[0] + 8 * primary[1]
    + 4 * optional[0] + 3 * optional[1]
)
instance.add_one_hot_constraint(OneHotConstraint(variables=primary))
instance.add_sos1_constraint(Sos1Constraint(variables=optional))
```

## Choose and apply a Policy

Start with other phases disabled and set only `special_constraints`. Allow lowering only OneHot. Creating the Policy does not change the Instance.

```{code-cell} ipython3
from ommx import PreparationPolicy, SpecialConstraintKind, SpecialConstraintPreparation
from ommx_pyscipopt_adapter import OMMXPySCIPOptAdapter

policy = PreparationPolicy(
    special_constraints=SpecialConstraintPreparation.lower_special_constraints(
        kinds={SpecialConstraintKind.OneHot}
    )
)

instance.prepare(OMMXPySCIPOptAdapter.INPUT_CLASS, policy)

assert OMMXPySCIPOptAdapter.INPUT_CLASS.contains(instance)
assert not instance.one_hot_constraints
assert instance.sos1_constraints
```

`prepare()` changes the Instance in place. OneHot becomes an ordinary linear equality while SOS1 remains native. The original OneHot is retained as a [removed constraint](../user_guide/removed_constraints.md).

A Policy is not a list of operations that must all execute. Preparation stops upon reaching the target `INPUT_CLASS`. This model needs lowering because of OneHot; an Instance already in the target would not execute configured transformations.

## Solve the prepared Instance

`solve_without_preparation()` performs no additional Preparation, so pass the result of your chosen Policy directly.

```{code-cell} ipython3
solution = OMMXPySCIPOptAdapter.solve_without_preparation(instance)

assert solution.feasible
assert abs(solution.objective - 14.0) < 1e-8
solution.decision_variables_df()
```

The usual `solve()` obtains a new recommended Policy rather than using the `policy` above. Pair `prepare()` with `solve_without_preparation()` for a custom Policy.

See [PreparationPolicy](../user_guide/preparation_policy.md) for phases and failure handling. Exact lowering conditions and formulas are linked from {meth}`~ommx.Instance.lower_special_constraints`.
