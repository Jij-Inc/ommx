from .v1 import Instance


def load_file(path: str) -> Instance:
    return Instance.load_mps(path)


def write_file(instance: Instance, path: str):
    """
    Outputs the instance as an MPS file.

    - The outputted file is compressed by gzip.
    - Only linear problems are supported.
    - Various forms of metadata, like problem description and variable/constraint names, are not preserved.
    """
    instance.write_mps(path)
