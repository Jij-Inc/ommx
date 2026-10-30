"""OMMX-specific rendering adaptations for pyo3-stub-gen API docs."""

import ast
import re
from contextlib import contextmanager
from functools import lru_cache, wraps
from pathlib import Path

import pyo3_stub_gen_ext as generated_extension
from docutils import nodes
from sphinx.addnodes import (
    desc_parameter,
    desc_parameterlist,
    desc_sig_operator,
    pending_xref,
)


_ORIGINAL_HOOKS_ATTRIBUTE = "_ommx_pyo3_stub_gen_ext_original_hooks"
try:
    _generated_parse_myst, _generated_build_callable_signatures = getattr(
        generated_extension, _ORIGINAL_HOOKS_ATTRIBUTE
    )
except AttributeError:
    _generated_parse_myst = generated_extension._parse_myst
    _generated_build_callable_signatures = (
        generated_extension._build_callable_signatures
    )
    setattr(
        generated_extension,
        _ORIGINAL_HOOKS_ATTRIBUTE,
        (_generated_parse_myst, _generated_build_callable_signatures),
    )
_STUB_PATH = (
    Path(__file__).resolve().parents[2]
    / "python"
    / "ommx"
    / "ommx"
    / "_ommx_rust"
    / "__init__.pyi"
)

_REFERENCE_HOOKS_ATTRIBUTE = "_ommx_pyo3_stub_gen_ext_reference_hooks"
_ITEM_BUILDERS = (
    "_build_class",
    "_build_function",
    "_build_type_alias",
    "_build_variable",
)
_DIRECTIVES = (
    "Pyo3APIDirective",
    "Pyo3APIPackageDirective",
    "Pyo3APISummaryDirective",
)
if not hasattr(generated_extension, _REFERENCE_HOOKS_ATTRIBUTE):
    setattr(
        generated_extension,
        _REFERENCE_HOOKS_ATTRIBUTE,
        {
            **{name: getattr(generated_extension, name) for name in _ITEM_BUILDERS},
            **{name: getattr(generated_extension, name).run for name in _DIRECTIVES},
            "_build_type_expr": generated_extension._build_type_expr,
            "_register_module": generated_extension._register_module,
        },
    )
_reference_hooks = getattr(generated_extension, _REFERENCE_HOOKS_ATTRIBUTE)


@contextmanager
def _reference_context(env, module_name, class_name=None):
    saved = dict(env.ref_context)
    env.ref_context["py:module"] = module_name
    env.ref_context["py:class"] = class_name
    try:
        yield
    finally:
        env.ref_context.clear()
        env.ref_context.update(saved)


def _with_reference_context(rendered, module_name, class_name=None):
    """Supply the owner context normally set by Python-domain directives."""
    for root in rendered:
        for reference in root.findall(pending_xref):
            if reference.get("refdomain") != "py":
                continue
            if not reference.get("py:module"):
                reference["py:module"] = module_name
            if class_name and not reference.get("py:class"):
                reference["py:class"] = class_name
    return rendered


def _item_builder_with_context(name):
    original = _reference_hooks[name]

    @wraps(original)
    def build(env, item, module_name):
        class_name = item["name"] if name == "_build_class" else None
        with _reference_context(env, module_name, class_name):
            return _with_reference_context(
                original(env, item, module_name), module_name, class_name
            )

    return build


def _register_module(env, module_name, doc_module):
    _reference_hooks["_register_module"](env, module_name, doc_module)
    domain = env.get_domain("py")
    node_id = f"module-{module_name}"
    domain.note_module(
        module_name,
        node_id,
        generated_extension._extract_first_line_doc(doc_module.get("doc", "")),
        "",
        False,
    )
    # note_module populates the module index, but py:mod resolves via objects.
    domain.note_object(module_name, "module", node_id, location=env.docname)
    # A package directive renders multiple module docstrings in one run.
    # Its enclosing context manager restores the caller's context afterwards.
    env.ref_context["py:module"] = module_name
    env.ref_context["py:class"] = None


def _module_directive_with_context(name):
    original = _reference_hooks[name]

    @wraps(original)
    def run(directive):
        module_name = directive.arguments[0]
        with _reference_context(directive.env, module_name):
            rendered = _with_reference_context(original(directive), module_name)
        ids = {
            node_id
            for root in rendered
            for node in root.findall(nodes.Element)
            for node_id in node.get("ids", [])
        }
        for registered_name, module in directive.env.get_domain("py").modules.items():
            if registered_name != module_name and not (
                name == "Pyo3APIPackageDirective"
                and registered_name.startswith(f"{module_name}.")
            ):
                continue
            if module.docname == directive.env.docname and module.node_id not in ids:
                rendered.insert(0, nodes.target(ids=[module.node_id]))
                ids.add(module.node_id)
        return rendered

    return run


