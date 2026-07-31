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
from ommx import Instance, DecisionVariable

x1 = DecisionVariable.integer(1, lower=0, upper=5)
ommx_instance = Instance.from_components(
    decision_variables=[x1],
    objective=x1,
    constraints={},
    sense=Instance.MINIMIZE,
)

# Create `ommx.Solution` through `highspy.Highs`
ommx_solution = OMMXHighsAdapter.solve(ommx_instance)

print(ommx_solution)
```

`solve()` remains a direct-input API and does not transform its argument. When
the source contains Indicator, OneHot, or SOS1 constraints, use the explicit
Preparation workflow. HiGHS declares exact lowering for those three families;
the produced input is checked again before it is returned.

```python
preparation = OMMXHighsAdapter.prepare(source)
input_solution = OMMXHighsAdapter.solve(preparation.input)
source_solution = preparation.decode(input_solution)
```

`input_solution` belongs to the lowered input. `decode()` removes any auxiliary
variables introduced by lowering and reevaluates the state against the retained
source instance. A caller can restrict automatic lowering with
`ommx.adapter.PreparationPolicy`; a policy never adds candidates that HiGHS did
not declare.
