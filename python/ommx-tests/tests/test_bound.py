from ommx.v1 import Bound


def test_bound_eq():
    b1 = Bound(1.0, 2.0)
    b2 = Bound(1.0, 2.0)
    assert b1 == b2, "Bounds with same values should be equal"
