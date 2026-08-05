from .adapter import OMMXOpenJijSAAdapter as _OMMXOpenJijSAAdapter
from ._decode import decode_to_samples


class OMMXOpenJijSAAdapter(_OMMXOpenJijSAAdapter):
    """
    Sample an applicable Binary polynomial input with OpenJij simulated annealing.

    The direct Adapter input must use only Binary decision variables, have
    no active regular or special constraints, and be a minimization problem.
    Arbitrary polynomial objective degree is supported through OpenJij's QUBO
    and Binary-HUBO paths.

    Integer slack, Integer encoding, sense normalization, and finite penalties
    are explicit OMMX preparation operations, not part of the declared input
    class. Use :meth:`recommended_preparation_policy` with
    :meth:`ommx.Instance.prepare` before passing that same Instance to the
    direct Adapter API. The recommendation tries the exact Integer-slack owner
    operation first and uses its approximate counterpart only when exact slack
    is unavailable.
    """


__all__ = [
    "OMMXOpenJijSAAdapter",
    "decode_to_samples",
]
