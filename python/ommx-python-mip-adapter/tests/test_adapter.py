import pytest

from ommx.v1 import Instance, DecisionVariable, Constraint
from ommx.v1 import Function, Quadratic

from ommx_python_mip_adapter import OMMXPythonMIPAdapter

from ommx_python_mip_adapter.exception import OMMXPythonMIPAdapterError


def test_error_nonlinear_objective():
    # Objective function: 2.3 * x * x (variable ID should match)
    quadratic = Quadratic(columns=[0], rows=[0], values=[2.3])
    ommx_instance = Instance.from_components(
        decision_variables=[DecisionVariable.continuous(0)],
        objective=Function(quadratic),
        constraints=[],
        sense=Instance.MINIMIZE,
    )

    with pytest.raises(OMMXPythonMIPAdapterError) as e:
        OMMXPythonMIPAdapter(ommx_instance)
    assert "The function must be either `constant` or `linear`." in str(e.value)


def test_error_nonlinear_constraint():
    # Objective function: 0
    # Constraint: 2.3 * x * x = 0
    quadratic = Quadratic(columns=[1], rows=[1], values=[2.3])
    constraint = Constraint(
        id=0,
        function=Function(quadratic),
        equality=Constraint.EQUAL_TO_ZERO,
    )
    ommx_instance = Instance.from_components(
        decision_variables=[DecisionVariable.continuous(1)],
        objective=Function(0),
        constraints=[constraint],
        sense=Instance.MINIMIZE,
    )

    with pytest.raises(OMMXPythonMIPAdapterError) as e:
        OMMXPythonMIPAdapter(ommx_instance)
    assert "The function must be either `constant` or `linear`." in str(e.value)


# NOTE: This test case is commented out because the new API doesn't allow
# creating constraints with invalid equality values through factory methods.
# The validation happens at creation time, so this error case is no longer reachable.
# def test_error_not_supported_constraint_equality():
#     # This test would require creating a constraint with EQUALITY_UNSPECIFIED
#     # which is not possible with the new factory method API
