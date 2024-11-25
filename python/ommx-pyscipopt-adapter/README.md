# OMMX adapter for SCIP

This package provides an adapter for the [SCIP](https://www.scipopt.org/) from [OMMX](https://github.com/Jij-Inc/ommx)

## Usage

`ommx-pyscipopt-adapter` can be installed from PyPI as follows:

```bash
pip install ommx-pyscipopt-adapter
```

SCIP can be used through `ommx-pyscipopt-adapter` by using the following:

```python markdown-code-runner
import ommx_pyscipopt_adapter as adapter
from ommx.v1 import Instance, DecisionVariable

x1 = DecisionVariable.integer(1, lower=0, upper=5)
ommx_instance = Instance.from_components(
    decision_variables=[x1],
    objective=x1,
    constraints=[],
    sense=Instance.MINIMIZE,
)

# Convert from `ommx.v1.Instance` to `pyscipopt.Model`
model = adapter.instance_to_model(ommx_instance)
model.optimize()
# Create `ommx.v1.State` from Optimized `pyscipopt.Model`
ommx_state = adapter.model_to_state(model, ommx_instance)

print(ommx_state)
```

## Reference

TBW
