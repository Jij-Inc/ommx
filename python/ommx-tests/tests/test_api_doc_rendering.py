import importlib
from pathlib import Path

from docutils import nodes
from sphinx.addnodes import desc_parameterlist, desc_sig_operator


API_DOCS_DIR = Path(__file__).resolve().parents[3] / "docs" / "api"


def test_bare_doctest_is_fenced_without_rewriting_existing_myst(
    monkeypatch,
) -> None:
    monkeypatch.syspath_prepend(str(API_DOCS_DIR))
    generated_extension = importlib.import_module("pyo3_stub_gen_ext")
    ommx_extension = importlib.import_module("ommx_pyo3_stub_gen_ext")

    doc = """Intro with an inline `>>>` token.

```{note}
The existing directive keeps `>>>` as prose.
```

```python
>>> already_fenced = True
```

>>> value = 1
>>> value
1

>>> raise RuntimeError("boom")
Traceback (most recent call last):
    ...
RuntimeError: boom

Tail prose.
"""

    converted = ommx_extension._fence_bare_doctest_blocks(doc)

    assert "```{note}\nThe existing directive keeps `>>>` as prose.\n```" in converted
    assert "```python\n>>> already_fenced = True\n```" in converted
    assert "```pycon\n>>> value = 1\n>>> value\n1\n```" in converted
    assert (
        '```pycon\n>>> raise RuntimeError("boom")\n'
        "Traceback (most recent call last):\n    ...\nRuntimeError: boom\n```"
        in converted
    )

    original_parse_myst = generated_extension._parse_myst
    original_builder = generated_extension._build_callable_signatures
    ommx_extension.setup(None)
    try:
        container = nodes.container()
        container.extend(generated_extension._parse_myst(doc))
        literal_blocks = list(container.findall(nodes.literal_block))
        assert [block.astext() for block in literal_blocks] == [
            ">>> already_fenced = True",
            ">>> value = 1\n>>> value\n1",
            '>>> raise RuntimeError("boom")\n'
            "Traceback (most recent call last):\n    ...\nRuntimeError: boom",
        ]
        assert not list(container.findall(nodes.block_quote))
        assert "Intro with an inline >>> token." in container.astext()
        assert "Tail prose." in container.astext()
    finally:
        setattr(generated_extension, "_parse_myst", original_parse_myst)
        setattr(generated_extension, "_build_callable_signatures", original_builder)


def test_keyword_only_separator_is_restored_from_generated_stub(
    monkeypatch,
) -> None:
    monkeypatch.syspath_prepend(str(API_DOCS_DIR))
    generated_extension = importlib.import_module("pyo3_stub_gen_ext")
    ommx_extension = importlib.import_module("ommx_pyo3_stub_gen_ext")

    type_expr = {"display": "str", "link_target": None, "children": []}
    signature = {
        "parameters": [
            {"name": name, "type_": type_expr, "default": None}
            for name in (
                "name",
                "lower",
                "upper",
                "subscripts",
                "parameters",
                "description",
            )
        ],
        "return_type": None,
    }

    original_builder = generated_extension._build_callable_signatures
    ommx_extension.setup(None)
    try:
        signature_node = generated_extension._build_callable_signatures(
            [signature],
            "new_integer",
            "ommx",
            "ommx.Instance.new_integer",
            None,
        )[0]
        parameter_list = next(signature_node.findall(desc_parameterlist))
        assert [child.astext() for child in parameter_list.children] == [
            "name: str",
            "*",
            "lower: str",
            "upper: str",
            "subscripts: str",
            "parameters: str",
            "description: str",
        ]
        assert [
            operator.astext() for operator in parameter_list.findall(desc_sig_operator)
        ] == ["*"]
    finally:
        setattr(generated_extension, "_build_callable_signatures", original_builder)
