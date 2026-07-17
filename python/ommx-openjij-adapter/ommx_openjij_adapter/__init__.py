from .adapter import OMMXOpenJijSAAdapter as _OMMXOpenJijSAAdapter
from ._preparation import (
    OpenJijPreparation,
    OpenJijPreparationError,
    OpenJijPreparationFailure,
    OpenJijPreparationReport,
    OpenJijPreparationSourceCheck,
    OpenJijPreparationStep,
)
from ._decode import decode_to_samples


class OMMXOpenJijSAAdapter(_OMMXOpenJijSAAdapter):
    """
    Sample an applicable Binary polynomial input with OpenJij simulated annealing.

    The direct Adapter input must use only Binary decision variables, have
    no active regular or special constraints, and be a minimization problem.
    Arbitrary polynomial objective degree is supported through OpenJij's QUBO
    and Binary-HUBO paths.

    Integer encoding, sense reversal, slack introduction, and finite constraint
    penalties are explicit preparation operations, not part of the declared
    input class. Pass :attr:`OpenJijPreparation.input` back to this Adapter
    as a separate :class:`ommx.Instance` value.
    """


__all__ = [
    "OMMXOpenJijSAAdapter",
    "OpenJijPreparation",
    "OpenJijPreparationError",
    "OpenJijPreparationFailure",
    "OpenJijPreparationReport",
    "OpenJijPreparationSourceCheck",
    "OpenJijPreparationStep",
    "decode_to_samples",
]
