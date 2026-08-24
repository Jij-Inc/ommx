# ommx-openjij-adapter

Provides an adapter between [OMMX](https://github.com/Jij-Inc/ommx) and
[OpenJij](https://www.openjij.org/).

## Usage

`ommx-openjij-adapter` can be installed from PyPI:

```bash
pip install ommx-openjij-adapter
```

The easy API prepares an isolated copy with the Adapter's recommended policy,
so it can directly accept inputs such as an Integer maximization model without
modifying the caller's `Instance`:

```python
sample_set = OMMXOpenJijSAAdapter.sample(instance, num_reads=16)
```

Preparation choices that require application knowledge remain explicit. For
example, prepare a copy with a selected fixed penalty weight and use the
preparation-free API for a constrained model:

```python markdown-code-runner
import copy

from ommx import DecisionVariable, FixedPenaltyPreparation, Instance
from ommx_openjij_adapter import OMMXOpenJijSAAdapter

x = DecisionVariable.binary(0, name="x")
instance = Instance.from_components(
    decision_variables=[x],
    objective=x,
    constraints={0: x == 1},
    sense=Instance.MINIMIZE,
)

prepared = copy.copy(instance)
policy = OMMXOpenJijSAAdapter.recommended_preparation_policy()
policy.fixed_penalty = FixedPenaltyPreparation.uniform_penalty_method_with_fixed_weight(
    weight=2.0
)
prepared.prepare(OMMXOpenJijSAAdapter.INPUT_CLASS, policy)

sample_set = OMMXOpenJijSAAdapter.sample_without_preparation(
    prepared,
    num_reads=16,
)
print(sample_set.summary)
```

The fixed penalty weight is part of the preparation policy, not an OpenJij
sampler parameter. The adapter deliberately does not choose a magnitude: zero
adds no preference for feasibility, while a sufficiently large positive value
is application-specific and still does not guarantee that every returned
sample is feasible.

## Accepted models and recommended preparation

This adapter directly accepts:

- Binary decision variables
- a polynomial objective of any degree (QUBO or Binary HUBO)
- no active regular or special constraints
- minimization

For the shared semantics of adapter input classes and preparation, see
[Adapter Input Classes and Explicit Constraint Lowering](https://jij-inc-ommx.readthedocs-hosted.com/en/latest/user_guide/capability_model.html).

When building OpenJij sampler input, used variable IDs must fit a signed 64-bit
integer and converted interaction coefficients must be finite. Violations are
reported as conversion errors.

`recommended_preparation_policy()` returns a fresh editable
`ommx.PreparationPolicy`. It recommends:

- lowering active Indicator, OneHot, and SOS1 constraints;
- converting the active objective from maximization to minimization;
- attempting exact Integer slack with range 32, while permitting
  inequality-preserving Integer slack with upper bound 32 when exact equality
  conversion is unavailable; and
- log-encoding all used Integer variables.

Fixed penalty remains disabled until the caller selects either a uniform
magnitude or magnitudes keyed by active regular constraint ID. Uniform weights
are usually the convenient choice when special-constraint lowering creates
regular constraints with generated IDs. OMMX validates the weight domain; the
caller remains responsible for selecting a sufficient magnitude.

Pass the prepared `Instance` to `sample_without_preparation`. It remains the evaluation
owner for the returned `SampleSet` and retains the data needed to restore
source-variable values and evaluate removed constraints. The easy `sample`
API performs the copy and recommended preparation automatically when no custom
preparation choices are needed.

Integer log encoding follows the representability and bit-count limits of the
OMMX encoding operation. OMMX does not yet implement `Kind::Spin`; direct
OpenJij Spin input is tracked separately in
[OMMX issue #1082](https://github.com/Jij-Inc/ommx/issues/1082).
