//! Crate-wide error type and diagnostics-emitting macros.
//!
//! [`Error`] and [`Result`] are re-exports of [`anyhow::Error`] and
//! [`anyhow::Result`]. Keeping `ommx::Error` / `ommx::Result` as the public
//! spelling lets downstream crates avoid depending on `anyhow` directly while
//! still being able to `err.downcast_ref::<T>()` / `err.is::<T>()` against
//! **signal types** like [`crate::InfeasibleDetected`],
//! [`crate::CoefficientError`], or [`crate::BoundError`].
//!
//! # Fail-site macros
//!
//! [`crate::bail!`], [`crate::error!`], and [`crate::ensure!`] bundle two
//! things that virtually
//! every OMMX failure site does together:
//!
//! 1. Emit a `tracing::error!` event (so subscribers see a structured record
//!    with the active span context).
//! 2. Produce / return an `anyhow::Error` carrying the rendered message.
//!
//! Callers write the message once — an optional `{ field = value, ... }`
//! block becomes the event's structured fields; the format string + args
//! become both the event message and the error's `Display`.
//!
//! ```ignore
//! // No structured fields — tracing still records the rendered message
//! // under the active span.
//! crate::bail!("invalid OBJSENSE: {s}");
//!
//! // With structured fields — `section` and `size` are attached to the
//! // tracing event; the error message is the rendered format string.
//! crate::bail!(
//!     { section, size },
//!     "invalid field size ({size}) in MPS section '{section}'",
//! );
//! ```
//!
//! The expression form `bail!(some_err)` still returns a pre-built error
//! without emitting tracing, since signal-type errors are typically caught
//! and recovered by the caller rather than observed as diagnostics.

pub use anyhow::{Error, Result};

/// Emit a `tracing::error!` event and short-circuit the current function with
/// an [`anyhow::Error`] built from the same format string.
///
/// See the [module docs](self) for details on the `{ fields }` form and the
/// signal-type expression form.
#[macro_export]
macro_rules! bail {
    // Structured fields + message
    ({ $($field:tt)+ } $(,)? $fmt:literal $(, $arg:expr)* $(,)?) => {{
        // Render the message once so format arguments with side effects (or
        // move-only values) are evaluated exactly once across both the
        // `tracing::error!` event and the resulting `anyhow::Error`.
        let __ommx_msg = ::std::format!($fmt $(, $arg)*);
        ::tracing::error!($($field)+, "{}", __ommx_msg);
        return ::std::result::Result::Err(::anyhow::Error::msg(__ommx_msg));
    }};
    // Plain format string (and args)
    ($fmt:literal $(, $arg:expr)* $(,)?) => {{
        let __ommx_msg = ::std::format!($fmt $(, $arg)*);
        ::tracing::error!("{}", __ommx_msg);
        return ::std::result::Result::Err(::anyhow::Error::msg(__ommx_msg));
    }};
    // Signal-style pre-built error — no tracing event, since callers
    // typically recover these by downcast rather than observe them.
    ($err:expr $(,)?) => {
        return ::std::result::Result::Err(::anyhow::Error::from($err))
    };
}

/// Emit a `tracing::error!` event and build an [`anyhow::Error`] inline
/// (for use in `.ok_or_else(|| ...)` and similar).
///
/// Mirrors [`crate::bail!`] without the `return`.
#[macro_export]
macro_rules! error {
    ({ $($field:tt)+ } $(,)? $fmt:literal $(, $arg:expr)* $(,)?) => {{
        let __ommx_msg = ::std::format!($fmt $(, $arg)*);
        ::tracing::error!($($field)+, "{}", __ommx_msg);
        ::anyhow::Error::msg(__ommx_msg)
    }};
    ($fmt:literal $(, $arg:expr)* $(,)?) => {{
        let __ommx_msg = ::std::format!($fmt $(, $arg)*);
        ::tracing::error!("{}", __ommx_msg);
        ::anyhow::Error::msg(__ommx_msg)
    }};
    ($err:expr $(,)?) => {
        ::anyhow::Error::from($err)
    };
}

