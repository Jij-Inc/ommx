from .v1 import Instance


def load_file(path: str) -> Instance:
    return Instance.load_mps(path)


def write_file(instance: Instance, path: str):
    instance.write_mps(path)
