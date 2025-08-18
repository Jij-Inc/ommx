# Import ATol functions to top-level namespace
from ._ommx_rust import get_default_atol, set_default_atol

__all__ = ["get_default_atol", "set_default_atol"]
