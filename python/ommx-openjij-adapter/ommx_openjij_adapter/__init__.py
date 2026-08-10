from .adapter import OMMXOpenJijSAAdapter as _OMMXOpenJijSAAdapter
from ._decode import decode_to_samples


class OMMXOpenJijSAAdapter(_OMMXOpenJijSAAdapter):
    """
    Sample an applicable Binary polynomial input with OpenJij simulated annealing.

    The direct Adapter input must use only Binary decision variables, have
    no active regular or special constraints, and be a minimization problem.
    Arbitrary polynomial objective degree is supported through OpenJij's QUBO
    and Binary-HUBO paths.

    Integer encoding, sense normalization, slack introduction, and finite
    constraint penalties are explicit preparation operations, not part of the
    declared input class. Start from
    :meth:`recommended_preparation_policy`, edit caller-owned choices such as
    finite penalty weights, and apply the policy with :meth:`Instance.prepare`.
    """


__all__ = [
    "OMMXOpenJijSAAdapter",
    "decode_to_samples",
]
