# ommx-highs-adapter

Provides an adapter to translate between [OMMX](https://github.com/Jij-Inc/ommx) and [HiGHS](https://highs.dev)

# Usage

`ommx-highs-adapter` can be installed from PyPI as follows:

```bash
pip install ommx-highs-adapter
```

An example usage of HiGHS through this adapter:

```python markdown-code-runner
from ommx_highs_adapter import OMMXHighsAdapter
from ommx.v1 import Instance, DecisionVariable

x1 = DecisionVariable.integer(1, lower=0, upper=5)
ommx_instance = Instance.from_components(
    decision_variables=[x1],
    objective=x1,
    constraints=[],
    sense=Instance.MINIMIZE,
)

# Create `ommx.v1.Solution` through `highspy.Highs`
ommx_solution = OMMXHighsAdapter.solve(ommx_instance)

print(ommx_solution)
```
