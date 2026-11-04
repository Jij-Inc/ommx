# Check where a metadata edit lands

```{index} Constraint.add_name (v2), Constraint.add_description (v2), Instance.get_constraint_by_id (v2), Instance.decision_variable_analysis (v2)
```

In 2.9.4, editing the constraint returned by `get_constraint_by_id` only modified that Python snapshot. When translating this code, decide whether you intended to edit the model or just a copy for display. Both operations are explicit in v3.

```python
from ommx import Instance

model = Instance.minimize()
x = model.new_binary("x")
live = model.add_constraint(x <= 1, "original")
local = live.detach()
local.set_name("display")
assert model.constraints[live.constraint_id].name == "original"
live.set_name("capacity")
assert model.constraints[live.constraint_id].name == "capacity"
```

A dictionary made with `dict(model.constraints)` still holds attached rows. Detach each active row you want to copy. On a detached constraint, a setter changes the receiver and returns a clone. To apply several edits to the same detached object, call its setters separately.

## Replace a variable analysis query

For one variable, use {meth}`~ommx.Instance.decision_variable_role`. For a report over many variables, use `decision_variables_df()` and its `state_role` and `substituted_value` columns. If your adapter only needs variables sent to the solver, use `used_decision_variables`. These queries answer different questions when fixed or dependent variables remain available for evaluating candidates.
