import pytest
import gurobipy as gp

from ommx_gurobipy_adapter import (
    OMMXGurobipyAdapterError,
    OMMXGurobipyAdapter,
)

from ommx.adapter import InfeasibleDetected
from ommx.v1 import Constraint, Instance, DecisionVariable, Polynomial
from ommx.v1.decision_variables_pb2 import DecisionVariable as _DecisionVariable
from ommx.v1.constraint_pb2 import Equality


def test_error_not_suppoerted_decision_variable():
    """Test error when unsupported decision variable type is used"""
    ommx_instance = Instance.from_components(
        decision_variables=[
            _DecisionVariable(id=1, kind=_DecisionVariable.KIND_UNSPECIFIED)
        ],
        objective=0,
        constraints=[],
        sense=Instance.MINIMIZE,
    )
    with pytest.raises(OMMXGurobipyAdapterError) as e:
        OMMXGurobipyAdapter(ommx_instance)
    assert "Unsupported decision variable" in str(e.value)


def test_error_polynomial_objective():
    """Test error when polynomial objective is used"""
    # Objective function: 2.3 * x * x * x
    ommx_instance = Instance.from_components(
        decision_variables=[DecisionVariable.continuous(1)],
        objective=Polynomial(terms={(1, 1, 1): 2.3}),
        constraints=[],
        sense=Instance.MINIMIZE,
    )
    with pytest.raises(OMMXGurobipyAdapterError) as e:
        OMMXGurobipyAdapter(ommx_instance)
    assert "The objective function must be" in str(e.value)


def test_error_nonlinear_constraint():
    """Test error when nonlinear constraint is used"""
    # Objective function: 0
    # Constraint: 2.3 * x * x * x = 0
    ommx_instance = Instance.from_components(
        decision_variables=[DecisionVariable.continuous(1)],
        objective=0,
        constraints=[
            Constraint(
                function=Polynomial(terms={(1, 1, 1): 2.3}),
                equality=Constraint.EQUAL_TO_ZERO,
            ),
        ],
        sense=Instance.MINIMIZE,
    )
    with pytest.raises(OMMXGurobipyAdapterError) as e:
        OMMXGurobipyAdapter(ommx_instance)
    assert "Constraints must be either `constant`, `linear` or `quadratic`." in str(
        e.value
    )


def test_error_not_supported_constraint_equality():
    """Test error when unsupported constraint equality is used"""
    # Objective function: 0
    # Constraint: 2x ?? 0 (equality: unspecified)
    x = DecisionVariable.continuous(1)
    ommx_instance = Instance.from_components(
        decision_variables=[x],
        objective=0,
        constraints=[
            Constraint(
                function=2 * x,
                equality=Equality.EQUALITY_UNSPECIFIED,
            ),
        ],
        sense=Instance.MINIMIZE,
    )
    with pytest.raises(OMMXGurobipyAdapterError) as e:
        OMMXGurobipyAdapter(ommx_instance)
    assert "Not supported constraint equality" in str(e.value)


def test_error_not_optimized_model():
    """Test error when model is not optimized"""
    model = gp.Model()
    instance = Instance.from_components(
        decision_variables=[],
        objective=0,
        constraints=[],
        sense=Instance.MINIMIZE,
    )
    with pytest.raises(OMMXGurobipyAdapterError) as e:
        OMMXGurobipyAdapter(instance).decode_to_state(model)
    assert "The model may not be optimized." in str(e.value)


def test_error_infeasible_model():
    """Test error when model is infeasible"""
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
        OMMXGurobipyAdapter.solve(ommx_instance)


def test_error_infeasible_constant_equality_constraint():
    """Test error when infeasible constant equality constraint is used"""
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
    with pytest.raises(OMMXGurobipyAdapterError) as e:
        OMMXGurobipyAdapter(ommx_instance)
    assert "Infeasible constant constraint was found" in str(e.value)


def test_error_infeasible_constant_inequality_constraint():
    """Test error when infeasible constant inequality constraint is used"""
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
    with pytest.raises(OMMXGurobipyAdapterError) as e:
        OMMXGurobipyAdapter(ommx_instance)
    assert "Infeasible constant constraint was found" in str(e.value)
