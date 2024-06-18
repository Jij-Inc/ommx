import mip

from ommx.v1 import Instance, DecisionVariable, Constraint

import ommx_python_mip_adapter as adapter


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

    ommx_instance = adapter.model_to_instance(model).raw

    assert ommx_instance.sense == Instance.MINIMIZE

    # Check the decision variables
    assert len(ommx_instance.decision_variables) == 3
    decision_variables_x1 = ommx_instance.decision_variables[0]
    assert decision_variables_x1.id == 0
    assert decision_variables_x1.kind == DecisionVariable.CONTINUOUS
    assert decision_variables_x1.bound.lower == CONTINUOUS_LOWER_BOUND
    assert decision_variables_x1.bound.upper == CONTINUOUS_UPPER_BOUND
    assert decision_variables_x1.name == "1"
    decision_variables_x2 = ommx_instance.decision_variables[1]
    assert decision_variables_x2.id == 1
    assert decision_variables_x2.kind == DecisionVariable.INTEGER
    assert decision_variables_x2.bound.lower == INTEGER_LOWER_BOUND
    assert decision_variables_x2.bound.upper == INTEGER_UPPER_BOUND
    assert decision_variables_x2.name == "2"
    decision_variables_x3 = ommx_instance.decision_variables[2]
    assert decision_variables_x3.id == 2
    assert decision_variables_x3.kind == DecisionVariable.BINARY
    assert decision_variables_x3.bound.lower == 0
    assert decision_variables_x3.bound.upper == 1
    assert decision_variables_x3.name == "3"

    # Check the objective function
    assert ommx_instance.objective.HasField("linear")
    objective = ommx_instance.objective.linear
    assert objective.constant == 0
    assert len(objective.terms) == 3
    objective_term_x1 = objective.terms[0]
    assert objective_term_x1.id == 0
    assert objective_term_x1.coefficient == -1
    objective_term_x2 = objective.terms[1]
    assert objective_term_x2.id == 1
    assert objective_term_x2.coefficient == -2
    objective_term_x3 = objective.terms[2]
    assert objective_term_x3.id == 2
    assert objective_term_x3.coefficient == -3

    # Check the constraints
    assert len(ommx_instance.constraints) == 3

    constraint1 = ommx_instance.constraints[0]
    assert constraint1.equality == Constraint.EQUAL_TO_ZERO
    assert constraint1.function.HasField("linear")
    assert constraint1.function.linear.constant == 6
    assert len(constraint1.function.linear.terms) == 2
    constraint1_term_x1 = constraint1.function.linear.terms[0]
    assert constraint1_term_x1.id == 0
    assert constraint1_term_x1.coefficient == 4
    constraint1_term_x2 = constraint1.function.linear.terms[1]
    assert constraint1_term_x2.id == 1
    assert constraint1_term_x2.coefficient == -5

    constraint2 = ommx_instance.constraints[1]
    assert constraint2.equality == Constraint.LESS_THAN_OR_EQUAL_TO_ZERO
    assert constraint2.function.HasField("linear")
    assert constraint2.function.linear.constant == -9
    assert len(constraint2.function.linear.terms) == 2
    constraint2_term_x1 = constraint2.function.linear.terms[0]
    assert constraint2_term_x1.id == 0
    assert constraint2_term_x1.coefficient == -7
    constraint2_term_x3 = constraint2.function.linear.terms[1]
    assert constraint2_term_x3.id == 2
    assert constraint2_term_x3.coefficient == 8

    constraint3 = ommx_instance.constraints[2]
    assert constraint3.equality == Constraint.LESS_THAN_OR_EQUAL_TO_ZERO
    assert constraint3.function.HasField("linear")
    assert constraint3.function.linear.constant == -12
    assert len(constraint3.function.linear.terms) == 2
    constraint3_term_x2 = constraint3.function.linear.terms[0]
    assert constraint3_term_x2.id == 1
    assert constraint3_term_x2.coefficient == -10
    constraint3_term_x3 = constraint3.function.linear.terms[1]
    assert constraint3_term_x3.id == 2
    assert constraint3_term_x3.coefficient == 11


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

    ommx_instance = adapter.model_to_instance(model).raw

    assert ommx_instance.sense == Instance.MAXIMIZE

    # Check the decision variables
    assert len(ommx_instance.decision_variables) == 2
    decision_variables_x1 = ommx_instance.decision_variables[0]
    assert decision_variables_x1.id == 0
    assert decision_variables_x1.kind == DecisionVariable.CONTINUOUS
    assert decision_variables_x1.bound.lower == LOWER_BOUND
    assert decision_variables_x1.bound.upper == UPPER_BOUND
    assert decision_variables_x1.name == "1"
    decision_variables_x2 = ommx_instance.decision_variables[1]
    assert decision_variables_x2.id == 1
    assert decision_variables_x2.kind == DecisionVariable.CONTINUOUS
    assert decision_variables_x2.bound.lower == LOWER_BOUND
    assert decision_variables_x2.bound.upper == UPPER_BOUND
    assert decision_variables_x2.name == "2"

    # check the objective function
    assert ommx_instance.objective.HasField("constant")
    assert ommx_instance.objective.constant == 0

    # Check the constraints
    assert len(ommx_instance.constraints) == 2

    constraint1 = ommx_instance.constraints[0]
    assert constraint1.equality == Constraint.EQUAL_TO_ZERO
    assert constraint1.function.HasField("linear")
    assert constraint1.function.linear.constant == -5
    assert len(constraint1.function.linear.terms) == 2
    constraint1_term_x1 = constraint1.function.linear.terms[0]
    assert constraint1_term_x1.id == 0
    assert constraint1_term_x1.coefficient == 1
    constraint1_term_x2 = constraint1.function.linear.terms[1]
    assert constraint1_term_x2.id == 1
    assert constraint1_term_x2.coefficient == 2

    constraint2 = ommx_instance.constraints[1]
    assert constraint2.equality == Constraint.EQUAL_TO_ZERO
    assert constraint2.function.HasField("linear")
    assert constraint2.function.linear.constant == -10
    assert len(constraint2.function.linear.terms) == 2
    constraint2_term_x1 = constraint2.function.linear.terms[0]
    assert constraint2_term_x1.id == 0
    assert constraint2_term_x1.coefficient == 4
    constraint2_term_x3 = constraint2.function.linear.terms[1]
    assert constraint2_term_x3.id == 1
    assert constraint2_term_x3.coefficient == 3
