# ommx-openjij-adapter

Provides an adapter between [OMMX](https://github.com/Jij-Inc/ommx) and
[OpenJij](https://www.openjij.org/).

## Usage

`ommx-openjij-adapter` can be installed from PyPI:

```bash
pip install ommx-openjij-adapter
```

OpenJij directly accepts a Binary, unconstrained minimization model through
this adapter. Prepare a constrained model explicitly before sampling it:

```python markdown-code-runner
from ommx import DecisionVariable, Instance
from ommx_openjij_adapter import OMMXOpenJijSAAdapter

x = DecisionVariable.binary(0, name="x")
instance = Instance.from_components(
    decision_variables=[x],
    objective=x,
    constraints={0: x == 1},
    sense=Instance.MINIMIZE,
)

preparation_check = OMMXOpenJijSAAdapter.check_preparation(
    instance,
    uniform_penalty_weight=2.0,
)
assert preparation_check.compatible

sample_set = OMMXOpenJijSAAdapter.sample(
    instance,
    preparation=True,
    uniform_penalty_weight=2.0,
    num_reads=16,
)

print(sample_set.summary)
```

The finite penalty weight is a preparation option passed through `sample`, not
an OpenJij backend sampler parameter. It must be chosen explicitly for a
constrained model. A finite penalty does not guarantee that every returned
sample is feasible for the source model; inspect the feasibility recorded in
`SampleSet`.

## Native capability and preparation

`OMMXOpenJijSAAdapter.CAPABILITIES` describes only models accepted directly by
the OpenJij translator:

- Binary decision variables
- a polynomial objective of any degree (QUBO or Binary HUBO)
- no active regular or special constraints
- minimization

`OMMXOpenJijSAAdapter.check_compatibility()` checks this native boundary. It
does not include conversions that the adapter can perform first. Integer
log-encoding, maximization-to-minimization conversion, exact lowering of
Indicator/OneHot/SOS1 constraints, integer slack, and finite constraint
penalties are explicit preparation operations provided by
`check_preparation()` and `prepare()`.

`sample()` and `solve()` keep the common `SamplerAdapter` contract: pass the
source `Instance` and set `preparation=True`. This also lets
`Experiment.Run.log_sample()` record the source model. Preparation options are
rejected unless that flag is set.

For inspection or direct backend use, `prepare()` returns an
`OpenJijPreparedModel`, which can be passed to the adapter constructor. Its
`report` contains:

- `source_compatibility`: whether the source model and the requested options
  satisfy the explicit preparation contract
- `encoding_compatibility`: whether the intermediate model satisfies the
  remaining Integer-to-Binary encoding conditions
- `steps`: the transformations actually applied
- `final_compatibility`: whether the prepared solver model satisfies the native
  adapter capability and adapter-specific preconditions

Each preparation step records one of these semantic effects:

- `Exact`: an exact rewrite such as sense reversal or valid log-encoding
- `Approximate`: an approximation such as discrete inequality slack when an
  exact rewrite is unavailable
- `FinitePenalty`: replacement of constraints by finite objective penalties,
  which does not assert exact constrained support

Per-constraint penalty weights use regular constraint IDs. A model containing
Indicator, OneHot, or SOS1 constraints must therefore use a uniform penalty
weight after their exact lowering.

If variable bounds prove an inequality infeasible, `check_preparation()`,
`prepare()`, and `sample()`/`solve()` with `preparation=True` raise
`ommx.adapter.InfeasibleDetected` instead of reporting an adapter limitation.

The maximum of 53 auxiliary bits checked for each used Integer variable is a
condition of OMMX's Integer-to-Binary log-encoding operation. It is neither a
native OpenJij backend capability nor an `ommx.v2.Feature`. The latter is a
wire-format forward-compatibility gate that tells readers which serialized
semantics they must understand; adapter capabilities describe which models a
solver adapter can accept.

OMMX does not yet implement `Kind::Spin`. Its addition, including native
OpenJij Spin support, is tracked separately in
[OMMX issue #1082](https://github.com/Jij-Inc/ommx/issues/1082).
