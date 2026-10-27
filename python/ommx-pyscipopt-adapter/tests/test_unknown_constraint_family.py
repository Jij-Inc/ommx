"""An adapter must reject future families even at its translation boundary."""

from types import SimpleNamespace
from typing import cast

import pytest
from ommx import ConstraintIDBase, Instance, OneHotConstraintID
from ommx_pyscipopt_adapter import OMMXPySCIPOptAdapter, OMMXPySCIPOptAdapterError


class FutureConstraintID(ConstraintIDBase):
    pass


@pytest.mark.parametrize("key", [FutureConstraintID(), OneHotConstraintID(1)])
def test_unknown_family_is_rejected_instead_of_omitted(key: ConstraintIDBase):
    # Exercise the consumer boundary independently of SDK applicability checks:
    # a newer SDK may recognize a family that this adapter cannot translate.
    adapter = object.__new__(OMMXPySCIPOptAdapter)
    adapter.instance = cast(Instance, SimpleNamespace(constraints=[key]))
    with pytest.raises(
        OMMXPySCIPOptAdapterError, match="Unsupported constraint family"
    ):
        adapter._set_constraints()
