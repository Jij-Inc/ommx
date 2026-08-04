from __future__ import annotations

import ommx_openjij_adapter as package
from ommx import PreparationPolicy
from ommx_openjij_adapter import _decode, adapter


def test_public_api_contains_only_adapter_and_decode_helper() -> None:
    assert package.__all__ == [
        "OMMXOpenJijSAAdapter",
        "decode_to_samples",
    ]
    assert issubclass(package.OMMXOpenJijSAAdapter, adapter.OMMXOpenJijSAAdapter)
    assert package.decode_to_samples is _decode.decode_to_samples


def test_adapter_recommends_common_preparation_policy() -> None:
    policy = package.OMMXOpenJijSAAdapter.recommended_preparation_policy(atol=1e-4)

    assert isinstance(policy, PreparationPolicy)
    assert policy.allowed_special_constraint_lowerings
    assert policy.allow_integer_log_encoding
    assert policy.allow_sense_normalization
    assert policy.uniform_penalty_weight is None
    assert policy.penalty_weights is None
    assert policy.atol == 1e-4


def test_adapter_specific_preparation_types_are_not_exported() -> None:
    for name in (
        "OpenJijPreparation",
        "OpenJijPreparationConfig",
        "OpenJijPreparationError",
        "OpenJijPreparationFailure",
        "OpenJijPreparationReport",
        "OpenJijPreparationSourceCheck",
        "OpenJijPreparationStep",
    ):
        assert not hasattr(package, name)
