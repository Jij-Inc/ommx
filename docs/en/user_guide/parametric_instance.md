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

# ParametricInstance: reuse a fixed problem structure

A {class}`~ommx.ParametricInstance` leaves some coefficients or constants as parameters. Use it to **create Instances with different numerical values while keeping variable and constraint structure fixed**. This page explains problem families versus concrete problems for readers familiar with Instance variables, objectives, and constraints.

## Set parameters to create an Instance

Consider selecting from two items to maximize a score, varying only the maximum number selected. `capacity` is a parameter supplied by the problem's creator, not a decision variable chosen by the solver.

```{code-cell} ipython3
from ommx import DecisionVariable, Parameter, ParametricInstance, Sense

xs = [DecisionVariable.binary(i, name="x", subscripts=[i]) for i in range(2)]
capacity = Parameter(2, name="capacity")

family = ParametricInstance.from_components(
    decision_variables=xs,
    parameters=[capacity],
    objective=3 * xs[0] + 2 * xs[1],
    constraints={0: xs[0] + xs[1] <= capacity},
    sense=Sense.Maximize,
)

one_item = family.with_parameters({capacity.id: 1})
two_items = family.with_parameters({capacity.id: 2})
```

The dictionary passed to {meth}`~ommx.ParametricInstance.with_parameters` maps parameter IDs to concrete values. The result is an ordinary {class}`~ommx.Instance`, usable for evaluation, Preparation, and Adapter execution. The original ParametricInstance remains available for reuse with other values.

```{code-cell} ipython3
candidate = {xs[0].id: 1, xs[1].id: 1}
assert not one_item.evaluate(candidate).feasible
assert two_items.evaluate(candidate).feasible
assert two_items.evaluate(candidate).objective == 5.0

assert {v.id for v in one_item.decision_variables} == {
    v.id for v in two_items.decision_variables
}
```

The same candidate's feasibility changes with the limit. Shared variable and constraint IDs let you associate results under different numerical conditions with the same items and conditions. Matching IDs do not imply matching feasibility or optima.

## Parameters are identified by IDs too

Parameters and decision variables can appear in the same expressions but have different roles. Parameters are supplied before creating an Instance; decision variables receive values in a candidate. They share an ID space, so their IDs must not overlap. Above, decision-variable IDs are 0 and 1, and the parameter ID is 2.

Inspect names, subscripts, and related metadata with {meth}`~ommx.ParametricInstance.parameters_df`. See `with_parameters()` in the API Reference for required parameters and value validation.

## The boundary with a modeler

This example fixes the number of items at two. Parameters cannot generate structure itself, such as increasing or decreasing variables and constraints when the item count changes. Rebuild the model in Python or use a modeler that generates that structure. See [Load an existing problem as an Instance](../how_to/load_existing_instance.md) for receiving models from external tools.

Experiment is not required to use ParametricInstance. Continue to [Recording trials](../tutorial/experiment_management) only if you want to organize problem conditions and results for later inspection.
