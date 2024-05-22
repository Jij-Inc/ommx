from __future__ import annotations
from typing import overload
from pandas import DataFrame, concat, MultiIndex

from .function_pb2 import Function
from .solution_pb2 import State, Solution
from .instance_pb2 import Instance
from .constraint_pb2 import Constraint, EvaluatedConstraint
from .linear_pb2 import Linear
from .quadratic_pb2 import Quadratic
from .polynomial_pb2 import Polynomial

from .._ommx_rust import (
    evaluate_function,
    evaluate_linear,
    evaluate_quadratic,
    evaluate_polynomial,
    evaluate_constraint,
    evaluate_instance,
)


def decision_variables(obj: Instance | Solution) -> DataFrame:
    decision_variables = obj.decision_variables
    parameters = DataFrame(dict(v.description.parameters) for v in decision_variables)
    parameters.columns = MultiIndex.from_product([["parameters"], parameters.columns])
    df = DataFrame(
        {
            "id": v.id,
            "kind": v.kind,
            "lower": v.bound.lower,
            "upper": v.bound.upper,
            "name": v.description.name,
        }
        for v in decision_variables
    )
    df.columns = MultiIndex.from_product([df.columns, [""]])
    return concat([df, parameters], axis=1)


@overload
def evaluate(
    obj: Function | Linear | Quadratic | Polynomial, state: State
) -> tuple[float, set[int]]: ...


@overload
def evaluate(obj: Instance, state: State) -> tuple[Solution, set[int]]: ...


@overload
def evaluate(obj: Constraint, state: State) -> tuple[EvaluatedConstraint, set[int]]: ...


def evaluate(
    obj: Function | Linear | Quadratic | Polynomial | Constraint | Instance,
    state: State,
) -> tuple[float | EvaluatedConstraint | Solution, set[int]]:
    """
    Evaluate an object with the given state.

    Examples
    ---------

    - Ready an instance and a state using :class:`ommx.testing.SingleFeasibleLPGenerator`:

        >>> from ommx.testing import SingleFeasibleLPGenerator, DataType
        >>> generator = SingleFeasibleLPGenerator(3, DataType.INT)
        >>> instance = generator.get_v1_instance()
        >>> state = generator.get_v1_state()

    - Evaluate the objective function of the type :class:`function_pb2.Function` into a float value:

        >>> from ommx.v1 import evaluate
        >>> value, used_ids = evaluate(instance.objective, state)
        >>> assert isinstance(value, float)

    - Evaluate the entire :class:`instance_pb2.Instance` into a :class:`solution_pb2.Solution` object:

        >>> from ommx.v1 import Solution
        >>> solution, used_ids = evaluate(instance, state)
        >>> assert isinstance(solution, Solution)

    """
    obj_bytes = obj.SerializeToString()
    state_bytes = state.SerializeToString()
    if isinstance(obj, Linear):
        return evaluate_linear(obj_bytes, state_bytes)
    if isinstance(obj, Quadratic):
        return evaluate_quadratic(obj_bytes, state_bytes)
    if isinstance(obj, Polynomial):
        return evaluate_polynomial(obj_bytes, state_bytes)
    if isinstance(obj, Function):
        return evaluate_function(obj_bytes, state_bytes)
    if isinstance(obj, Constraint):
        out, used_ids = evaluate_constraint(obj_bytes, state_bytes)
        decoded = EvaluatedConstraint()
        decoded.ParseFromString(out)
        return decoded, used_ids
    if isinstance(obj, Instance):
        out, used_ids = evaluate_instance(obj_bytes, state_bytes)
        decoded = Solution()
        decoded.ParseFromString(out)
        return decoded, used_ids
    raise NotImplementedError(f"Evaluation for {type(obj)} is not implemented yet")
