"""Public test-data generation must work outside the small proptest model space."""

import math
import sys

import pytest
from ommx import DecisionVariable, Instance, Rng, Sense


def _instance(variable: DecisionVariable) -> Instance:
    return Instance.from_components(
        decision_variables=[variable],
        objective=variable,
        active_constraints={},
        sense=Sense.Minimize,
    )


@pytest.mark.parametrize(
    "kind", ["integer", "continuous", "semi_integer", "semi_continuous"]
)
@pytest.mark.parametrize(
    ("lower", "upper"),
    [
        (-math.inf, math.inf),
        (0.0, math.inf),
        (100.0, math.inf),
        (101.0, math.inf),
        (-math.inf, -101.0),
        (101.0, 102.0),
        (101.2, 102.8),
        (-102.0, -101.0),
        (101.0, 101.0),
        (0.0, 0.0),
        (float(2**53), float(2**53 + 4)),
        (float(2**63), float(2**63 + 4096)),
        (-float(2**64), -float(2**63)),
        (1e100, math.inf),
        (-math.inf, -1e100),
        (sys.float_info.max, math.inf),
        (-math.inf, -sys.float_info.max),
        (-sys.float_info.max, sys.float_info.max),
    ],
)
def test_random_states_and_samples_respect_domains(kind, lower, upper):
    variable = getattr(DecisionVariable, kind)(0, lower=lower, upper=upper)
    model = _instance(variable)
    rng = Rng()
    states = [model.random_state(rng) for _ in range(16)]
    samples = model.random_samples(rng, num_different_samples=16, num_samples=20)
    states.extend(samples.get_state(sample_id) for sample_id in samples.sample_ids())
    for state in states:
        assert set(state.entries) == {0}
        value = state.entries[0]
        assert math.isfinite(value)
        assert lower <= value <= upper or (kind.startswith("semi_") and value == 0)
        if "integer" in kind:
            assert value == round(value)
        assert model.evaluate(state).feasible

    # The same RNG must remain usable after every formerly panicking case.
    binary = _instance(DecisionVariable.binary(0))
    assert binary.random_state(rng).entries[0] in (0, 1)


@pytest.mark.parametrize(
    ("lower", "upper"),
    [
        (0.0, math.ulp(0.0)),
        (-math.ulp(0.0), math.ulp(0.0)),
        (101.5, math.nextafter(101.5, math.inf)),
        (sys.float_info.max, sys.float_info.max),
    ],
)
def test_random_continuous_narrow_intervals(lower, upper):
    model = _instance(DecisionVariable.continuous(0, lower=lower, upper=upper))
    rng = Rng()
    for _ in range(16):
        value = model.random_state(rng).entries[0]
        assert math.isfinite(value)
        assert lower <= value <= upper


def test_random_finite_domain_uses_members_not_hull():
    values = [-sys.float_info.max, 101.5, sys.float_info.max]
    model = _instance(DecisionVariable.finite_domain(0, values=values))
    rng = Rng()
    samples = model.random_samples(rng, num_different_samples=16, num_samples=20)
    for sample_id in samples.sample_ids():
        assert samples.get_state(sample_id).entries[0] in values


def test_random_state_does_not_require_constraint_feasibility():
    x = DecisionVariable.binary(0)
    model = Instance.from_components(
        decision_variables=[x],
        objective=x,
        active_constraints={0: x >= 2},
        sense=Sense.Minimize,
    )
    rng = Rng()
    state = model.random_state(rng)
    assert state.entries[0] in (0, 1)
    assert not model.evaluate(state).feasible
    samples = model.random_samples(rng, num_different_samples=3, num_samples=5)
    for sample_id in samples.sample_ids():
        assert not model.evaluate(samples.get_state(sample_id)).feasible


def test_invalid_sample_request_does_not_consume_or_poison_rng():
    model = _instance(DecisionVariable.integer(0, lower=101))
    rng = Rng()
    reference = Rng()
    with pytest.raises(ValueError):
        model.random_samples(rng, num_different_samples=2, num_samples=1)
    for _ in range(8):
        assert model.random_state(rng).entries == model.random_state(reference).entries


def test_singleton_domain_can_generate_repeated_groups():
    model = _instance(DecisionVariable.integer(0, lower=101, upper=101))
    samples = model.random_samples(Rng(), num_different_samples=3, num_samples=5)
    assert samples.num_samples() == 5
    for sample_id in samples.sample_ids():
        assert samples.get_state(sample_id).entries == {0: 101.0}


def test_semi_integer_with_empty_interval_generates_zero():
    model = _instance(DecisionVariable.semi_integer(0, lower=0.1, upper=0.9))
    samples = model.random_samples(Rng(), num_different_samples=3, num_samples=5)
    for sample_id in samples.sample_ids():
        state = samples.get_state(sample_id)
        assert state.entries == {0: 0.0}
        assert model.evaluate(state).feasible
