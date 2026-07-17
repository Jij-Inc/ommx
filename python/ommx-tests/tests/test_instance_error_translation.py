import pytest

from ommx import (
    DecisionVariable,
    Instance,
    NamedFunction,
    Parameter,
    ParametricInstance,
)


def test_instance_from_components_rejects_duplicate_decision_variable_ids():
    variables = [DecisionVariable.binary(0), DecisionVariable.binary(0)]

    with pytest.raises(ValueError, match="Duplicate decision variable ID: 0"):
        Instance.from_components(
            decision_variables=variables,
            objective=0,
            constraints={},
            sense=Instance.MINIMIZE,
        )


def test_parametric_from_components_rejects_duplicate_decision_variable_ids():
    variables = [DecisionVariable.binary(0), DecisionVariable.binary(0)]

    with pytest.raises(ValueError, match="Duplicate decision variable ID: 0"):
        ParametricInstance.from_components(
            decision_variables=variables,
            parameters=[],
            objective=0,
            constraints={},
            sense=Instance.MINIMIZE,
        )


def test_parametric_from_components_rejects_duplicate_named_function_ids():
    variable = DecisionVariable.binary(0)
    parameter = Parameter(100)
    named_functions = [
        NamedFunction(id=0, function=variable + parameter),
        NamedFunction(id=0, function=variable - parameter),
    ]

    with pytest.raises(ValueError, match="Duplicate named function ID: 0"):
        ParametricInstance.from_components(
            decision_variables=[variable],
            parameters=[parameter],
            objective=variable + parameter,
            constraints={},
            sense=Instance.MINIMIZE,
            named_functions=named_functions,
        )


def test_to_qubo_rejects_missing_penalty_weight():
    x = [DecisionVariable.binary(i) for i in range(2)]
    instance = Instance.from_components(
        decision_variables=x,
        objective=x[0],
        constraints={123: x[0] == 0, 456: x[1] == 1},
        sense=Instance.MINIMIZE,
    )

    with pytest.raises(
        ValueError, match="No penalty weight provided for constraint ID 456"
    ):
        instance.to_qubo(penalty_weights={123: 1.0})
