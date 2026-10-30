import pytest

from ommx import DecisionVariable, Instance, Sense
from ommx.adapter import AdapterNotApplicableError
import ommx_openjij_adapter as package
from ommx_openjij_adapter import _decode, adapter


def test_package_root_is_the_stable_public_facade() -> None:
    assert package.__all__ == [
        "OMMXOpenJijSAAdapter",
        "decode_to_samples",
    ]
    assert issubclass(package.OMMXOpenJijSAAdapter, adapter.OMMXOpenJijSAAdapter)
    assert package.decode_to_samples is _decode.decode_to_samples
    assert package.OMMXOpenJijSAAdapter.__module__ == "ommx_openjij_adapter"


def test_adapter_identity_in_applicability_error_uses_public_facade() -> None:
    x = DecisionVariable.continuous(0)
    instance = Instance.from_components(
        decision_variables=[x],
        objective=x,
        constraints={},
        sense=Sense.Minimize,
    )

    report = package.OMMXOpenJijSAAdapter.check_applicability(instance)
    with pytest.raises(AdapterNotApplicableError) as error:
        package.OMMXOpenJijSAAdapter(instance)

    assert error.value.adapter == "ommx_openjij_adapter.OMMXOpenJijSAAdapter"
    assert error.value.report == report
