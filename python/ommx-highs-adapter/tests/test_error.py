import pytest

from ommx.v1.constraint_pb2 import Constraint as _Constraint, Equality
from ommx.v1.decision_variables_pb2 import DecisionVariable as _DecisionVariable
from ommx.v1.function_pb2 import Function
from ommx.v1.linear_pb2 import Linear
from ommx.v1.quadratic_pb2 import Quadratic
from ommx.v1 import Instance, DecisionVariable, Constraint
from ommx.adapter import InfeasibleDetected

from ommx_highs_adapter import OMMXHighsAdapter, OMMXHighsAdapterError


def test_error_unsupported_decision_variable():
    ommx_instance = Instance.from_components(
        decision_variables=[
            DecisionVariable(
                _DecisionVariable(id=1, kind=_DecisionVariable.KIND_UNSPECIFIED)
            )
        ],
        objective=Function(constant=0),
        constraints=[],
        sense=Instance.MINIMIZE,
    )
    with pytest.raises(OMMXHighsAdapterError) as e:
        OMMXHighsAdapter(ommx_instance)
    assert "Unsupported decision variable kind" in str(e.value)


def test_error_nonlinear_objective():
    # Objective function: 2.3 * x * x
    ommx_instance = Instance.from_components(
        decision_variables=[DecisionVariable.continuous(0)],
        objective=Function(
            quadratic=Quadratic(rows=[1], columns=[1], values=[2.3]),
        ),
        constraints=[],
        sense=Instance.MINIMIZE,
    )

    with pytest.raises(OMMXHighsAdapterError) as e:
        OMMXHighsAdapter(ommx_instance)
    assert "The function must be either `constant` or `linear`." in str(e.value)


def test_error_nonlinear_constraint():
    # Objective function: 0
    # Constraint: 2.3 * x * x = 0
    ommx_instance = Instance.from_components(
        decision_variables=[DecisionVariable.continuous(1)],
        objective=Function(
            constant=0,
        ),
        constraints=[
            _Constraint(
                function=Function(
                    quadratic=Quadratic(rows=[1], columns=[1], values=[2.3]),
                ),
                equality=Equality.EQUALITY_EQUAL_TO_ZERO,
            ),
        ],
        sense=Instance.MINIMIZE,
    )

    with pytest.raises(OMMXHighsAdapterError) as e:
        OMMXHighsAdapter(ommx_instance)
    assert "The function must be either `constant` or `linear`." in str(e.value)


def test_error_unsupported_constraint_equality():
    # Objective function: 0
    # Constraint: 2x ?? 0 (equality: unspecified)
    ommx_instance = Instance.from_components(
        decision_variables=[DecisionVariable.continuous(1)],
        objective=Function(constant=0),
        constraints=[
            _Constraint(
                function=Function(
                    linear=Linear(terms=[Linear.Term(id=1, coefficient=2)])
                ),
                equality=Equality.EQUALITY_UNSPECIFIED,
            ),
        ],
        sense=Instance.MINIMIZE,
    )

    with pytest.raises(OMMXHighsAdapterError) as e:
        OMMXHighsAdapter(ommx_instance)
    assert "Unsupported constraint equality kind" in str(e.value)


def test_error_infeasible_constant_equality_constraint():
    ommx_instance = Instance.from_components(
        decision_variables=[],
        objective=0,
        constraints=[
            Constraint(
                function=-1,
                equality=Constraint.EQUAL_TO_ZERO,
            ),
        ],
        sense=Instance.MINIMIZE,
    )
    with pytest.raises(OMMXHighsAdapterError) as e:
        OMMXHighsAdapter(ommx_instance)
    assert "Infeasible constant equality constraint" in str(e.value)


def test_error_infeasible_constant_inequality_constraint():
    ommx_instance = Instance.from_components(
        decision_variables=[],
        objective=0,
        constraints=[
            Constraint(
                function=1,
                equality=Constraint.LESS_THAN_OR_EQUAL_TO_ZERO,
            ),
        ],
        sense=Instance.MINIMIZE,
    )
    with pytest.raises(OMMXHighsAdapterError) as e:
        OMMXHighsAdapter(ommx_instance)
    assert "Infeasible constant inequality constraint" in str(e.value)


def test_error_infeasible_model():
    x = DecisionVariable.continuous(1)
    ommx_instance = Instance.from_components(
        decision_variables=[x],
        objective=0,
        constraints=[
            Constraint(
                function=x,
                equality=Constraint.EQUAL_TO_ZERO,
            ),
            Constraint(
                function=x - 1,
                equality=Constraint.EQUAL_TO_ZERO,
            ),
        ],
        sense=Instance.MINIMIZE,
    )
    with pytest.raises(InfeasibleDetected):
        OMMXHighsAdapter.solve(ommx_instance)
