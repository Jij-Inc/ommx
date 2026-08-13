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

When an adapter can handle a model as-is, pass the {class}`~ommx.Instance` directly to `solve()`. Only when the model needs conversion, copy the original Instance and prepare that copy as the user, using the adapter's recommendation as a starting point.

First, we solve an Instance with Indicator and SOS1 constraints directly with the PySCIPOpt Adapter. Then, we prepare a copy of the same Instance for the HiGHS Adapter, which does not accept those special constraints directly.

## Installing the Required Library

```
pip install ommx-pyscipopt-adapter ommx-highs-adapter
```

## Solving Indicator and SOS1 Constraints Directly

An Indicator constraint is active only when its binary indicator variable is 1. An SOS1 constraint requires at most one of its variables to be nonzero. The PySCIPOpt Adapter can handle linear Indicator constraints and SOS1 constraints directly.

In the following model, the Indicator constraint `production <= 5` is active when `enabled = 1`. The SOS1 constraint prevents `option_a` and `option_b` from both being nonzero.

```{code-cell} ipython3
from ommx import DecisionVariable, Instance, Sos1Constraint

enabled = DecisionVariable.binary(0, name="enabled")
production = DecisionVariable.continuous(
    1, lower=0, upper=10, name="production"
)
option_a = DecisionVariable.continuous(2, lower=0, upper=4, name="option_a")
option_b = DecisionVariable.continuous(3, lower=0, upper=6, name="option_b")

source = Instance.from_components(
    decision_variables=[enabled, production, option_a, option_b],
    objective=production + option_a + option_b,
    constraints={0: enabled == 1},
    indicator_constraints={
        0: (production <= 5).with_indicator(enabled),
    },
    sos1_constraints={
        0: Sos1Constraint(variables=[option_a, option_b]),
    },
    sense=Instance.MAXIMIZE,
)
```

This Instance can be solved as-is, without preparation.

```{code-cell} ipython3
from ommx_pyscipopt_adapter import OMMXPySCIPOptAdapter

direct_solution = OMMXPySCIPOptAdapter.solve(source)

# production = 5, option_a = 0, option_b = 6
assert abs(direct_solution.objective - 11.0) < 1e-6
```

## Inputs Accepted Directly by an Adapter

Each adapter's `INPUT_CLASS` is the **set of exact Instance values that the adapter accepts directly**. The PySCIPOpt Adapter's input class includes the linear Indicator and SOS1 constraints used above.

```{code-cell} ipython3
scip_input_class = OMMXPySCIPOptAdapter.INPUT_CLASS
assert scip_input_class is not None
assert scip_input_class.contains(source)
```

The HiGHS Adapter accepts regular linear constraints, but not Indicator or SOS1 constraints directly. The same source Instance therefore does not belong to its input class.

```{code-cell} ipython3
from ommx_highs_adapter import OMMXHighsAdapter

highs_input_class = OMMXHighsAdapter.INPUT_CLASS
assert highs_input_class is not None
assert not highs_input_class.contains(source)
```

## Preparing with the Recommended Policy

An adapter's `recommended_preparation_policy()` returns a {class}`~ommx.PreparationPolicy` recommended for moving toward its input class. The HiGHS recommendation lowers Indicator and SOS1 constraints to regular constraints. Each call returns a fresh policy, so the user can edit it when needed.

Obtaining the recommendation does not change an Instance. The user chooses which Instance to prepare and calls {meth}`Instance.prepare() <ommx.Instance.prepare>`. To keep the original model, we prepare a copy made with `copy.copy()`.

```{code-cell} ipython3
import copy

prepared = copy.copy(source)
policy = OMMXHighsAdapter.recommended_preparation_policy()

prepared.prepare(highs_input_class, policy)

assert highs_input_class.contains(prepared)
assert source.indicator_constraints and source.sos1_constraints
assert not prepared.indicator_constraints
assert not prepared.sos1_constraints
```

The source Instance remains unchanged; only `prepared` has been lowered. Pass that prepared input to the HiGHS Adapter and solve it.

```{code-cell} ipython3
prepared_solution = OMMXHighsAdapter.solve(prepared)

# Both exact inputs represent the same source problem.
assert abs(prepared_solution.objective - direct_solution.objective) < 1e-6
```

## Summary

- Pass an Instance directly to `solve()` when the adapter accepts it as-is.
- `INPUT_CLASS` is the set of exact inputs accepted directly by an adapter.
- A recommended policy proposes conversions; it does not execute preparation.
- The user is responsible for choosing the Instance and policy and calling `Instance.prepare()`.
- The same source model can have different exact inputs for different adapters.

See [Adapter Exact Inputs and Instance Preparation](../user_guide/capability_model.md) for the detailed meaning of exact inputs and preparation steps. See [Special Constraint Types](../user_guide/special_constraints.md) for the mathematical definitions of Indicator, OneHot, and SOS1 constraints and their individual conversion APIs.
