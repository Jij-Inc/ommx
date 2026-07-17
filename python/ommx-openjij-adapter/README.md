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

prepared = OMMXOpenJijSAAdapter.prepare(
    instance,
    uniform_penalty_weight=2.0,
)

prepared_samples = OMMXOpenJijSAAdapter.sample(
    prepared.input,
    num_reads=16,
)
sample_set = prepared.evaluate_source(prepared_samples)

print(sample_set.summary)
```

The finite penalty weight is an option passed to `prepare`, not an OpenJij
backend sampler parameter. It must be chosen explicitly when constraints remain
after exact preparation. A finite penalty does not guarantee that every returned
sample is feasible for the source model; inspect the feasibility recorded in the
decoded `SampleSet`.

## Input class and explicit preparation

`OMMXOpenJijSAAdapter.INPUT_CLASS` describes the instances that the adapter
accepts directly:

- Binary decision variables
- a polynomial objective of any degree (QUBO or Binary HUBO)
- no active regular or special constraints
- minimization

`OMMXOpenJijSAAdapter.check_applicability()` checks whether an instance belongs
to this input class and satisfies the adapter-specific preconditions. It does
not include preparation that the adapter can perform first. Integer
log-encoding, maximization-to-minimization conversion, exact lowering of
Indicator/OneHot/SOS1 constraints, integer slack, and finite constraint
penalties are explicit preparation operations provided by `check_preparation()`
and `prepare()`.

`sample()` and `solve()` keep the common adapter contract and accept an
`Instance` only. Explicit preparation therefore returns an
`OpenJijPreparation`: pass its `input` `Instance` to the adapter, then use
`evaluate_source()` to evaluate the resulting samples against the source
model. The preparation itself is not an Adapter input. Its `report` contains:

- `source_check`: membership in the preparation source class and the
  Adapter-owned preparation preconditions
- `steps`: the OpenJij-specific operations actually applied
- `preparation_failures`: failures discovered while materializing an accepted
  source into an Adapter input; empty for a successful preparation
- `input_applicability`: whether `OpenJijPreparation.input` belongs to
  the Adapter input class and satisfies its Adapter-specific preconditions

The step list is an operation audit, not a composed mathematical guarantee.
Common preparation policy, guarantees, and automatic selection are tracked in
[OMMX issue #1111](https://github.com/Jij-Inc/ommx/issues/1111). By default,
this prototype applies only the available exact operations. Discrete integer
slack approximation requires `allow_approximate_integer_slack=True`; choosing
an integer slack range does not itself opt into approximation. Finite penalties
remain an explicit operation selected by supplying their required weights, and
do not assert exact constrained support.

Per-constraint penalty weights use regular constraint IDs. A model containing
Indicator, OneHot, or SOS1 constraints must therefore use a uniform penalty
weight after their exact lowering.

If variable bounds prove an inequality infeasible, `check_preparation()` and
`prepare()` raise `ommx.adapter.InfeasibleDetected` instead of reporting an
adapter limitation.

The maximum of 53 auxiliary bits checked for each used Integer variable is a
condition of OMMX's Integer-to-Binary log-encoding operation. It is neither a
property of the OpenJij adapter's input class nor an `ommx.v2.Feature`. The
latter is a wire-format forward-compatibility gate that tells readers which
serialized semantics they must understand; an adapter's input class and
preconditions determine its applicability to an in-memory `Instance`.

OMMX does not yet implement `Kind::Spin`. Its addition, including direct
OpenJij Spin input, is tracked separately in
[OMMX issue #1082](https://github.com/Jij-Inc/ommx/issues/1082).
