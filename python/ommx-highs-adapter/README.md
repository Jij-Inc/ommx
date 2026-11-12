# OMMX adapter for HiGHS

Use HiGHS to solve a linear optimization problem expressed as an OMMX Instance. The adapter prepares a copy of the problem and evaluates the returned candidate in OMMX.

[Installation from a prerelease or `main`](https://jij-inc-ommx.readthedocs-hosted.com/en/latest/getting_started/install.html#install-highs-adapter) · [日本語の導入手順](https://jij-inc-ommx.readthedocs-hosted.com/ja/latest/getting_started/install.html#install-highs-adapter)

## Solve an OMMX problem

```python markdown-code-runner
from ommx_highs_adapter import OMMXHighsAdapter
from ommx import DecisionVariable, Instance, Sense

x1 = DecisionVariable.integer(1, lower=0, upper=5)
ommx_instance = Instance.from_components(
    decision_variables=[x1],
    objective=x1,
    active_constraints={},
    sense=Sense.Minimize,
)

ommx_solution = OMMXHighsAdapter.solve(ommx_instance)

print(ommx_solution)
```

## Reference

[HiGHS adapter API](https://jij-inc-ommx.readthedocs-hosted.com/en/latest/autoapi/ommx_highs_adapter/index.html#ommx_highs_adapter.OMMXHighsAdapter) · [日本語サイトのAPI Reference](https://jij-inc-ommx.readthedocs-hosted.com/ja/latest/autoapi/ommx_highs_adapter/index.html#ommx_highs_adapter.OMMXHighsAdapter)

For search progress, objective bounds and termination status, use the [HiGHS diagnostics reference](https://jij-inc-ommx.readthedocs-hosted.com/en/latest/autoapi/ommx_highs_adapter/index.html#ommx_highs_adapter.HighsDiagnosticsAnalyzer).
