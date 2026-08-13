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

# Prepare an Instance for an Adapter

In [Solve Special Constraints Directly with the PySCIPOpt Adapter](./solve_special_constraints_with_pyscipopt_adapter.md), we passed Indicator and SOS1 constraints directly to SCIP. Not every Adapter accepts the same kinds of Instance directly.

In this tutorial, we prepare the same model for the HiGHS Adapter and solve it. Remember these three steps:

1. Check the Adapter's `INPUT_CLASS` to determine whether the Instance can be passed directly.
2. Obtain the Adapter's recommended Policy, and choose the Instance to prepare.
3. Call `Instance.prepare()` yourself, then pass the prepared Instance to the Adapter.

## Installing the Required Library

```
pip install ommx-highs-adapter
```

## Create the Same Model

To make this page runnable on its own, we recreate the model from the previous tutorial.

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

## Check the Input Accepted Directly by the Adapter

Each Adapter's `INPUT_CLASS` is the **set of Instance values that it accepts directly without conversion**. Use `contains()` to check whether an Instance belongs to that set.

```{code-cell} ipython3
from ommx_highs_adapter import OMMXHighsAdapter

highs_input_class = OMMXHighsAdapter.INPUT_CLASS
assert highs_input_class is not None
assert not highs_input_class.contains(source)
```

The HiGHS Adapter accepts regular linear constraints, but it does not accept Indicator or SOS1 constraints directly. Therefore, `source` is not yet an input for the HiGHS Adapter.

## Prepare with the Recommended Policy

An Adapter's `recommended_preparation_policy()` proposes conversions that are generally useful for reaching its `INPUT_CLASS`. The HiGHS Adapter's recommendation converts Indicator and SOS1 constraints to regular linear constraints.

Obtaining a recommended Policy does not change an Instance. The user chooses which Instance to convert and calls `prepare()`. To preserve the original model, we prepare a copy made with `copy.copy()`.

```{code-cell} ipython3
import copy

policy = OMMXHighsAdapter.recommended_preparation_policy()

prepared = copy.copy(source)
prepared.prepare(highs_input_class, policy)

# The prepared Instance can now be passed directly to the HiGHS Adapter.
assert highs_input_class.contains(prepared)

# source remains unchanged; only prepared has been converted.
assert source.indicator_constraints
assert source.sos1_constraints
assert not prepared.indicator_constraints
assert not prepared.sos1_constraints

prepared_solution = OMMXHighsAdapter.solve(prepared)
assert prepared_solution.feasible
assert abs(prepared_solution.objective - 20.0) < 1e-8
prepared_solution.decision_variables_df()
```

## Summary

- If an Adapter accepts an Instance directly, pass it to `solve()` as-is.
- `INPUT_CLASS` is the set of inputs an Adapter accepts directly without conversion.
- A recommended Policy proposes conversions; it does not perform Preparation.
- The user chooses which Instance to convert and calls `Instance.prepare()`.
- To preserve the original model, prepare a copy before passing it to the Adapter.

For more details, see [Adapter Exact Inputs (`INPUT_CLASS`)](../user_guide/adapter_input_class.md), [Instance Preparation and `PreparationPolicy`](../user_guide/preparation_policy.md), [Special Constraint Types](../user_guide/special_constraints.md), and [Removed Constraints and Feasibility](../user_guide/removed_constraints.md).
