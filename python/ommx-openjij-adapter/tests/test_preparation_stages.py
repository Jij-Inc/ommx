from __future__ import annotations

import pytest

from ommx import DecisionVariable, Instance, Sense
from ommx_openjij_adapter._preparation_stages import (
    _AdapterInputCandidate,
    _Applied,
    _ApproximateIntegerSlack,
    _Blocked,
    _CheckedAdapterInput,
    _EncodingInput,
    _ExactIntegerSlack,
    _PenaltyReady,
    _RegularSource,
    _SourceMember,
    _SourceEncoded,
    _StageInvariantError,
)
from ommx_openjij_adapter._preparation_checks import check_preparation_source
from ommx_openjij_adapter._preparation_phases import (
    apply_penalties,
    encode_source_integers,
)
from ommx_openjij_adapter import OMMXOpenJijSAAdapter, OpenJijPreparationConfig


def test_penalty_ready_accepts_an_approximate_slack_inequality() -> None:
    x = DecisionVariable.binary(0)
    instance = Instance.from_components(
        decision_variables=[x],
        objective=x,
        constraints={7: x <= 0},
        sense=Sense.Minimize,
    )

    state = _PenaltyReady(
        instance,
        inequality_ids=frozenset({7}),
        slack_outcomes=(_ApproximateIntegerSlack(7, 0.5),),
    )

    assert state.slack_outcomes == (_ApproximateIntegerSlack(7, 0.5),)


def test_penalty_ready_requires_exactly_one_matching_slack_outcome() -> None:
    x = DecisionVariable.binary(0)
    instance = Instance.from_components(
        decision_variables=[x],
        objective=x,
        constraints={7: x <= 0},
        sense=Sense.Minimize,
    )

    with pytest.raises(_StageInvariantError, match="cover exactly"):
        _PenaltyReady(
            instance,
            inequality_ids=frozenset({7}),
            slack_outcomes=(),
        )

    wrong_exact_result = Instance.from_components(
        decision_variables=[x],
        objective=x,
        constraints={7: x <= 0},
        sense=Sense.Minimize,
    )
    with pytest.raises(_StageInvariantError, match="active equality"):
        _PenaltyReady(
            wrong_exact_result,
            inequality_ids=frozenset({7}),
            slack_outcomes=(_ExactIntegerSlack(7),),
        )


def test_penalty_ready_rejects_an_untracked_active_inequality() -> None:
    x = DecisionVariable.binary(0)
    instance = Instance.from_components(
        decision_variables=[x],
        objective=x,
        constraints={7: x <= 0},
        sense=Sense.Minimize,
    )

    with pytest.raises(_StageInvariantError, match="explicitly approximated"):
        _PenaltyReady(
            instance,
            inequality_ids=frozenset(),
            slack_outcomes=(),
        )


@pytest.mark.parametrize("residual_step", [0.0, -1.0, float("inf"), float("nan")])
def test_approximate_slack_requires_a_positive_finite_residual(
    residual_step: float,
) -> None:
    with pytest.raises(_StageInvariantError, match="finite and positive"):
        _ApproximateIntegerSlack(7, residual_step)


def test_source_encoded_requires_source_integer_ids_to_be_replaced() -> None:
    x = DecisionVariable.integer(3, lower=0, upper=2)
    instance = Instance.from_components(
        decision_variables=[x],
        objective=x,
        constraints={},
        sense=Sense.Minimize,
    )

    with pytest.raises(_StageInvariantError, match="must not remain used"):
        _SourceEncoded(instance, source_integer_ids=frozenset({3}))


def test_stage_transition_consumes_the_previous_owner() -> None:
    binary = DecisionVariable.binary(0)
    instance = Instance.from_components(
        decision_variables=[binary],
        objective=binary,
        constraints={},
        sense=Sense.Minimize,
    )
    source = _RegularSource(instance)

    outcome = encode_source_integers(source, frozenset())

    assert isinstance(outcome, _Applied)
    with pytest.raises(_StageInvariantError, match="already been consumed"):
        _ = source.instance


def test_applicability_check_transfers_candidate_ownership_once() -> None:
    binary = DecisionVariable.binary(0)
    instance = Instance.from_components(
        decision_variables=[binary],
        objective=binary,
        constraints={},
        sense=Sense.Minimize,
    )
    candidate = _AdapterInputCandidate(instance)

    checked = _CheckedAdapterInput.check(
        candidate,
        OMMXOpenJijSAAdapter.check_applicability,
    )

    assert checked.applicability.is_applicable
    with pytest.raises(_StageInvariantError, match="already been consumed"):
        _ = candidate.instance
    assert checked.take_instance() is instance
    with pytest.raises(_StageInvariantError, match="already been consumed"):
        checked.take_instance()


def test_policy_rejection_still_consumes_the_penalty_ready_stage() -> None:
    binary = DecisionVariable.binary(0)
    source_instance = Instance.from_components(
        decision_variables=[binary],
        objective=binary,
        constraints={7: binary == 0},
        sense=Sense.Minimize,
    )
    state = _PenaltyReady(
        Instance.from_components(
            decision_variables=[binary],
            objective=binary,
            constraints={7: binary == 0},
            sense=Sense.Minimize,
        ),
        inequality_ids=frozenset(),
        slack_outcomes=(),
    )

    outcome = apply_penalties(state, source_instance, OpenJijPreparationConfig())

    assert isinstance(outcome, _Blocked)
    with pytest.raises(_StageInvariantError, match="already been consumed"):
        _ = state.instance


def test_source_membership_evidence_must_match_the_owned_instance() -> None:
    binary = DecisionVariable.binary(0)
    accepted = Instance.from_components(
        decision_variables=[binary],
        objective=binary,
        constraints={},
        sense=Sense.Minimize,
    )
    continuous = DecisionVariable.continuous(0)
    rejected = Instance.from_components(
        decision_variables=[continuous],
        objective=continuous,
        constraints={},
        sense=Sense.Minimize,
    )

    with pytest.raises(_StageInvariantError, match="must describe"):
        _SourceMember(rejected, check_preparation_source(accepted))


def test_encoding_and_adapter_input_stages_enforce_their_shapes() -> None:
    binary = DecisionVariable.binary(0)
    constrained = Instance.from_components(
        decision_variables=[binary],
        objective=binary,
        constraints={7: binary == 0},
        sense=Sense.Minimize,
    )
    with pytest.raises(_StageInvariantError, match="no active regular"):
        _EncodingInput(constrained)

    integer = DecisionVariable.integer(0, lower=0, upper=2)
    integer_input = Instance.from_components(
        decision_variables=[integer],
        objective=integer,
        constraints={},
        sense=Sense.Minimize,
    )
    _EncodingInput(integer_input)
    with pytest.raises(_StageInvariantError, match="only Binary"):
        _AdapterInputCandidate(integer_input)
