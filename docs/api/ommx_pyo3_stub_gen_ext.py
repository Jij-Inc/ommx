"""OMMX-specific rendering adaptations for pyo3-stub-gen API docs."""

import ast
import re
from functools import lru_cache
from pathlib import Path

import pyo3_stub_gen_ext as generated_extension
from docutils import nodes
from sphinx.addnodes import desc_parameter, desc_parameterlist, desc_sig_operator


_generated_parse_myst = generated_extension._parse_myst
_generated_build_callable_signatures = generated_extension._build_callable_signatures
_STUB_PATH = (
    Path(__file__).resolve().parents[2]
    / "python"
    / "ommx"
    / "ommx"
    / "_ommx_rust"
    / "__init__.pyi"
)


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
        if len(parameters) == len(json_names) and {
            name for name, _ in parameters
        } == json_names:
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
        kind: [rendered[name] for name, parameter_kind in parameters if parameter_kind == kind]
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
    return {"version": "0.1", "parallel_read_safe": True}
