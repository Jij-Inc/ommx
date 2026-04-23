"""Lazy setup of the OTel pipeline for the cell magic.

``opentelemetry-sdk`` is a hard runtime dependency of ``ommx``, so we
can import the SDK at the top level. The function below is still called
"lazy" in the architectural sense: it only installs the collector on
first use, not at ``import ommx.tracing`` time.
"""

from __future__ import annotations

import threading
from typing import Optional

from opentelemetry import trace
from opentelemetry.sdk.trace import TracerProvider as SdkTracerProvider

from ._collector import _CellSpanCollector


_COLLECTOR: Optional[_CellSpanCollector] = None
_LOCK = threading.Lock()


def ensure_collector_installed() -> _CellSpanCollector:
    """Install the cell-trace collector onto the active ``TracerProvider``.

    Behavior:

    * If the global provider already supports ``add_span_processor``
      (i.e. it's an SDK provider, or something compatible), attach a
      collector to it. Existing processors are undisturbed.
    * If the global provider does not support ``add_span_processor``
      (e.g. the default ``ProxyTracerProvider`` from a fresh notebook),
      install an SDK provider. OpenTelemetry only honours the *first*
      ``set_tracer_provider`` call, so after the attempt we re-read the
      global and fail with a helpful message if we still don't have
      something we can attach to.

    The collector instance is cached so repeated magic invocations in the
    same session reuse a single collector (no per-cell processor
    accumulation on the provider).
    """
    global _COLLECTOR
    with _LOCK:
        if _COLLECTOR is not None:
            return _COLLECTOR

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


def get_collector() -> Optional[_CellSpanCollector]:
    """Return the cached collector, or ``None`` if not yet installed."""
    return _COLLECTOR


def reset_for_testing() -> None:
    """Drop the cached collector. Only intended for unit tests."""
    global _COLLECTOR
    with _LOCK:
        _COLLECTOR = None
