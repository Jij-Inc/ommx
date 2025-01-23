from .v1 import Instance


def load_file(path: str) -> Instance:
    return Instance.load_qplib(path)
