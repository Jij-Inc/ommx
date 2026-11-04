import itertools
import math
import sys

import pytest

from ommx import Instance


METHODS = ("as_qubo_format", "as_hubo_format", "to_qubo", "to_hubo")


def energy(
    coefficients: dict[tuple[int, ...], float],
    offset: float,
    state: dict[int, float],
) -> float:
    return offset + sum(
        coefficient * math.prod(state[variable_id] for variable_id in ids)
        for ids, coefficient in coefficients.items()
    )


@pytest.mark.parametrize(
    ("method_name", "degree"),
    [
        (method_name, degree)
        for method_name in METHODS
        for degree in range(1, 4 if "hubo" in method_name else 3)
    ],
)
@pytest.mark.parametrize(
    "magnitude",
    [
        1e-20,
        sys.float_info.epsilon / 2,
        sys.float_info.epsilon,
        math.nextafter(sys.float_info.epsilon, math.inf),
    ],
)
@pytest.mark.parametrize("sign", [1, -1])
def test_formats_preserve_tiny_coefficients_and_offsets(
    method_name: str, degree: int, magnitude: float, sign: int
) -> None:
    instance = Instance.minimize()
    variables = [instance.new_binary(f"x{i}") for i in range(degree)]
    term = 1.0 * variables[0]
    for variable in variables[1:]:
        term = term * variable
    coefficient = sign * magnitude
    instance.objective = coefficient * term + coefficient
    source = instance.objective
    before = instance.to_v2_bytes()

    coefficients, offset = getattr(instance, method_name)()

    ids = tuple(variable.id for variable in variables)
    key = ids * 2 if "qubo" in method_name and degree == 1 else ids
    assert coefficients == {key: coefficient}
    assert offset == coefficient
    for values in itertools.product((0.0, 1.0), repeat=degree):
        state = dict(zip(ids, values))
        # Exact comparisons are intentional: default approximate tolerances
        # would allow a zero export to pass for these tiny objectives.
        assert energy(coefficients, offset, state) == source.evaluate(state)
    if method_name.startswith("as_"):
        assert instance.to_v2_bytes() == before


@pytest.mark.parametrize("method_name", METHODS)
def test_tiny_linear_objective_keeps_unique_minimizer(method_name: str) -> None:
    instance = Instance.minimize()
    x = instance.new_binary("x")
    instance.objective = 1e-20 * x + 1e-20

    coefficients, offset = getattr(instance, method_name)()

    energies = [energy(coefficients, offset, {x.id: value}) for value in (0.0, 1.0)]
    assert energies == [instance.evaluate({x.id: value}).objective for value in (0, 1)]
    assert energies == [1e-20, 2e-20]
    assert energies[0] < energies[1]


@pytest.mark.parametrize("method_name", METHODS)
@pytest.mark.parametrize("residual", [0.0, 2.0**-53, -(2.0**-53)])
def test_binary_key_aggregation_preserves_nonzero_residuals(
    method_name: str, residual: float
) -> None:
    instance = Instance.minimize()
    x = instance.new_binary("x")
    sign = -1 if residual < 0 else 1
    instance.objective = sign * (x - (1.0 - abs(residual)) * x * x)
    source = instance.objective
    before = instance.to_v2_bytes()

    coefficients, offset = getattr(instance, method_name)()

    key = (x.id, x.id) if "qubo" in method_name else (x.id,)
    assert coefficients == ({key: residual} if residual else {})
    assert offset == 0.0
    for value in (0.0, 1.0):
        state = {x.id: value}
        assert energy(coefficients, offset, state) == source.evaluate(state)
    if method_name.startswith("as_"):
        assert instance.to_v2_bytes() == before


def test_qubo_rejects_tiny_term_with_three_distinct_variables() -> None:
    instance = Instance.minimize()
    x, y, z = [instance.new_binary(name) for name in ("x", "y", "z")]
    instance.objective = 1e-20 * x * y * z
    before = instance.to_v2_bytes()

    with pytest.raises(RuntimeError, match="Invalid ID for QUBO"):
        instance.as_qubo_format()

    assert instance.to_v2_bytes() == before
