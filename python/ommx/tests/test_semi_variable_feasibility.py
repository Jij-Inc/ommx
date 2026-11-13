import pytest
from ommx import Instance


@pytest.mark.parametrize("kind", ["semi_integer", "semi_continuous"])
@pytest.mark.parametrize("sign", [1, -1])
def test_semi_variable_feasibility(kind: str, sign: int):
    model = Instance.minimize()
    lower, upper = sorted((sign * 2, sign * 4))
    x = getattr(model, f"new_{kind}")(lower=lower, upper=upper)
    model.objective = x
    # Use an exactly representable tolerance to test the inclusive zero boundary.
    atol = 0.125
    candidates = [
        (0, True),
        (-0.125, True),
        (0.125, True),
        (-0.25, False),
        (0.25, False),
        (sign, False),
        (sign * 2, True),
        (sign * 2.5, kind == "semi_continuous"),
        (sign * 4, True),
        (sign * 5, False),
    ]
    states = {i: {x.id: value} for i, (value, _) in enumerate(candidates)}
    samples = model.evaluate_samples(states, atol=atol)
    expected = {i: feasible for i, (_, feasible) in enumerate(candidates)}
    assert samples.feasible == expected
    assert samples.feasible_relaxed == expected
    for i, (_, feasible) in enumerate(candidates):
        solution = model.evaluate(states[i], atol=atol)
        assert solution.feasible is feasible
        assert solution.feasible_relaxed is feasible
        assert samples.get(i).feasible is feasible
        assert samples.get(i).feasible_relaxed is feasible
