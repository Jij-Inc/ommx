"""Private type variables used by generated SDK overloads."""

from typing import TYPE_CHECKING, TypeVar

if TYPE_CHECKING:
    from ommx import ConstraintIDBase, ConstraintView

_T = TypeVar("_T")
_K = TypeVar("_K", bound="ConstraintIDBase")
_V_co = TypeVar("_V_co", bound="ConstraintView", covariant=True)
