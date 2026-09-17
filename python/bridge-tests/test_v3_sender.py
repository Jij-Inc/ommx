"""Required wheel integration suite for the independent Rust SDK v3 sender."""

import ommx
import ommx_pyo3_bridge_fixture as sender
import pytest
from ommx import v1


def test_v3_sender_to_real_v1_receiver():
    for factory, cls in [
        (sender.v1_function, ommx.Function),
        (sender.v1_constraint, ommx.Constraint),
        (sender.v1_decision_variable, ommx.DecisionVariable),
        (sender.negotiated_instance, ommx.Instance),
        (sender.negotiated_parametric_instance, ommx.ParametricInstance),
        (sender.negotiated_solution, ommx.Solution),
        (sender.negotiated_sample_set, ommx.SampleSet),
    ]:
        assert type(factory()) is cls is getattr(v1, cls.__name__)
    assert sender.bridge_error_type() is ommx.BridgeError
    instance = sender.compile_for_target()
    assert instance.constraint_hints.one_hot_constraints == [
        v1.OneHot(id=23, variables=[7])
    ]
    assert [row.id for row in instance.constraints] == [23]
    assert not instance.evaluate({7: 0}).feasible
    assert instance.evaluate({7: 1}).objective == -2
    assert sender.has_legacy_hint(sender.raw_v1_instance().to_bytes())


def test_v2_only_sender_fails_before_transfer():
    with pytest.raises(ommx.BridgeError, match="does not support the protobuf v2"):
        sender.function()


def test_payload_error_retains_receiver_cause():
    with pytest.raises(ommx.BridgeError) as error:
        sender.invalid_instance()
    assert isinstance(error.value.__cause__, ommx.BridgeError)
    assert error.value.__cause__.__cause__ is not None