@lru_cache(maxsize=1)
def _binding_reexports():
    """Read actual public aliases without importing the compiled extension."""
    package = _STUB_PATH.parent.parent
    exports = {}
    for module, source in (
        ("ommx", package / "__init__.py"),
        ("ommx.artifact", package / "artifact/__init__.py"),
        ("ommx.experiment", package / "experiment/__init__.py"),
        ("ommx.adapter", package / "adapter.py"),
    ):
        tree = ast.parse(source.read_text(encoding="utf-8"))
        for node in tree.body:
            if isinstance(node, ast.ImportFrom) and node.module == "ommx._ommx_rust":
                for alias in node.names:
                    exports[f"ommx._ommx_rust.{alias.name}"] = (
                        f"{module}.{alias.asname or alias.name}"
                    )
    return exports


@lru_cache(maxsize=1)
def _stub_type_aliases():
    tree = ast.parse(_STUB_PATH.read_text(encoding="utf-8"))
    return {
        f"ommx._ommx_rust.{node.target.id}": node.value
        for node in tree.body
        if isinstance(node, ast.AnnAssign)
        and isinstance(node.target, ast.Name)
        and isinstance(node.annotation, ast.Name)
        and node.annotation.id == "TypeAlias"
    }


def _alias_type_expression(expression):
    """Expand a stub-only alias while retaining links to its constituent types."""
    result = {"display": ast.unparse(expression), "link_target": None, "children": []}
    if isinstance(expression, ast.BinOp) and isinstance(expression.op, ast.BitOr):
        result["children"] = [
            _alias_type_expression(expression.left),
            _alias_type_expression(expression.right),
        ]
    elif isinstance(expression, ast.Name):
        result["link_target"] = {
            "fqn": f"ommx._ommx_rust.{expression.id}",
            "kind": "Class",
        }
    return result


def _build_type_expr(type_expr):
    target = type_expr.get("link_target")
    if target:
        public_name = _binding_reexports().get(target["fqn"])
        if public_name:
            type_expr = {**type_expr, "link_target": {**target, "fqn": public_name}}
        elif target["fqn"] in _stub_type_aliases():
            type_expr = _alias_type_expression(_stub_type_aliases()[target["fqn"]])
    return _reference_hooks["_build_type_expr"](type_expr)


def _callable_parameters(node: ast.FunctionDef | ast.AsyncFunctionDef):
    """Return the Python parameter order and kinds for one stub signature."""
    parameters = [
        *((argument.arg, "positional_only") for argument in node.args.posonlyargs),
        *((argument.arg, "positional_or_keyword") for argument in node.args.args),
    ]
    if parameters and parameters[0][0] in {"self", "cls"}:
        parameters.pop(0)
    if node.args.vararg is not None:
        parameters.append((node.args.vararg.arg, "var_positional"))
    parameters.extend(
        (argument.arg, "keyword_only") for argument in node.args.kwonlyargs
    )
    if node.args.kwarg is not None:
        parameters.append((node.args.kwarg.arg, "var_keyword"))
    return tuple(parameters)


@lru_cache(maxsize=1)
def _stub_callable_parameters():
    """Index callable signatures from the generated stub by qualified name."""
    tree = ast.parse(_STUB_PATH.read_text(encoding="utf-8"), filename=str(_STUB_PATH))
    callables = {}

    def visit(body, prefix=()):
        for node in body:
            if isinstance(node, ast.ClassDef):
                visit(node.body, (*prefix, node.name))
            elif isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)):
                qualified_name = ".".join((*prefix, node.name))
                callables.setdefault(qualified_name, []).append(
                    _callable_parameters(node)
                )

    visit(tree.body)
    return callables


def _find_stub_parameters(fullname, signature):
    """Match a JSON signature to its generated-stub parameter kinds."""
    callables = _stub_callable_parameters()
    candidates = [
        (qualified_name, signatures)
        for qualified_name, signatures in callables.items()
        if fullname == qualified_name or fullname.endswith(f".{qualified_name}")
    ]
    if not candidates:
        return None

    _, signatures = max(candidates, key=lambda item: len(item[0]))
    json_names = {parameter["name"] for parameter in signature["parameters"]}
    for parameters in signatures:
        if (
            len(parameters) == len(json_names)
            and {name for name, _ in parameters} == json_names
        ):
            return parameters
    return None


def _operator(text):
    operator = desc_sig_operator()
    operator += nodes.Text(text)
    return operator


