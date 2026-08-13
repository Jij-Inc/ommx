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

# Solve Special Constraints Directly with the PySCIPOpt Adapter

OMMX can represent constraints that a solver handles through dedicated features as "special constraints." When an Adapter supports them, you can pass these constraints to the solver without first converting them to regular constraints.

In this tutorial, we build a small model with two kinds of special constraints and solve it with the PySCIPOpt Adapter:

- **Indicator constraint**: active only when a specified Binary variable is 1
- **SOS1 constraint**: requires at most one of the specified variables to be nonzero

## Installing the Required Library

```
pip install ommx-pyscipopt-adapter
```

## Create an Instance with Special Constraints

The following model has four Binary variables. When `enabled = 1`, it imposes `option_a + option_b <= 1`. It also allows at most one of `option_b` and `option_c` to be selected.

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

# Apply option_a + option_b <= 1 only when enabled = 1.
source.add_indicator_constraint(
    (option_a + option_b <= 1).with_indicator(enabled)
)

# At most one of option_b and option_c may be nonzero.
source.add_sos1_constraint(
    Sos1Constraint(
        variables=[option_b, option_c],
        name="exclusive_options",
    )
)

assert len(source.indicator_constraints) == 1
assert len(source.sos1_constraints) == 1
source
```

`Instance.maximize()` creates an empty maximization problem. We then build the model incrementally with `new_binary()` and the `add_*_constraint()` methods.

## Solve with the PySCIPOpt Adapter

The PySCIPOpt Adapter accepts linear Indicator constraints and SOS1 constraints directly. This model needs no preparation, so we pass `source` directly to `solve()`.

```{code-cell} ipython3
from ommx_pyscipopt_adapter import OMMXPySCIPOptAdapter

solution = OMMXPySCIPOptAdapter.solve(source)

assert solution.feasible
assert abs(solution.objective - 20.0) < 1e-8
solution.decision_variables_df()
```

The solution has `enabled = 1`, `option_a = 1`, `option_b = 0`, and `option_c = 1`, satisfying both the Indicator and SOS1 constraints. We did not need to rewrite either one as a regular constraint before calling the Adapter.

To solve the same model with an Adapter that does not support these special constraints directly, continue to [Prepare an Instance for an Adapter](./prepare_instance_for_adapter.md). For the meanings of each special constraint and links to their individual APIs, see [Special Constraint Types](../user_guide/special_constraints.md).
