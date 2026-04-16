# OMMX Python SDK 3.0.x

```{note}
Python SDK 3.0.0 contains breaking API changes. A migration guide is available in the [Python SDK v2 to v3 Migration Guide](https://github.com/Jij-Inc/ommx/blob/main/PYTHON_SDK_MIGRATION_GUIDE.md).
```

## Unreleased

### Indicator Constraint support ([#789](https://github.com/Jij-Inc/ommx/pull/789), [#790](https://github.com/Jij-Inc/ommx/pull/790), [#795](https://github.com/Jij-Inc/ommx/pull/795), [#796](https://github.com/Jij-Inc/ommx/pull/796))

Indicator Constraints are now a first-class feature in OMMX. An indicator constraint expresses a conditional relationship: a constraint `f(x) <= 0` (or `f(x) = 0`) is enforced only when a user-defined binary indicator variable `z = 1`. When `z = 0`, the constraint is unconditionally satisfied.

#### Creating Indicator Constraints

```python
from ommx.v1 import DecisionVariable, IndicatorConstraint, Equality, Sense, Instance

z = DecisionVariable.binary(0)
x = DecisionVariable.continuous(1, lower=0, upper=10)

# z = 1 → x <= 5
ic = IndicatorConstraint(
    indicator_variable=z,
    function=x - 5,
    equality=Equality.LessThanOrEqualToZero,
)

instance = Instance.from_components(
    decision_variables=[z, x],
    indicator_constraints=[ic],
    objective=x,
    sense=Sense.Minimize,
)
```

#### Evaluation results

After solving, `Solution` and `SampleSet` provide DataFrames for indicator constraints:

- `Solution.indicator_constraints_df` — columns: id, indicator_variable_id, equality, value, indicator_active, used_ids, name, subscripts, description
- `Solution.indicator_removed_reasons_df` — removal reasons for relaxed indicator constraints
- `SampleSet.indicator_constraints_df` / `SampleSet.indicator_removed_reasons_df` — per-sample versions

The `indicator_active` column disambiguates between "the indicator was OFF (constraint trivially satisfied)" and "the indicator was ON and the constraint was satisfied." Note that indicator constraints do not have dual variables, as dual values are not well-defined for conditional constraints.

#### Relax and restore

Indicator constraints support the same relax/restore workflow as regular constraints:

- `Instance.relax_indicator_constraint(id, reason, **parameters)` — relax (deactivate) an indicator constraint with a reason
- `Instance.restore_indicator_constraint(id)` — restore a previously relaxed indicator constraint, with safety checks (fails if the indicator variable was substituted or fixed)

#### `removed_reasons_df` separation

As part of this work, `removed_reason` is no longer a column in `constraints_df`. Instead, `removed_reasons_df` is available as a separate table on both `Solution` and `SampleSet`, which can be joined with `constraints_df`:

```python
df = solution.constraints_df.join(solution.removed_reasons_df)
```

This applies to both regular constraints and indicator constraints.

### Adapter Capability model ([#790](https://github.com/Jij-Inc/ommx/pull/790))

As specialized constraint types (such as Indicator Constraints) are added and support varies across solvers, an Adapter Capability model has been introduced. Adapters declare their supported capabilities via `ADDITIONAL_CAPABILITIES`, and `Instance.check_capabilities()` validates that the problem is compatible before solving.

```python
from ommx.v1 import AdditionalCapability
from ommx.adapter import SolverAdapter

class MySolverAdapter(SolverAdapter):
    ADDITIONAL_CAPABILITIES = frozenset({AdditionalCapability.Indicator})
```

Currently, the PySCIPOpt Adapter declares indicator constraint support. **Each OMMX Adapter will need changes to support Python SDK 3.0.0** — specifically, calling `super().__init__(instance)` for automatic capability checking.

## 3.0.0 Alpha 1

[![Static Badge](https://img.shields.io/badge/GitHub_Release-Python_SDK_3.0.0a1-orange?logo=github)](https://github.com/Jij-Inc/ommx/releases/tag/python-3.0.0a1)

See the GitHub Release above for full details. The following summarizes the main changes. This is a pre-release version. APIs may change before the final release.

### Complete Rust re-export of `ommx.v1` and `ommx.artifact` types ([#770](https://github.com/Jij-Inc/ommx/pull/770), [#771](https://github.com/Jij-Inc/ommx/pull/771), [#774](https://github.com/Jij-Inc/ommx/pull/774), [#775](https://github.com/Jij-Inc/ommx/pull/775), [#782](https://github.com/Jij-Inc/ommx/pull/782))

Python SDK 3.0.0 is fully based on Rust/PyO3.
In 2.0.0, the core implementation was rewritten in Rust while Python wrapper classes remained for compatibility. In 3.0.0, those Python wrappers are removed entirely — all types in `ommx.v1` and `ommx.artifact` are now direct re-exports from Rust, and the `protobuf` Python runtime dependency is eliminated. The `.raw` attribute that previously provided access to the underlying PyO3 implementation has also been removed.

### Migration to Sphinx and ReadTheDocs hosting ([#780](https://github.com/Jij-Inc/ommx/pull/780), [#785](https://github.com/Jij-Inc/ommx/pull/785))

In v2, the Sphinx-based API Reference and Jupyter Book-based documentation were each hosted on [GitHub Pages](https://jij-inc.github.io/ommx/en/introduction.html). In v3, documentation has been fully migrated to Sphinx and is now hosted on [ReadTheDocs](https://jij-inc-ommx.readthedocs-hosted.com/en/latest/). GitHub Pages will continue to host the documentation as of v2.5.1, but all future updates will be on ReadTheDocs only.
