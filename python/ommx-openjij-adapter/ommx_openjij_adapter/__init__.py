from .adapter import OMMXOpenJijSAAdapter as _OMMXOpenJijSAAdapter
from ._decode import decode_to_samples


class OMMXOpenJijSAAdapter(_OMMXOpenJijSAAdapter):
    """
    Sample an applicable Binary polynomial input with OpenJij simulated annealing.

    The direct Adapter input must use only Binary decision variables, have
    no active regular or special constraints, and be a minimization problem.
    Arbitrary polynomial objective degree is supported through OpenJij's QUBO
    and Binary-HUBO paths.

    :meth:`sample` and :meth:`solve` prepare an isolated copy with
    :meth:`recommended_preparation_policy`. Use :meth:`sample_strict` or
    :meth:`solve_strict` after explicitly preparing an instance when
    caller-owned choices such as fixed penalty magnitudes are required.
    """


__all__ = [
    "OMMXOpenJijSAAdapter",
    "decode_to_samples",
]
