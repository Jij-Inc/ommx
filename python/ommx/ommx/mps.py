from ._ommx_rust import load_mps_bytes
from .v1 import Instance


def load_file(path: str) -> Instance:
    return Instance.load_mps(path)
