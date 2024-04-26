import pytest
import ommx


def test_sum_as_string():
    assert ommx.sum_as_string(1, 1) == "2"
