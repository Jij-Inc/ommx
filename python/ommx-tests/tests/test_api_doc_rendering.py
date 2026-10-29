import ast
import importlib
import json
from pathlib import Path

import pytest
from docutils import nodes
from sphinx.addnodes import desc_parameterlist, desc_sig_operator


API_DOCS_DIR = Path(__file__).resolve().parents[3] / "docs" / "api"


@pytest.mark.parametrize(
    "api_name",
    [
        "Function",
        "NamedFunction",
        "State",
        "Samples",
        "ConstraintCollection",
        "RemovedConstraintCollection",
        "EvaluatedConstraintCollection",
        "SampledConstraintCollection",
        "ConstraintFamilyView",
        "Instance.populate_state",
        "Instance.removed_constraints",
        "Instance.relax_constraint",
        "Instance.convert_active_objective",
        "IndicatorConstraint",
        "OneHotConstraint",
        "Sos1Constraint",
        "Instance.lower_special_constraints",
        "Instance.promote_sos1_big_m",
        "InstanceClass",
        "PreparationPolicy",
        "Instance.prepare",
        "FixedPenaltyPreparation.uniform_penalty_method_with_fixed_weight",
        "Instance.partial_evaluate",
    ],
)
def test_runtime_and_reference_share_canonical_documentation(api_name: str) -> None:
    import ommx

    project_root = API_DOCS_DIR.parents[1]
    source = (
        (project_root / f"python/ommx/src/docstrings/ommx/{api_name}.md")
        .read_text()
        .strip()
    )
    reference = json.loads((API_DOCS_DIR / "api_reference.json").read_text())
    class_name, _, member_name = api_name.partition(".")
    api_class = next(
        item
        for item in reference["modules"]["ommx"]["items"]
        if item["name"] == class_name
    )

    runtime_owner = getattr(ommx, class_name)
    reference_owner = api_class
    if member_name:
        runtime_owner = getattr(runtime_owner, member_name)
        reference_owner = next(
            item
            for item in api_class["methods"] + api_class["attributes"]
            if item["name"] == member_name
        )
    runtime_doc = runtime_owner.__doc__
    assert runtime_doc is not None
    assert runtime_doc.strip() == source
    assert reference_owner["doc"].strip() == source

    stub = ast.parse(
        (project_root / "python/ommx/ommx/_ommx_rust/__init__.pyi").read_text()
    )
    stub_class = next(
        node
        for node in stub.body
        if isinstance(node, ast.ClassDef) and node.name == class_name
    )
    stub_owner: ast.ClassDef | ast.FunctionDef = stub_class
    if member_name:
        stub_owner = next(
            node
            for node in stub_class.body
            if isinstance(node, ast.FunctionDef) and node.name == member_name
        )
    assert ast.get_docstring(stub_owner) == source


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
                "atol",
            )
        ],
        "return_type": None,
    }

    original_parse_myst = generated_extension._parse_myst
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
            "atol: str",
        ]
        assert [
            operator.astext() for operator in parameter_list.findall(desc_sig_operator)
        ] == ["*"]
    finally:
        setattr(generated_extension, "_parse_myst", original_parse_myst)
        setattr(generated_extension, "_build_callable_signatures", original_builder)


def test_positional_only_separator_is_restored_from_generated_stub(
    monkeypatch,
) -> None:
    monkeypatch.syspath_prepend(str(API_DOCS_DIR))
    generated_extension = importlib.import_module("pyo3_stub_gen_ext")
    ommx_extension = importlib.import_module("ommx_pyo3_stub_gen_ext")

    signature = {
        "parameters": [
            {
                "name": "other",
                "type_": {"display": "object", "link_target": None, "children": []},
                "default": None,
            }
        ],
        "return_type": None,
    }

    original_parse_myst = generated_extension._parse_myst
    original_builder = generated_extension._build_callable_signatures
    ommx_extension.setup(None)
    try:
        signature_node = generated_extension._build_callable_signatures(
            [signature],
            "__eq__",
            "ommx",
            "ommx.Bound.__eq__",
            None,
        )[0]
        parameter_list = next(signature_node.findall(desc_parameterlist))
        assert [child.astext() for child in parameter_list.children] == [
            "other: object",
            "/",
        ]
        assert [
            operator.astext() for operator in parameter_list.findall(desc_sig_operator)
        ] == ["/"]
    finally:
        setattr(generated_extension, "_parse_myst", original_parse_myst)
        setattr(generated_extension, "_build_callable_signatures", original_builder)


def test_setup_is_idempotent_across_extension_reload(monkeypatch) -> None:
    monkeypatch.syspath_prepend(str(API_DOCS_DIR))
    generated_extension = importlib.import_module("pyo3_stub_gen_ext")
    ommx_extension = importlib.import_module("ommx_pyo3_stub_gen_ext")

    original_parse_myst = generated_extension._parse_myst
    original_builder = generated_extension._build_callable_signatures
    ommx_extension.setup(None)
    first_parse_myst = generated_extension._parse_myst
    first_builder = generated_extension._build_callable_signatures

    try:
        reloaded_extension = importlib.reload(ommx_extension)
        reloaded_extension.setup(None)

        assert getattr(
            generated_extension, reloaded_extension._ORIGINAL_HOOKS_ATTRIBUTE
        ) == (original_parse_myst, original_builder)
        assert generated_extension._parse_myst is reloaded_extension._parse_myst
        assert (
            generated_extension._build_callable_signatures
            is reloaded_extension._build_callable_signatures
        )
        assert generated_extension._parse_myst is not first_parse_myst
        assert generated_extension._build_callable_signatures is not first_builder

        container = nodes.container()
        container.extend(generated_extension._parse_myst(">>> value = 1\n>>> value\n1"))
        assert [block.astext() for block in container.findall(nodes.literal_block)] == [
            ">>> value = 1\n>>> value\n1"
        ]

        signature = {
            "parameters": [
                {
                    "name": "other",
                    "type_": {
                        "display": "object",
                        "link_target": None,
                        "children": [],
                    },
                    "default": None,
                }
            ],
            "return_type": None,
        }
        signature_node = generated_extension._build_callable_signatures(
            [signature],
            "__eq__",
            "ommx",
            "ommx.Bound.__eq__",
            None,
        )[0]
        parameter_list = next(signature_node.findall(desc_parameterlist))
        assert [child.astext() for child in parameter_list.children] == [
            "other: object",
            "/",
        ]
    finally:
        setattr(generated_extension, "_parse_myst", original_parse_myst)
        setattr(generated_extension, "_build_callable_signatures", original_builder)
