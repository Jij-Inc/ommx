from . import Instance


def load_file(path: str) -> Instance:
    return Instance.load_mps(path)


def write_file(instance: Instance, path: str):
    """
    Write the active formulation to a gzip-compressed MPS file.

    Equivalent to :meth:`instance.save_mps(path) <ommx.Instance.save_mps>`.
    Compression is enabled regardless of the filename extension. Use
    :meth:`ommx.Instance.save_mps` with ``compress=False`` for plain text.

    Linear and quadratic objectives and regular constraints are supported,
    subject to that method's variable-kind and special-constraint restrictions.
    See :meth:`ommx.Instance.save_mps` for the exact supported models, errors,
    variable filtering, and information lost during export.
    """
    instance.save_mps(path)
