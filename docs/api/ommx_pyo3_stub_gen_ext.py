"""OMMX-specific rendering adaptations for pyo3-stub-gen API docs."""

import re

import pyo3_stub_gen_ext as generated_extension


_generated_parse_myst = generated_extension._parse_myst


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
    return {"version": "0.1", "parallel_read_safe": True}
