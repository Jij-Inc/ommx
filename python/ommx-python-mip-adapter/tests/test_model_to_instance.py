import mip

from ommx.v1 import DecisionVariable
from ommx.v1 import Instance
from ommx._ommx_rust import Equality

from ommx_python_mip_adapter import model_to_instance


def test_milp():
    # Objective function: - x1 - 2x2 - 3x3
    # Constraints:
    #     4x1 - 5x2 - 6 = 0
    #     -7x1 + 8x3 - 9 <= 0
    #     -10 <= x1 <= 10    (x: continuous)
    #     -11 <= x2 <= 11    (x: integer)
    #     x = 0 or 1    (x: binary)
    CONTINUOUS_LOWER_BOUND = -10
    CONTINUOUS_UPPER_BOUND = 10
    INTEGER_LOWER_BOUND = -11
    INTEGER_UPPER_BOUND = 11

    model = mip.Model(sense=mip.MINIMIZE)

    x1 = model.add_var(
        name="1",
        var_type=mip.CONTINUOUS,
        lb=CONTINUOUS_LOWER_BOUND,  # type: ignore
        ub=CONTINUOUS_UPPER_BOUND,  # type: ignore
    )
    x2 = model.add_var(
        name="2",
        var_type=mip.INTEGER,
        lb=INTEGER_LOWER_BOUND,  # type: ignore
        ub=INTEGER_UPPER_BOUND,  # type: ignore
    )
    x3 = model.add_var(
        name="3",
        var_type=mip.BINARY,
    )

    model.objective = -1 * x1 - 2 * x2 - 3 * x3  # type: ignore

    model.add_constr(4 * x1 - 5 * x2 + 6 == 0)  # type: ignore
    model.add_constr(-7 * x1 + 8 * x3 - 9 <= 0)  # type: ignore
    model.add_constr(10 * x2 - 11 * x3 + 12 >= 0)  # type: ignore

    ommx_instance = model_to_instance(model)

    assert ommx_instance.sense == Instance.MINIMIZE

    # Check the decision variables using .raw for direct dict access
    assert len(ommx_instance.raw.decision_variables) == 3
    decision_variables_x1 = ommx_instance.raw.decision_variables[0]
    assert decision_variables_x1.id == 0
    assert decision_variables_x1.kind == DecisionVariable.CONTINUOUS
    assert decision_variables_x1.bound.lower == CONTINUOUS_LOWER_BOUND
    assert decision_variables_x1.bound.upper == CONTINUOUS_UPPER_BOUND
    assert decision_variables_x1.name == "1"
    decision_variables_x2 = ommx_instance.raw.decision_variables[1]
    assert decision_variables_x2.id == 1
    assert decision_variables_x2.kind == DecisionVariable.INTEGER
    assert decision_variables_x2.bound.lower == INTEGER_LOWER_BOUND
    assert decision_variables_x2.bound.upper == INTEGER_UPPER_BOUND
    assert decision_variables_x2.name == "2"
    decision_variables_x3 = ommx_instance.raw.decision_variables[2]
    assert decision_variables_x3.id == 2
    assert decision_variables_x3.kind == DecisionVariable.BINARY
    assert decision_variables_x3.bound.lower == 0
    assert decision_variables_x3.bound.upper == 1
    assert decision_variables_x3.name == "3"

    # Check the objective function
    objective_linear = ommx_instance.raw.objective.as_linear()
    assert objective_linear is not None
    assert objective_linear.constant_term() == 0
    linear_terms = objective_linear.linear_terms()
    assert len(linear_terms) == 3
    assert linear_terms[0] == -1
    assert linear_terms[1] == -2
    assert linear_terms[2] == -3

    # Check the constraints using .raw for access
    assert len(ommx_instance.raw.constraints) == 3

    constraint1 = ommx_instance.raw.constraints[0]
    assert constraint1.equality == Equality.EqualToZero
    constraint1_linear = constraint1.function.as_linear()
    assert constraint1_linear is not None
    assert constraint1_linear.constant_term() == 6
    constraint1_terms = constraint1_linear.linear_terms()
    assert len(constraint1_terms) == 2
    assert constraint1_terms[0] == 4
    assert constraint1_terms[1] == -5

    constraint2 = ommx_instance.raw.constraints[1]
    assert constraint2.equality == Equality.LessThanOrEqualToZero
    constraint2_linear = constraint2.function.as_linear()
    assert constraint2_linear is not None
    assert constraint2_linear.constant_term() == -9
    constraint2_terms = constraint2_linear.linear_terms()
    assert len(constraint2_terms) == 2
    assert constraint2_terms[0] == -7
    assert constraint2_terms[2] == 8

    constraint3 = ommx_instance.raw.constraints[2]
    assert constraint3.equality == Equality.LessThanOrEqualToZero
    constraint3_linear = constraint3.function.as_linear()
    assert constraint3_linear is not None
    assert constraint3_linear.constant_term() == -12
    constraint3_terms = constraint3_linear.linear_terms()
    assert len(constraint3_terms) == 2
    assert constraint3_terms[1] == -10
    assert constraint3_terms[2] == 11


