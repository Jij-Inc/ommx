from .adapter import OMMXOpenJijSAAdapter as _OMMXOpenJijSAAdapter
from ._preparation import (
    OpenJijPreparation,
    OpenJijPreparationConfig,
    OpenJijPreparationError,
    OpenJijPreparationFailure,
    OpenJijPreparationReport,
    OpenJijPreparationSourceCheck,
    OpenJijPreparationStep,
)
from ._decode import decode_to_samples

_ALL_SPECIAL_CONSTRAINT_KINDS = frozenset(
    {
        SpecialConstraintKind.Indicator,
        SpecialConstraintKind.OneHot,
        SpecialConstraintKind.Sos1,
    }
)


class OpenJijPreparationSemantics(str, Enum):
    """Semantic effect of one explicit OpenJij preparation step."""

    Exact = "exact"
    Approximate = "approximate"
    FinitePenalty = "finite_penalty"


@dataclass(frozen=True, slots=True)
class OpenJijPreparationStep:
    """One auditable transformation applied before native OpenJij translation."""

    operation: str
    semantics: OpenJijPreparationSemantics
    description: str
    variable_ids: frozenset[int] = field(default_factory=frozenset)
    constraint_refs: frozenset[ConstraintRef] = field(default_factory=frozenset)


@dataclass(frozen=True, slots=True)
class OpenJijPreparationReport:
    """Compatibility checks and semantic steps for an explicit preparation."""

    source_compatibility: AdapterCompatibilityReport
    encoding_compatibility: AdapterCompatibilityReport
    steps: tuple[OpenJijPreparationStep, ...]
    final_compatibility: AdapterCompatibilityReport


@dataclass(frozen=True, slots=True)
class OpenJijPreparedModel:
    """Prepared native solver input plus the model used to evaluate samples."""

    _solver_instance: Instance = field(repr=False)
    _decoder_instance: Instance = field(repr=False)
    _evaluation_instance: Instance = field(repr=False)
    report: OpenJijPreparationReport

    @property
    def solver_instance(self) -> Instance:
        """Return an isolated copy of the Binary, unconstrained minimization input."""
        return copy.deepcopy(self._solver_instance)

    @property
    def evaluation_instance(self) -> Instance:
        """Return an isolated copy retaining the source optimization sense."""
        return copy.deepcopy(self._evaluation_instance)


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
    "OpenJijPreparationConfig",
    "OpenJijPreparationError",
    "OpenJijPreparationFailure",
    "OpenJijPreparationReport",
    "OpenJijPreparationSourceCheck",
    "OpenJijPreparationStep",
    "decode_to_samples",
]
