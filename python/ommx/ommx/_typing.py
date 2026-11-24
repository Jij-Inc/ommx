"""Private type variables used by generated SDK signatures."""

from collections.abc import Sequence
from typing import TYPE_CHECKING
from typing_extensions import TypeVar

if TYPE_CHECKING:
    from ommx import ConstraintIDBase, ConstraintView

_T = TypeVar("_T")
_MonomialKey = TypeVar("_MonomialKey", bound=Sequence[int])
_K = TypeVar("_K", bound="ConstraintIDBase")
_V_co = TypeVar("_V_co", bound="ConstraintView", covariant=True)

_C_co = TypeVar(
    "_C_co", bound="ConstraintIDBase", default="ConstraintIDBase", covariant=True
)
