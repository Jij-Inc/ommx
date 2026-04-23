"""Lazy setup of the OTel pipeline for the cell magic.

The cell magic must work in notebooks that have *not* configured OTel at
all, as well as in notebooks where the user has already set up a
``TracerProvider``. This module hides that distinction behind
:func:`ensure_collector_installed`.
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

    * If the global provider is already an ``sdk.trace.TracerProvider``,
      attach a collector to it (non-destructive — existing processors keep
      receiving spans).
    * Otherwise install a new SDK provider as the global and attach the
      collector to it. This mirrors the escape hatch documented in
      :mod:`ommx.tracing` for notebooks that have not configured OTel.

    The collector instance is cached so repeated magic invocations in the
    same session reuse a single collector.
    """
    global _COLLECTOR
    with _LOCK:
        if _COLLECTOR is not None:
            return _COLLECTOR

        provider = trace.get_tracer_provider()
        if not isinstance(provider, SdkTracerProvider):
            # The default ``ProxyTracerProvider`` from the OTel API package
            # has no ``add_span_processor``. Install a bare SDK provider so
            # Rust spans flowing through ``pyo3-tracing-opentelemetry`` have
            # somewhere to land.
            provider = SdkTracerProvider()
            trace.set_tracer_provider(provider)

        collector = _CellSpanCollector()
        # ``add_span_processor`` wraps the processor directly; no exporter
        # is required because we read finished spans out of the collector.
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
