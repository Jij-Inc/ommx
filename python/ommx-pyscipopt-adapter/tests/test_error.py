import pytest
import pyscipopt

from ommx_pyscipopt_adapter import (
    instance_to_model,
    model_to_state,
    OMMXPySCIPOptAdapterError,
)

from ommx.v1 import Constraint, Instance, DecisionVariable, Polynomial
from ommx.v1.decision_variables_pb2 import DecisionVariable as _DecisionVariable
from ommx.v1.constraint_pb2 import Equality


def test_error_not_suppoerted_decision_variable():
    ommx_instance = Instance.from_components(
        decision_variables=[
            _DecisionVariable(id=1, kind=_DecisionVariable.KIND_UNSPECIFIED)
        ],
        objective=0,
        constraints=[],
        sense=Instance.MINIMIZE,
    )
    with pytest.raises(OMMXPySCIPOptAdapterError) as e:
        instance_to_model(ommx_instance)
    assert "Not supported decision variable" in str(e.value)


def test_error_polynomial_objective():
    # Objective function: 2.3 * x * x * x
    ommx_instance = Instance.from_components(
        decision_variables=[DecisionVariable.continuous(1)],
        objective=Polynomial(terms={(1, 1, 1): 2.3}),
        constraints=[],
        sense=Instance.MINIMIZE,
    )
    with pytest.raises(OMMXPySCIPOptAdapterError) as e:
        instance_to_model(ommx_instance)
    assert "The objective function must be" in str(e.value)


def test_error_nonlinear_constraint():
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
    with pytest.raises(OMMXPySCIPOptAdapterError) as e:
        instance_to_model(ommx_instance)
    assert "Constraints must be either `constant`, `linear` or `quadratic`." in str(
        e.value
    )


def test_error_not_supported_constraint_equality():
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
    with pytest.raises(OMMXPySCIPOptAdapterError) as e:
        instance_to_model(ommx_instance)
    assert "Not supported constraint equality" in str(e.value)


def test_error_not_optimized_model():
    model = pyscipopt.Model()
    instance = Instance.from_components(
        decision_variables=[],
        objective=0,
        constraints=[],
        sense=Instance.MINIMIZE,
    )
    with pytest.raises(OMMXPySCIPOptAdapterError) as e:
        model_to_state(model, instance)
    assert "The model may not be optimized." in str(e.value)


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
    model = instance_to_model(ommx_instance)
    model.optimize()
    with pytest.raises(OMMXPySCIPOptAdapterError) as e:
        model_to_state(model, ommx_instance)
    assert "There is no feasible solution." in str(e.value)


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
    with pytest.raises(OMMXPySCIPOptAdapterError) as e:
        instance_to_model(ommx_instance)
    assert "Infeasible constant constraint was found" in str(e.value)


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
    with pytest.raises(OMMXPySCIPOptAdapterError) as e:
        instance_to_model(ommx_instance)
    assert "Infeasible constant constraint was found" in str(e.value)
