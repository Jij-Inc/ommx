from ommx.v1 import DecisionVariable


def test_serialize_decision_variable():
    base = DecisionVariable.binary(1, name="x", description="x for test")
    new = DecisionVariable.from_bytes(base.to_bytes())
    assert base.equals_to(new)
