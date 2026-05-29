"""Decorator API built on top of :class:`capture_trace`."""

from __future__ import annotations

import functools
import inspect
from pathlib import Path
from typing import Any, Callable, Optional, Union, overload

from ._capture import capture_trace
from ._render import save_chrome_trace
from ._result import TraceResult


_F = Callable[..., Any]


@overload
def traced(func: _F) -> _F: ...


@overload
def traced(
    *,
    name: Optional[str] = ...,
    output: Optional[Union[str, Path]] = ...,
) -> Callable[[_F], _F]: ...


def traced(
    func: Optional[_F] = None,
    *,
    name: Optional[str] = None,
    output: Optional[Union[str, Path]] = None,
) -> Any:
    """Decorator that runs the wrapped function under :class:`capture_trace`.

    Supports all three call shapes::

        @traced
        def process(): ...

        @traced()
        def process(): ...

        @traced(name="build_qubo", output="qubo.json")
        def process(): ...

    If ``output`` is given, the Chrome Trace JSON is written to that
    path when the function returns **or raises** — information is
    never dropped. The exception, if any, is re-raised unchanged after
    the file is written.

    If ``name`` is omitted, the span is named after the function
    (``fn.__qualname__``) so traces from multiple decorated functions
    are easy to tell apart in the rendered tree.
    """

    def _save_if_configured(result: Optional[TraceResult]) -> None:
        if output is not None and result is not None:
            save_chrome_trace(result, output)

    def _save_best_effort(result: Optional[TraceResult]) -> None:
        """Like ``_save_if_configured`` but swallows any I/O failure.

        Used on the exception path: a save failure here would *replace*
        the user's original exception, which is the signal they care
        about most. Silently dropping the save is the lesser evil.
        """
        if output is None or result is None:
            return
        try:
            save_chrome_trace(result, output)
        except Exception:  # noqa: BLE001 - intentional swallow
            pass

    def _decorator(fn: _F) -> _F:
        span_name = name if name is not None else fn.__qualname__

        if inspect.iscoroutinefunction(fn):
            # ``async def`` needs its own wrapper: a plain sync wrapper
            # would trace only the coroutine-object creation, finish
            # the ``capture_trace`` block, and return the still-
            # unawaited coroutine — by the time it runs, the capture
            # window is closed and every span is silently dropped.
            @functools.wraps(fn)
            async def _async_wrapper(*args, **kwargs):
                capture = capture_trace(span_name)
                result: Optional[TraceResult] = None
                retval: Any = None
                try:
                    with capture as r:
                        result = r
                        retval = await fn(*args, **kwargs)
                except BaseException:
                    _save_best_effort(result)
                    raise
                else:
                    _save_if_configured(result)
                    return retval

            return _async_wrapper  # type: ignore[return-value]

        @functools.wraps(fn)
        def _wrapper(*args, **kwargs):
            capture = capture_trace(span_name)
            result: Optional[TraceResult] = None
            retval: Any = None
            try:
                with capture as r:
                    result = r
                    retval = fn(*args, **kwargs)
            except BaseException:
                _save_best_effort(result)
                raise
            else:
                _save_if_configured(result)
                return retval

        return _wrapper

    if func is not None:
        return _decorator(func)
    return _decorator