/// Short-circuit if `cond` is false. Mirrors [`crate::bail!`] syntactically.
#[macro_export]
macro_rules! ensure {
    ($cond:expr, { $($field:tt)+ } $(,)? $fmt:literal $(, $arg:expr)* $(,)?) => {{
        if !$cond {
            let __ommx_msg = ::std::format!($fmt $(, $arg)*);
            ::tracing::error!($($field)+, "{}", __ommx_msg);
            return ::std::result::Result::Err(::anyhow::Error::msg(__ommx_msg));
        }
    }};
    ($cond:expr, $fmt:literal $(, $arg:expr)* $(,)?) => {{
        if !$cond {
            let __ommx_msg = ::std::format!($fmt $(, $arg)*);
            ::tracing::error!("{}", __ommx_msg);
            return ::std::result::Result::Err(::anyhow::Error::msg(__ommx_msg));
        }
    }};
    ($cond:expr, $err:expr $(,)?) => {
        if !$cond {
            return ::std::result::Result::Err(::anyhow::Error::from($err));
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, thiserror::Error)]
    #[error("signal variant for testing")]
    struct TestSignal;

    #[test]
    fn downcast_recovers_signal() {
        let e: Error = TestSignal.into();
        assert!(e.is::<TestSignal>());
        assert!(e.downcast_ref::<TestSignal>().is_some());
    }

    #[test]
    fn macro_bail_plain_message() {
        fn inner() -> Result<()> {
            crate::bail!("boom")
        }
        assert_eq!(inner().unwrap_err().to_string(), "boom");
    }

    #[test]
    fn macro_bail_formatted_message() {
        fn inner(code: u32) -> Result<()> {
            crate::bail!("boom: code={code}")
        }
        assert_eq!(inner(7).unwrap_err().to_string(), "boom: code=7");
    }

    #[test]
    fn macro_bail_with_fields() {
        fn inner(code: u32) -> Result<()> {
            crate::bail!({ code = code }, "boom: code={code}")
        }
        assert_eq!(inner(9).unwrap_err().to_string(), "boom: code=9");
    }

    #[test]
    fn macro_bail_with_signal_expression() {
        fn inner() -> Result<()> {
            crate::bail!(TestSignal)
        }
        let err = inner().unwrap_err();
        assert!(err.is::<TestSignal>());
    }

    #[test]
    fn macro_ensure_short_circuits() {
        fn inner(ok: bool) -> Result<()> {
            crate::ensure!(ok, "not ok");
            Ok(())
        }
        assert!(inner(true).is_ok());
        assert_eq!(inner(false).unwrap_err().to_string(), "not ok");
    }

    #[test]
    fn macro_ensure_with_fields() {
        fn inner(ok: bool, code: u32) -> Result<()> {
            crate::ensure!(ok, { code = code }, "not ok: code={code}");
            Ok(())
        }
        assert!(inner(true, 5).is_ok());
        assert_eq!(inner(false, 5).unwrap_err().to_string(), "not ok: code=5");
    }

    #[test]
    fn macro_ensure_with_signal_expression() {
        fn inner(ok: bool) -> Result<()> {
            crate::ensure!(ok, TestSignal);
            Ok(())
        }
        assert!(inner(true).is_ok());
        assert!(inner(false).unwrap_err().is::<TestSignal>());
    }

    #[test]
    fn macro_error_builds_inline() {
        let err: Error = crate::error!("inline message {}", 42);
        assert_eq!(err.to_string(), "inline message 42");
    }

    #[test]
    fn macro_error_with_fields() {
        let code = 7u32;
        let err: Error = crate::error!({ code }, "inline: code={code}");
        assert_eq!(err.to_string(), "inline: code=7");
    }

    #[test]
    fn macro_error_with_signal_expression() {
        let err: Error = crate::error!(TestSignal);
        assert!(err.is::<TestSignal>());
    }

    // Each of the three macros renders the format string + args into a
    // single message that is shared with both `tracing::error!` and the
    // produced `anyhow::Error`. A format argument with observable side
    // effects must be evaluated exactly once. Regression tests for this
    // guarantee follow.

    #[test]
    fn macro_bail_evaluates_args_once() {
        use std::cell::Cell;
        let counter: Cell<u32> = Cell::new(0);
        fn inner(counter: &Cell<u32>) -> Result<()> {
            crate::bail!("count={}", {
                counter.set(counter.get() + 1);
                42
            });
        }
        let err = inner(&counter).unwrap_err();
        assert_eq!(counter.get(), 1, "arg evaluated {} times", counter.get());
        assert_eq!(err.to_string(), "count=42");
    }

    #[test]
    fn macro_error_evaluates_args_once() {
        use std::cell::Cell;
        let counter: Cell<u32> = Cell::new(0);
        let err: Error = crate::error!("count={}", {
            counter.set(counter.get() + 1);
            42
        });
        assert_eq!(counter.get(), 1, "arg evaluated {} times", counter.get());
        assert_eq!(err.to_string(), "count=42");
    }

    #[test]
    fn macro_ensure_evaluates_args_once() {
        use std::cell::Cell;
        let counter: Cell<u32> = Cell::new(0);
        fn inner(counter: &Cell<u32>) -> Result<()> {
            crate::ensure!(false, "count={}", {
                counter.set(counter.get() + 1);
                42
            });
            Ok(())
        }
        let err = inner(&counter).unwrap_err();
        assert_eq!(counter.get(), 1, "arg evaluated {} times", counter.get());
        assert_eq!(err.to_string(), "count=42");
    }
}
