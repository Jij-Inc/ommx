import ast
import importlib
import json
import subprocess
import sys
from pathlib import Path

import pytest
from docutils import nodes
from sphinx.addnodes import desc_parameterlist, desc_sig_operator


API_DOCS_DIR = Path(__file__).resolve().parents[3] / "docs" / "api"


@pytest.mark.parametrize("missing_reference", [False, True])
@pytest.mark.parametrize("package_directive", [False, True])
def test_generated_reference_context_and_type_links(
    tmp_path, missing_reference, package_directive
) -> None:
    """Resolve real generated nodes, including the negative nitpicky build."""
    source = tmp_path / "source"
    source.mkdir()
    (source / "conf.py").write_text(
        f"import sys\nsys.path.insert(0, {str(API_DOCS_DIR)!r})\n"
        "extensions = ['myst_parser', 'pyo3_stub_gen_ext', 'ommx_pyo3_stub_gen_ext']\n"
        "nitpicky = True\nproject = 'Reference resolution regression'\n"
    )

    def method(name, doc="", target=None):
        return {
            "name": name,
            "doc": doc,
            "signatures": [{"parameters": [], "return_type": target}],
        }

    def cls(name, doc="", methods=()):
        return {"kind": "Class", "name": name, "doc": doc, "methods": list(methods)}

    def internal_type(name):
        return {
            "display": name,
            "link_target": {"fqn": f"ommx._ommx_rust.{name}", "kind": "Class"},
            "children": [],
        }

    first_doc = "Use {meth}`detach` and {mod}`ommx.artifact`."
    if missing_reference:
        first_doc += " Missing {class}`Sos1BigMFormulation`."
    package = {
        "modules": {
            "ommx": {
                "doc": "",
                "items": [
                    cls(
                        "First",
                        first_doc,
                        [
                            method("detach", "Return {class}`Second`."),
                            method("expression", target=internal_type("ToFunction")),
                            method(
                                "collector", target=internal_type("DiagnosticCollector")
                            ),
                        ],
                    ),
                    cls("Second", "Use {meth}`detach`.", [method("detach")]),
                ],
            },
            "ommx.artifact": {
                "doc": "Read {meth}`Artifact.import_archive`.",
                "items": [cls("Artifact", methods=[method("import_archive")])],
            },
        }
    }
    (source / "api_reference.json").write_text(json.dumps(package))
    index = """Reference resolution
====================

.. py:currentmodule:: unrelated

.. pyo3-api-summary:: ommx

.. pyo3-api-class:: ommx First

.. pyo3-api-class:: ommx Second

.. pyo3-api:: ommx.artifact

.. py:currentmodule:: None

.. py:class:: ommx.adapter.DiagnosticCollector

"""
    if package_directive:
        start = index.index(".. pyo3-api-summary:: ommx")
        end = index.index(".. py:currentmodule:: None")
        index = index[:start] + ".. pyo3-api-package:: ommx\n\n" + index[end:]
    # The fixture owns these public destinations; the wrapper obtains the union
    # and its actual reexports from the project's canonical stub and sources.
    for name in (
        "DecisionVariable",
        "AttachedDecisionVariable",
        "Parameter",
        "Linear",
        "Quadratic",
        "Polynomial",
        "Function",
    ):
        index += f".. py:class:: ommx.{name}\n\n"
    index += ".. py:class:: numpy.integer\n\n.. py:class:: numpy.floating\n"
    (source / "index.rst").write_text(index)
    output = tmp_path / "html"
    result = subprocess.run(
        [
            sys.executable,
            "-m",
            "sphinx",
            "-b",
            "html",
            "-W",
            "--keep-going",
            str(source),
            str(output),
        ],
        capture_output=True,
        text=True,
    )
    messages = result.stdout + result.stderr
    assert result.returncode == int(missing_reference), messages
    if missing_reference:
        assert "reference target not found: Sos1BigMFormulation" in messages
    else:
        assert "WARNING:" not in messages
    html = (output / "index.html").read_text()
    for target in (
        "ommx.First.detach",
        "ommx.Second.detach",
        "ommx.Second",
        "ommx.artifact.Artifact.import_archive",
        "module-ommx.artifact",
        "ommx.Function",
        "ommx.adapter.DiagnosticCollector",
    ):
        assert f'href="#{target}"' in html
    assert 'href="#ommx._ommx_rust.ToFunction"' not in html
    assert html.count('id="module-ommx"') == 1
    assert html.count('id="module-ommx.artifact"') == 1


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
