"""Notebook display helpers for OMMX objects."""

from __future__ import annotations

from dataclasses import dataclass
from html import escape


@dataclass(frozen=True)
class FunctionDisplay:
    """Context-aware display payload for a formatted OMMX function."""

    text: str
    total_terms: int
    written_terms: int
    omitted_terms: int
    truncated_by_chars: bool

    @property
    def truncated(self) -> bool:
        return self.omitted_terms > 0 or self.truncated_by_chars

    def __str__(self) -> str:
        return self.text

    def __repr__(self) -> str:
        return self.text

    def _repr_html_(self) -> str:
        body = f"<pre><code>{escape(self.text)}</code></pre>"
        if not self.truncated:
            return body

        notes: list[str] = []
        if self.omitted_terms:
            notes.append(
                f"showing {self.written_terms} of {self.total_terms} terms; "
                f"{self.omitted_terms} omitted"
            )
        if self.truncated_by_chars:
            notes.append("truncated by character limit")
        note = escape("; ".join(notes))
        return f'{body}<div class="ommx-function-truncation">{note}</div>'
