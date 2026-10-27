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

# Prepare an Instance for an Adapter

[The preceding tutorial](./solve_special_constraints_with_pyscipopt_adapter.md) passed Indicator and SOS1 constraints directly to SCIP. However, Adapters do not all accept the same representations.

Solve the same model with the HiGHS Adapter. The key idea is that **OMMX can transform a representation before passing it to an Adapter that cannot accept it directly.** This is Instance Preparation.

## Environment

Add the HiGHS Adapter to the v3 environment using the [installation guide](../getting_started/install.md). The PySCIPOpt Adapter is not needed to execute this page.

## Rebuild the model

Recreate the model so this page can run independently.

```{code-cell} ipython3
from ommx import Instance, Sos1Constraint

source = Instance.maximize()
enabled = source.new_binary("enabled")
option_a = source.new_binary("option_a")
option_b = source.new_binary("option_b")
option_c = source.new_binary("option_c")

source.objective = (
    10 * enabled
    + 6 * option_a
    + 5 * option_b
    + 4 * option_c
)
source.add_indicator_constraint(
    (option_a + option_b <= 1).with_indicator(enabled)
)
source.add_sos1_constraint(
    Sos1Constraint(
        variables=[option_b, option_c],
        name="exclusive_options",
    )
)
```

## Let `solve()` handle Preparation

The HiGHS Adapter accepts ordinary linear constraints but not Indicator or SOS1 constraints directly. You can still use `solve()` as before.

```{code-cell} ipython3
from ommx_highs_adapter import OMMXHighsAdapter

solution = OMMXHighsAdapter.solve(source)

assert solution.feasible
assert abs(solution.objective - 20.0) < 1e-8

# 呼び出し元のInstanceは変更されない
assert source.indicator_constraints
assert source.sos1_constraints

solution.decision_variables_df()
```

`solve()` prepares a copy for HiGHS without modifying the original Instance. Here it converts Indicator and SOS1 into ordinary linear constraints. Rewriting special constraints as ordinary constraints is called **lowering**.

All variables here are Binary and have the finite bounds needed for this conversion. Not every model can be transformed this way. Exact formulas and applicability conditions belong to the API Reference.

## Direct input conditions and transformation choices

`INPUT_CLASS` is the set of Instances an Adapter accepts directly. `PreparationPolicy` selects transformations toward that set. Check that the source needs transformation for HiGHS:

```{code-cell} ipython3
assert not OMMXHighsAdapter.INPUT_CLASS.contains(source)
```

`solve()` used the Adapter's recommended Policy. HiGHS recommends the lowering needed here, so you did not have to write transformation code. When a transformation choice or setting depends on your application, choose your own Policy instead.

## Summary

- An Adapter may solve a model after lowering even if it cannot accept its special constraints directly.
- `solve()` applies a recommended Policy to a copy, preserving your original Instance.
- `INPUT_CLASS` describes direct inputs; `PreparationPolicy` selects transformations toward them.

When you need to choose transformations, use [Choose which constraint families Preparation transforms](../how_to/customize_preparation.md). For concepts, see [InstanceClass and Adapter input conditions](../user_guide/instance_class.md) and [Instance Preparation and PreparationPolicy](../user_guide/preparation_policy.md).
