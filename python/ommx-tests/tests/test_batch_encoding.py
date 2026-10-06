from itertools import product

import pytest

from ommx import DecisionVariable, Instance, InstanceClass, PreparationPolicy, Sense


@pytest.mark.parametrize("format_name", ["qubo", "hubo"])
@pytest.mark.parametrize("keyed", [False, True])
def test_encoding_precedes_penalty_and_preserves_output_semantics(format_name, keyed):
    x = DecisionVariable.integer(0, lower=0, upper=3)
    y = DecisionVariable.integer(1, lower=0, upper=1)
    instance = Instance.from_components(
        decision_variables=[x, y],
        objective=x + y,
        constraints={7: x + y == 2},
        sense=Sense.Minimize,
    )
    parametric = (
        instance.penalty_method() if keyed else instance.uniform_penalty_method()
    )
    reference = parametric.with_parameters({parametric.parameters[0].id: 3.0})
    reference.log_encode()
    expected = getattr(reference, f"as_{format_name}_format")()
    options = (
        {"penalty_weights": {7: 3.0}} if keyed else {"uniform_penalty_weight": 3.0}
    )
    assert getattr(instance, f"to_{format_name}")(**options) == expected
    policy = getattr(PreparationPolicy, f"for_{format_name}")(**options)
    instance.prepare(getattr(InstanceClass, format_name)(), policy)
    removed = instance.removed_constraints[7]
    assert removed.function.required_ids() == {2, 3, 4}
    for bits in product([0, 1], repeat=3):
        state = dict(zip([2, 3, 4], bits))
        decoded = bits[0] + 2 * bits[1] + bits[2]
        solution = instance.evaluate(state)
        assert instance.objective.evaluate(state) == decoded + 3 * (decoded - 2) ** 2
        assert solution.objective == decoded
        assert solution.feasible == (decoded == 2)
        assert removed.function.evaluate(state) == decoded - 2
