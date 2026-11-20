# pyright: reportUnnecessaryTypeIgnoreComment=error

from collections import UserDict

import pytest

from ommx import Function, Polynomial


def test_polynomial_accepts_tuple_keyed_terms():
    terms: dict[tuple[int, ...], float] = {(0, 1, 2): 2.0, (): 1.0}
    assert Function(Polynomial(terms=terms)).terms == terms


def test_polynomial_accepts_user_dict():
    terms: UserDict[tuple[int, ...], float] = UserDict({(0, 0, 2): 2.0, (): 1.0})
    assert Function(Polynomial(terms=terms)).terms == terms


def test_polynomial_rejects_string_ids():
    terms: dict[tuple[str, ...], float] = {("x",): 1.0}
    with pytest.raises(TypeError):
        Polynomial(terms=terms)  # pyright: ignore[reportArgumentType]
    with pytest.raises(TypeError):
        Polynomial(terms=UserDict(terms))  # pyright: ignore[reportArgumentType]