def _delimiter(text):
    parameter = desc_parameter()
    parameter += _operator(text)
    return parameter


def _restore_parameter_kinds(signature_node, signature, parameters):
    """Restore delimiters omitted from pyo3-stub-gen's documentation IR."""
    parameter_list = next(signature_node.findall(desc_parameterlist), None)
    if parameter_list is None:
        return

    rendered = dict(
        zip(
            (parameter["name"] for parameter in signature["parameters"]),
            parameter_list.children,
            strict=True,
        )
    )
    if rendered.keys() != {name for name, _ in parameters}:
        return

    for child in list(parameter_list.children):
        parameter_list.remove(child)

    grouped = {
        kind: [
            rendered[name]
            for name, parameter_kind in parameters
            if parameter_kind == kind
        ]
        for kind in (
            "positional_only",
            "positional_or_keyword",
            "var_positional",
            "keyword_only",
            "var_keyword",
        )
    }

    parameter_list.extend(grouped["positional_only"])
    if grouped["positional_only"]:
        parameter_list += _delimiter("/")
    parameter_list.extend(grouped["positional_or_keyword"])
    if grouped["var_positional"]:
        grouped["var_positional"][0].insert(0, _operator("*"))
        parameter_list.extend(grouped["var_positional"])
    elif grouped["keyword_only"]:
        parameter_list += _delimiter("*")
    parameter_list.extend(grouped["keyword_only"])
    if grouped["var_keyword"]:
        grouped["var_keyword"][0].insert(0, _operator("**"))
        parameter_list.extend(grouped["var_keyword"])


def _build_callable_signatures(signatures, name, module_name, fullname, env):
    signature_nodes = _generated_build_callable_signatures(
        signatures, name, module_name, fullname, env
    )
    for signature, signature_node in zip(signatures, signature_nodes, strict=True):
        parameters = _find_stub_parameters(fullname, signature)
        if parameters is not None:
            _restore_parameter_kinds(signature_node, signature, parameters)
    return signature_nodes


def _fence_bare_doctest_blocks(markdown_text: str) -> str:
    """Fence bare Python doctests before passing docstrings to MyST.

    Runtime docstrings intentionally keep native ``>>>`` / ``...`` prompts so
    the standard-library doctest parser can execute them. CommonMark instead
    reads a leading ``>`` as a block quote, so the generated renderer needs a
    ``pycon`` fence at the presentation boundary.

    Existing Markdown fences are copied verbatim. A blank line ends a bare
    doctest block, matching the standard doctest layout used by the bindings.
    """
    lines = markdown_text.splitlines()
    converted: list[str] = []
    open_fence: str | None = None
    index = 0

    while index < len(lines):
        line = lines[index]
        fence_match = re.match(r"^[ \t]*(`{3,}|~{3,})(.*)$", line)
        if open_fence is not None:
            converted.append(line)
            if fence_match is not None:
                marker, suffix = fence_match.groups()
                if (
                    marker[0] == open_fence[0]
                    and len(marker) >= len(open_fence)
                    and not suffix.strip()
                ):
                    open_fence = None
            index += 1
            continue

        if fence_match is not None:
            marker, _ = fence_match.groups()
            open_fence = marker
            converted.append(line)
            index += 1
            continue

        prompt_match = re.match(r"^([ \t]*)>>>(?: |$)", line)
        if prompt_match is None:
            converted.append(line)
            index += 1
            continue

        end = index + 1
        while end < len(lines) and lines[end].strip():
            end += 1
        block = lines[index:end]
        longest_backtick_run = max(
            (len(run) for item in block for run in re.findall(r"`+", item)),
            default=0,
        )
        fence = "`" * max(3, longest_backtick_run + 1)
        indentation = prompt_match.group(1)
        converted.append(f"{indentation}{fence}pycon")
        converted.extend(block)
        converted.append(f"{indentation}{fence}")
        index = end

    return "\n".join(converted)


def _parse_myst(markdown_text: str, env=None):
    return _generated_parse_myst(_fence_bare_doctest_blocks(markdown_text), env)


def setup(app):
    generated_extension._parse_myst = _parse_myst
    generated_extension._build_callable_signatures = _build_callable_signatures
    generated_extension._build_type_expr = _build_type_expr
    generated_extension._register_module = _register_module
    for name in _ITEM_BUILDERS:
        setattr(generated_extension, name, _item_builder_with_context(name))
    for name in _DIRECTIVES:
        getattr(generated_extension, name).run = _module_directive_with_context(name)
    return {"version": "0.1", "env_version": 1, "parallel_read_safe": True}
