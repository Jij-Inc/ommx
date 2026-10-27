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

# Solve special constraints natively with PySCIPOpt

OMMX represents conditional constraints and conditions on combinations of variables as special constraints. OMMX defines their meaning. A supporting Adapter can pass them to a solver without first rewriting them as ordinary constraints.

Build a small model containing:

- **Indicator constraints:** active only when a specified Binary variable is 1.
- **SOS1 constraints:** at most one of the specified variables may be nonzero.

## Environment

Use the v3 environment from [Install the Python SDK and Adapters](../getting_started/install.md), including the PySCIPOpt Adapter.

## Build an Instance with special constraints

The model has four Binary variables. Require `option_a + option_b <= 1` only when `enabled = 1`. Also allow at most one of `option_b` and `option_c` to be selected.

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

# enabled = 1 のときだけ option_a + option_b <= 1
source.add_indicator_constraint(
    (option_a + option_b <= 1).with_indicator(enabled)
)

# option_b と option_c のうち、高々1つだけが非ゼロ
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

`Instance.maximize()` creates an empty maximization problem. Build it incrementally with `new_binary()` and the `add_*_constraint()` methods.

## Solve with the PySCIPOpt Adapter

The PySCIPOpt Adapter accepts linear Indicator constraints and SOS1 directly. As in the knapsack example, pass the Instance to `solve()`.

```{code-cell} ipython3
from ommx_pyscipopt_adapter import OMMXPySCIPOptAdapter

solution = OMMXPySCIPOptAdapter.solve(source)

assert solution.feasible
assert abs(solution.objective - 20.0) < 1e-8
solution.decision_variables_df()
```

The solution has `enabled = 1`, `option_a = 1`, `option_b = 0`, and `option_c = 1`, satisfying both special constraints. You did not need to rewrite them as ordinary constraints.

Some solvers do not directly support these constraints. Next, [Prepare an Instance for an Adapter](./prepare_instance_for_adapter.md) solves the same model with HiGHS, letting OMMX handle the conversion. [Special constraints](../user_guide/special_constraints.md) explains their meanings and links individual APIs.
