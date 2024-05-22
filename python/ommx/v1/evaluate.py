from __future__ import annotations
from typing import overload

from .function_pb2 import Function
from .solution_pb2 import State, Solution
from .instance_pb2 import Instance

from .._ommx_rust import evaluate_function


@overload
def evaluate(obj: Function, state: State) -> tuple[float, set[int]]: ...


@overload
def evaluate(obj: Instance, state: State) -> tuple[Solution, set[int]]: ...


def evaluate(
    obj: Function | Instance, state: State
) -> tuple[float | Solution, set[int]]:
    state_bytes = state.SerializeToString()
    if isinstance(obj, Function):
        function_bytes = obj.SerializeToString()
        return evaluate_function(function_bytes, state_bytes)

    raise NotImplementedError(f"Evaluation for {type(obj)} is not implemented yet")
