from . import Instance, State


def load_file(path: str) -> Instance:
    return Instance.load_qplib(path)


def load_solution(path: str, *, num_variables: int) -> State:
    """Load a published QPLIB solution; see :meth:`ommx.State.load_qplib_solution`."""
    return State.load_qplib_solution(path, num_variables=num_variables)
