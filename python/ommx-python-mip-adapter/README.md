# OMMX adapter for Python-MIP

Solve a linear OMMX Instance with [Python-MIP](https://www.python-mip.com/), or import a Python-MIP model as an OMMX Instance to evaluate it or use another solver.

[Installation from a prerelease or `main`](https://jij-inc-ommx.readthedocs-hosted.com/en/latest/getting_started/install.html#install-python-mip-adapter) · [日本語の導入手順](https://jij-inc-ommx.readthedocs-hosted.com/ja/latest/getting_started/install.html#install-python-mip-adapter)

## Solve an OMMX problem

```python markdown-code-runner
from ommx_python_mip_adapter import OMMXPythonMIPAdapter
from ommx import DecisionVariable, Instance, Sense

x1 = DecisionVariable.integer(1, lower=0, upper=5)
ommx_instance = Instance.from_components(
    decision_variables=[x1],
    objective=x1,
    constraints={},
    sense=Sense.Minimize,
)

ommx_solution = OMMXPythonMIPAdapter.solve(ommx_instance)

print(ommx_solution)
```

## Import a Python-MIP model

```python markdown-code-runner
import mip
import ommx_python_mip_adapter as adapter

model = mip.Model()
x1 = model.add_var(name="1", var_type=mip.INTEGER, lb=0, ub=5)
x2 = model.add_var(name="2", var_type=mip.CONTINUOUS, lb=0, ub=5)
model.objective = -x1 - 2 * x2
model.add_constr(x1 + x2 - 6 <= 0)

ommx_instance = adapter.model_to_instance(model)

print(ommx_instance)
```

## Reference

[Python-MIP adapter API](https://jij-inc-ommx.readthedocs-hosted.com/en/latest/autoapi/ommx_python_mip_adapter/index.html#ommx_python_mip_adapter.OMMXPythonMIPAdapter) · [日本語サイトのAPI Reference](https://jij-inc-ommx.readthedocs-hosted.com/ja/latest/autoapi/ommx_python_mip_adapter/index.html#ommx_python_mip_adapter.OMMXPythonMIPAdapter)

The [model conversion reference](https://jij-inc-ommx.readthedocs-hosted.com/en/latest/autoapi/ommx_python_mip_adapter/index.html#ommx_python_mip_adapter.model_to_instance) covers the supported input and includes an executable example.