def test_no_objective_model():
    # Objective function: 0    (unspecified)
    # Constraints:
    #     x1 + 2x2 - 5 = 0
    #     4x1 + 3x2 - 10 = 0
    #     -15 <= x1 <= 15   (x: continuous)
    #     -15 <= x2 <= 15   (x: continuous)
    LOWER_BOUND = -15
    UPPER_BOUND = 15

    model = mip.Model(sense=mip.MAXIMIZE)

    x1 = model.add_var(
        name="1",
        var_type=mip.CONTINUOUS,
        lb=LOWER_BOUND,  # type: ignore
        ub=UPPER_BOUND,  # type: ignore
    )
    x2 = model.add_var(
        name="2",
        var_type=mip.CONTINUOUS,
        lb=LOWER_BOUND,  # type: ignore
        ub=UPPER_BOUND,  # type: ignore
    )

    model.add_constr(1 * x1 + 2 * x2 - 5 == 0)  # type: ignore
    model.add_constr(4 * x1 + 3 * x2 - 10 == 0)  # type: ignore

    ommx_instance = model_to_instance(model)

    assert ommx_instance.sense == Instance.MAXIMIZE

    # Check the decision variables using .raw for direct dict access
    assert len(ommx_instance.raw.decision_variables) == 2
    decision_variables_x1 = ommx_instance.raw.decision_variables[0]
    assert decision_variables_x1.id == 0
    assert decision_variables_x1.kind == DecisionVariable.CONTINUOUS
    assert decision_variables_x1.bound.lower == LOWER_BOUND
    assert decision_variables_x1.bound.upper == UPPER_BOUND
    assert decision_variables_x1.name == "1"
    decision_variables_x2 = ommx_instance.raw.decision_variables[1]
    assert decision_variables_x2.id == 1
    assert decision_variables_x2.kind == DecisionVariable.CONTINUOUS
    assert decision_variables_x2.bound.lower == LOWER_BOUND
    assert decision_variables_x2.bound.upper == UPPER_BOUND
    assert decision_variables_x2.name == "2"

    # check the objective function - should be a zero constant
    assert ommx_instance.raw.objective.degree() == 0
    assert ommx_instance.raw.objective.num_terms() == 0  # Zero function has 0 terms

    # Check the constraints using .raw for access
    assert len(ommx_instance.raw.constraints) == 2

    constraint1 = ommx_instance.raw.constraints[0]
    assert constraint1.equality == Equality.EqualToZero
    constraint1_linear = constraint1.function.as_linear()
    assert constraint1_linear is not None
    assert constraint1_linear.constant_term() == -5
    constraint1_terms = constraint1_linear.linear_terms()
    assert len(constraint1_terms) == 2
    assert constraint1_terms[0] == 1
    assert constraint1_terms[1] == 2

    constraint2 = ommx_instance.raw.constraints[1]
    assert constraint2.equality == Equality.EqualToZero
    constraint2_linear = constraint2.function.as_linear()
    assert constraint2_linear is not None
    assert constraint2_linear.constant_term() == -10
    constraint2_terms = constraint2_linear.linear_terms()
    assert len(constraint2_terms) == 2
    assert constraint2_terms[0] == 4
    assert constraint2_terms[1] == 3
