import pytest

from ommx import Constraint, DecisionVariable, Equality, Instance, Sense
from ommx_highs_adapter import OMMXHighsAdapter


@pytest.mark.parametrize("constant_id,bound_id", [(None, 42), (7, 42), (0, 1)])
def test_dual_mapping_uses_solver_rows(constant_id, bound_id):
    x = DecisionVariable.continuous(0, lower=0, upper=10)
    constraints = {bound_id: x <= 2}
    if constant_id is not None:
        constraints[constant_id] = Constraint(function=0, equality=Equality.EqualToZero)
    instance = Instance.from_components(
        decision_variables=[x],
        objective=x,
        sense=Sense.Maximize,
        constraints=constraints,
    )
    adapter = OMMXHighsAdapter(instance)
    model = adapter.solver_input
    model.run()
    solution = adapter.decode(model)
    assert model.getNumRow() == 1
    assert solution.get_dual_variable(bound_id) == pytest.approx(1)
    if constant_id is not None:
        assert solution.get_dual_variable(constant_id) is None
