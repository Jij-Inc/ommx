from __future__ import annotations
from typing import overload

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
        return EvaluatedConstraint().ParseFromString(out), used_ids
    if isinstance(obj, Instance):
        out, used_ids = evaluate_instance(obj_bytes, state_bytes)
        return Solution().ParseFromString(out), used_ids
    raise NotImplementedError(f"Evaluation for {type(obj)} is not implemented yet")
