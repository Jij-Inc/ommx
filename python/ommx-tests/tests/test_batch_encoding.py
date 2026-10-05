from copy import deepcopy
from itertools import product

import pytest

from ommx.v1 import DecisionVariable, Instance


@pytest.mark.parametrize("format_name", ["qubo", "hubo"])
@pytest.mark.parametrize("keyed", [False, True])
def test_driver_encodes_before_penalty_and_preserves_energy(format_name, keyed):
    x = DecisionVariable.integer(0, lower=0, upper=3)
    y = DecisionVariable.integer(1, lower=0, upper=1)
    instance = Instance.from_components(
        decision_variables=[x, y],
        objective=x + y,
        constraints=[(x + y == 2).set_id(7)],
        sense=Instance.MINIMIZE,
    )
    reference = deepcopy(instance)
    if keyed:
        parametric = reference.penalty_method()
    else:
        parametric = reference.uniform_penalty_method()
    reference = parametric.with_parameters({parametric.parameters[0].id: 3.0})
    reference.log_encode()
    expected = getattr(reference, f"as_{format_name}_format")()
    options = (
        {"penalty_weights": {7: 3.0}} if keyed else {"uniform_penalty_weight": 3.0}
    )
    assert getattr(instance, f"to_{format_name}")(**options) == expected
    removed = instance.get_removed_constraint_by_id(7)
    assert removed.function.used_decision_variable_ids() == {2, 3, 4}
    for bits in product([0, 1], repeat=3):
        state = dict(zip([2, 3, 4], bits))
        decoded = bits[0] + 2 * bits[1] + bits[2]
        solution = instance.evaluate(state)
        assert solution.objective == decoded + 3 * (decoded - 2) ** 2
        assert solution.feasible == (decoded == 2)
        assert removed.function.evaluate(state) == decoded - 2


def test_batch_log_encode_rejects_invalid_later_variable_atomically():
    x = DecisionVariable.integer(0, lower=0, upper=3)
    y = DecisionVariable.integer(1, lower=0, upper=float("inf"))
    instance = Instance.from_components(
        decision_variables=[x, y],
        objective=x + y,
        constraints=[],
        sense=Instance.MINIMIZE,
    )
    before = instance.to_bytes()
    with pytest.raises(RuntimeError, match="Bound must be finite"):
        instance.log_encode()
    assert instance.to_bytes() == before
