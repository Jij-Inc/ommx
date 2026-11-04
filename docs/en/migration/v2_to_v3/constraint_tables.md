# Migrate a join on constraint ID

```{index} Instance.constraints_df (v2), Instance.removed_constraints_df (v2), SampleSet.summary_with_constraints (v2)
```

In v2 a report might join the `id` index of `instance.constraints_df` to a table keyed by integers. In v3 the regular-constraint index is named `regular_constraint_id` and contains `ConstraintID` objects. Convert the keys in the application's lookup table to the appropriate ID type before joining.

```python
import pandas as pd
from ommx import ConstraintID, ConstraintKind, DecisionVariable, Instance, Sense

x = DecisionVariable.binary(0)
model = Instance.from_components(
    decision_variables=[x], objective=x, constraints={7: x <= 1}, sense=Sense.Maximize,
)
rows = model.constraints_df(kind=ConstraintKind.Regular, include=[])
owners = pd.Series({ConstraintID(7): "planning"}, name="owner")
report = rows.join(owners)
assert report.loc[ConstraintID(7), "owner"] == "planning"
```

Use `include=["label", "parameters"]` for a wide table with metadata. For a join with one row per key/value, use `constraint_parameters_df(kind=...)`. Transformation history is in `constraint_provenance_df(kind=...)`, with one row per provenance step. For a model, `removed=True` includes active rows as well: filter the non-null `removed_reason` values if your old report contained only removed rows.

## Compare feasibility near the tolerance

A threshold is applied to each constraint's violation. For SOS1, leaving at most one nonzero value in `(1, 0.00006, 0.00006)` takes total absolute change `0.00012`. At a tolerance of `0.0001` the row fails, even though each of the two small values is below that threshold. When reconciling reports, use the stored evaluation tolerance and compare the same constraint family. A new query tolerance does not rerun discrete-value canonicalization. See {meth}`~ommx.Solution.total_violation`.
