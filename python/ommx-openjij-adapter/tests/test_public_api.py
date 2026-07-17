import inspect
from typing import get_type_hints

from ommx import DecisionVariable, Instance, Sense
import ommx_openjij_adapter as package
from ommx_openjij_adapter import _decode, _preparation, adapter


def test_package_root_is_the_stable_public_facade() -> None:
    expected_exports = [
        "OMMXOpenJijSAAdapter",
        "OpenJijPreparation",
        "OpenJijPreparationError",
        "OpenJijPreparationReport",
        "OpenJijPreparationSourceCheck",
        "OpenJijPreparationStep",
        "decode_to_samples",
    ]

    assert package.__all__ == expected_exports
    assert issubclass(package.OMMXOpenJijSAAdapter, adapter.OMMXOpenJijSAAdapter)
    assert package.OpenJijPreparation is _preparation.OpenJijPreparation
    assert package.OpenJijPreparationError is _preparation.OpenJijPreparationError
    assert package.OpenJijPreparationReport is _preparation.OpenJijPreparationReport
    assert (
        package.OpenJijPreparationSourceCheck
        is _preparation.OpenJijPreparationSourceCheck
    )
    assert package.OpenJijPreparationStep is _preparation.OpenJijPreparationStep
    assert package.decode_to_samples is _decode.decode_to_samples
    assert not hasattr(package, "response_to_samples")
    assert not hasattr(package, "sample_qubo_sa")
    assert not hasattr(_decode, "response_to_samples")

    assert package.OMMXOpenJijSAAdapter.__module__ == "ommx_openjij_adapter"


def test_public_classes_support_standard_introspection() -> None:
    public_classes = [
        package.OMMXOpenJijSAAdapter,
        package.OpenJijPreparation,
        package.OpenJijPreparationError,
        package.OpenJijPreparationReport,
        package.OpenJijPreparationSourceCheck,
        package.OpenJijPreparationStep,
    ]

    for class_ in public_classes:
        assert inspect.getsource(class_)
        get_type_hints(class_)

    assert (
        get_type_hints(package.OpenJijPreparation)["report"]
        is package.OpenJijPreparationReport
    )


def test_adapter_identity_in_applicability_report_is_unchanged() -> None:
    x = DecisionVariable.binary(0)
    instance = Instance.from_components(
        decision_variables=[x],
        objective=x,
        constraints={},
        sense=Sense.Minimize,
    )

    report = package.OMMXOpenJijSAAdapter.check_applicability(instance)

    assert report.adapter == "ommx_openjij_adapter.OMMXOpenJijSAAdapter"
