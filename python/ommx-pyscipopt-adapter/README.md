# OMMX adapter for SCIP

Solve an OMMX Instance with [SCIP](https://www.scipopt.org/) through PySCIPOpt. The adapter can pass supported indicator and SOS1 constraints directly to SCIP.

[Installation from a prerelease or `main`](https://jij-inc-ommx.readthedocs-hosted.com/en/latest/getting_started/install.html#install-pyscipopt-adapter) · [日本語の導入手順](https://jij-inc-ommx.readthedocs-hosted.com/ja/latest/getting_started/install.html#install-pyscipopt-adapter)

## Solve an OMMX problem

```python markdown-code-runner
from ommx_pyscipopt_adapter import OMMXPySCIPOptAdapter
from ommx import DecisionVariable, Instance, Sense

x1 = DecisionVariable.integer(1, lower=0, upper=5)
ommx_instance = Instance.from_components(
    decision_variables=[x1],
    objective=x1,
    active_constraints={},
    sense=Sense.Minimize,
)

ommx_solution = OMMXPySCIPOptAdapter.solve(ommx_instance)

print(ommx_solution)
```

## Supply a previous candidate by name

```python markdown-code-runner
from ommx import Instance, NamedState
from ommx_pyscipopt_adapter import OMMXPySCIPOptAdapter

instance = Instance.maximize()
x = instance.new_integer("x", lower=0, upper=5, subscripts=[0])
y = instance.new_integer("x", lower=0, upper=5, subscripts=[1])
instance.objective = 2 * x + y
instance.add_constraint(x + y <= 5)

initial_values: NamedState = {"x": {(0,): 1.0, (1,): 4.0}}
solution = OMMXPySCIPOptAdapter.solve(instance, initial_state=initial_values)
assert solution.state.entries == {x.id: 5.0, y.id: 0.0}

# Explicit conversion is also available when you need an ommx.State.
state = instance.state_from_named_values(initial_values)
```

## Reference

[SCIP adapter API](https://jij-inc-ommx.readthedocs-hosted.com/en/latest/autoapi/ommx_pyscipopt_adapter/index.html#ommx_pyscipopt_adapter.OMMXPySCIPOptAdapter) · [日本語サイトのAPI Reference](https://jij-inc-ommx.readthedocs-hosted.com/ja/latest/autoapi/ommx_pyscipopt_adapter/index.html#ommx_pyscipopt_adapter.OMMXPySCIPOptAdapter)

For the termination report and the bound/gap history, see the [SCIP diagnostics reference](https://jij-inc-ommx.readthedocs-hosted.com/en/latest/autoapi/ommx_pyscipopt_adapter/index.html#ommx_pyscipopt_adapter.SCIPDiagnosticsAnalyzer).
