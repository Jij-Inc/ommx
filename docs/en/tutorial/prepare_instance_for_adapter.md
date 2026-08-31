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

In this tutorial, we solve the same model with the HiGHS Adapter in two ways. Remember these three facts:

1. `INPUT_CLASS` is the exact set accepted by the preparation-free API.
2. `solve()` applies the Adapter's recommended Policy to an isolated copy automatically.
3. For custom choices, prepare a copy yourself and call `solve_without_preparation()`.

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
assert not highs_input_class.contains(source)
```

The HiGHS Adapter accepts regular linear constraints, but its strict input does not include Indicator or SOS1 constraints. Therefore, `source` cannot be passed to `solve_without_preparation()` yet.

## Let `solve()` Apply the Recommendation

The usual `solve()` workflow accepts the source model. It makes an isolated copy, applies `recommended_preparation_policy()` to reach `INPUT_CLASS`, and solves the copy. The HiGHS recommendation converts Indicator and SOS1 constraints to regular linear constraints.

```{code-cell} ipython3
solution = OMMXHighsAdapter.solve(source)

assert solution.feasible
assert abs(solution.objective - 20.0) < 1e-8

# The easy API did not mutate the source model.
assert source.indicator_constraints
assert source.sos1_constraints
solution.decision_variables_df()
```

## Run the Same Preparation Explicitly

When the application needs to inspect or edit the Policy, obtain a fresh recommendation, prepare a chosen Instance, and use the strict `solve_without_preparation()` method. To preserve the source model, prepare a copy made with `copy.copy()`.

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

prepared_solution = OMMXHighsAdapter.solve_without_preparation(prepared)
assert prepared_solution.feasible
assert abs(prepared_solution.objective - 20.0) < 1e-8
prepared_solution.decision_variables_df()
```

## Summary

- `INPUT_CLASS` is the exact set accepted by `solve_without_preparation()`.
- `solve()` prepares an isolated copy with the Adapter's fresh recommended Policy and leaves the source unchanged.
- The recommendation method only creates a Policy; it does not inspect or mutate an Instance by itself.
- For custom Preparation, copy the source, edit and apply the Policy, then call `solve_without_preparation()`.

For more details, see [Adapter Exact Inputs (`INPUT_CLASS`)](../user_guide/adapter_input_class.md), [Instance Preparation and `PreparationPolicy`](../user_guide/preparation_policy.md), [Special Constraint Types](../user_guide/special_constraints.md), and [Removed Constraints and Feasibility](../user_guide/removed_constraints.md).
