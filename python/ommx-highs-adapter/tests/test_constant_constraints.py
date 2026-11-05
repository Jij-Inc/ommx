import highspy
import pytest

from ommx import Constraint, DecisionVariable, Equality, Instance, Sense
from ommx.adapter import InfeasibleDetected
from ommx_highs_adapter import OMMXHighsAdapter


@pytest.mark.parametrize("variable_mode", ["none", "unused", "active"])
@pytest.mark.parametrize(
    "equality,constant,feasible",
    [
        (Equality.EqualToZero, -1, False),
        (Equality.EqualToZero, 0, True),
        (Equality.EqualToZero, 1, False),
        (Equality.LessThanOrEqualToZero, -1, True),
        (Equality.LessThanOrEqualToZero, 0, True),
        (Equality.LessThanOrEqualToZero, 1, False),
    ],
)
def test_constant_constraint_solver_lifecycle(
    variable_mode, equality, constant, feasible
):
    x = DecisionVariable.continuous(11, lower=0, upper=1)
    instance = Instance.from_components(
        decision_variables=[] if variable_mode == "none" else [x],
        objective=x + 7 if variable_mode == "active" else 7,
        constraints={42: Constraint(function=constant, equality=equality)},
        sense=Sense.Minimize,
    )
    before = instance.to_v2_bytes()
    assert OMMXHighsAdapter.check_applicability(instance).is_member
    adapter = OMMXHighsAdapter(instance)
    model = adapter.solver_input
    assert model.getModelStatus() == highspy.HighsModelStatus.kNotset
    assert model.getNumRow() == 1
    lp = model.getLp()
    assert lp.row_upper_[0] == -constant
    assert lp.row_lower_[0] == (
        -constant if equality == Equality.EqualToZero else -highspy.kHighsInf
    )

    assert model.run() == highspy.HighsStatus.kOk
    if feasible:
        assert model.getModelStatus() == highspy.HighsModelStatus.kOptimal
        state = adapter.decode_to_state(model)
        assert state.entries == ({11: 0.0} if variable_mode == "active" else {})
        solution = adapter.decode(model)
        assert solution.feasible
        assert solution.objective == 7
    else:
        assert model.getModelStatus() == highspy.HighsModelStatus.kInfeasible
        with pytest.raises(InfeasibleDetected):
            adapter.decode(model)

    for solve in (OMMXHighsAdapter.solve, OMMXHighsAdapter.solve_without_preparation):
        if feasible:
            solution = solve(instance)
            assert solution.feasible
            assert solution.objective == 7
        else:
            with pytest.raises(InfeasibleDetected):
                solve(instance)
    assert instance.to_v2_bytes() == before
