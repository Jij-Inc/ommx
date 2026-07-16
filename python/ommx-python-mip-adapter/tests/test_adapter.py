import pytest
from ommx import Instance, DecisionVariable, OneHotConstraint
from ommx_python_mip_adapter import OMMXPythonMIPAdapter
from ommx_python_mip_adapter.exception import OMMXPythonMIPAdapterError


def test_constructor_lowers_one_hot_constraint():
    x = [DecisionVariable.binary(i) for i in range(2)]
    instance = Instance.from_components(
        decision_variables=x,
        objective=sum(x),
        constraints={},
        one_hot_constraints={7: OneHotConstraint(variables=x)},
        sense=Instance.MINIMIZE,
    )

    adapter = OMMXPythonMIPAdapter(instance)

    assert adapter.instance is instance
    assert instance.one_hot_constraints == {}
    assert set(instance.removed_one_hot_constraints) == {7}
    assert len(instance.constraints) == 1
    assert len(adapter.solver_input.constrs) == 1


def test_error_nonlinear_objective():
    # Objective function: 2.3 * x * x (variable ID should match)
    x = DecisionVariable.continuous(0)
    ommx_instance = Instance.from_components(
        decision_variables=[x],
        objective=2.3 * x * x,
        constraints={},
        sense=Instance.MINIMIZE,
    )

    with pytest.raises(OMMXPythonMIPAdapterError) as e:
        OMMXPythonMIPAdapter(ommx_instance)
    assert (
        "Function with degree 2 is not supported. Only linear (degree 1) and constant (degree 0) functions are supported."
        in str(e.value)
    )


def test_error_nonlinear_constraint():
    # Objective function: 0
    # Constraint: 2.3 * x * x = 0
    x = DecisionVariable.continuous(1)
    ommx_instance = Instance.from_components(
        decision_variables=[x],
        objective=0.0,
        constraints={0: 2.3 * x * x == 0},
        sense=Instance.MINIMIZE,
    )

    with pytest.raises(OMMXPythonMIPAdapterError) as e:
        OMMXPythonMIPAdapter(ommx_instance)
    assert (
        "Function with degree 2 is not supported. Only linear (degree 1) and constant (degree 0) functions are supported."
        in str(e.value)
    )
