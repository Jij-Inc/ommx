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
    policy = package.OMMXOpenJijSAAdapter.recommended_preparation_policy()

    assert isinstance(policy, PreparationPolicy)
    assert policy.lower_special_constraints
    assert policy.log_encode_used_integers is not None
    assert policy.as_minimization_problem
    assert policy.convert_inequality_to_equality_with_integer_slack == (
        32,
        policy.log_encode_used_integers,
    )
    assert policy.add_integer_slack_to_inequality == 32
    assert policy.uniform_penalty_method_with_weight is None
    assert policy.penalty_method_with_weights is None


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
