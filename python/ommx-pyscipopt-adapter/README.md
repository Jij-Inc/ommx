# OMMX adapter for SCIP

This package provides an adapter for the [SCIP](https://www.scipopt.org/) from [OMMX](https://github.com/Jij-Inc/ommx)

## Usage

`ommx-pyscipopt-adapter` can be installed from PyPI as follows:

```bash
pip install ommx-pyscipopt-adapter
```

SCIP can be used through `ommx-pyscipopt-adapter` by using the following:

```python markdown-code-runner
from ommx_pyscipopt_adapter import OMMXPySCIPOptAdapter
from ommx.v1 import Instance, DecisionVariable

x1 = DecisionVariable.integer(1, lower=0, upper=5)
ommx_instance = Instance.from_components(
    decision_variables=[x1],
    objective=x1,
    constraints=[],
    sense=Instance.MINIMIZE,
)

# Create `ommx.v1.Solution` from the `pyscipot.Model`
ommx_solution = OMMXPySCIPOptAdapter.solve(ommx_instance)

print(ommx_solution)
```

## Reference

TBW
