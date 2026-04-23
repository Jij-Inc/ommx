"""Lazy setup of the OTel pipeline for the cell magic.

The cell magic must work in notebooks that have *not* configured OTel at
all, as well as in notebooks where the user has already set up a
``TracerProvider``. This module hides that distinction behind
:func:`ensure_collector_installed`.

The OpenTelemetry SDK is an optional dependency (``ommx[tracing]``);
we defer the import so that ``import ommx`` stays cheap for users who
never touch the tracing magic, and so the failure message points at the
right pip install.
"""

from __future__ import annotations

import threading
from typing import TYPE_CHECKING, Optional


if TYPE_CHECKING:  # pragma: no cover - type hints only
    from ._collector import _CellSpanCollector


_COLLECTOR: Optional["_CellSpanCollector"] = None
_LOCK = threading.Lock()


_OTEL_SDK_MISSING_MESSAGE = (
    "ommx.tracing requires opentelemetry-sdk. Install it with "
    "`pip install ommx[tracing]` (or `pip install opentelemetry-sdk`) "
    "and reload the extension."
)


def _import_sdk():
    """Import the OTel SDK classes we need, with a friendly error.

    Returning the module objects keeps :func:`ensure_collector_installed`
    short — no need to thread ``try``/``except`` through it.
    """
    try:
        from opentelemetry.sdk.trace import TracerProvider as SdkTracerProvider
    except ImportError as exc:  # pragma: no cover - exercised manually
        raise RuntimeError(_OTEL_SDK_MISSING_MESSAGE) from exc
    from opentelemetry import trace

    from ._collector import _CellSpanCollector

    return trace, SdkTracerProvider, _CellSpanCollector


def ensure_collector_installed() -> "_CellSpanCollector":
    """Install the cell-trace collector onto the active ``TracerProvider``.

    Behavior:

    * If the global provider already supports ``add_span_processor``
      (i.e. it's an SDK provider, or something compatible), attach a
      collector to it. Existing processors are undisturbed.
    * If the global provider does not support ``add_span_processor``
      (e.g. the default ``ProxyTracerProvider`` from a fresh notebook),
      try to install an SDK provider. OpenTelemetry only honours the
      *first* ``set_tracer_provider`` call, so after the attempt we
      re-read the global and fail with a helpful message if we still
      don't have something we can attach to.

    The collector instance is cached so repeated magic invocations in the
    same session reuse a single collector (no per-cell processor
    accumulation on the provider).
    """
    global _COLLECTOR
    with _LOCK:
        if _COLLECTOR is not None:
            return _COLLECTOR

        trace, SdkTracerProvider, _CellSpanCollector = _import_sdk()

        provider = trace.get_tracer_provider()
        if not hasattr(provider, "add_span_processor"):
            # Typically the default ``ProxyTracerProvider`` — we have a
            # licence to install a real one. If the user installed a
            # custom non-SDK provider, ``set_tracer_provider`` below is
            # silently ignored and we'll detect that on the re-read.
            trace.set_tracer_provider(SdkTracerProvider())
            provider = trace.get_tracer_provider()
            if not hasattr(provider, "add_span_processor"):
                raise RuntimeError(
                    "ommx.tracing could not install an SDK TracerProvider: "
                    f"a {type(provider).__name__!s} is already active and "
                    "OpenTelemetry refuses to replace the global "
                    "TracerProvider once set. Install "
                    "``opentelemetry.sdk.trace.TracerProvider`` yourself "
                    "before loading this extension, or clear the existing "
                    "provider."
                )

        collector = _CellSpanCollector()
        provider.add_span_processor(collector)
        _COLLECTOR = collector
        return collector


def get_collector() -> Optional["_CellSpanCollector"]:
    """Return the cached collector, or ``None`` if not yet installed."""
    return _COLLECTOR


def reset_for_testing() -> None:
    """Drop the cached collector. Only intended for unit tests."""
    global _COLLECTOR
    with _LOCK:
        _COLLECTOR = None
