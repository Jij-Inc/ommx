//! Crate-wide error type for OMMX Rust SDK.
//!
//! All fallible public APIs return [`Result<T>`] (alias for `std::result::Result<T, Error>`).
//! The concrete [`Error`] type is a thin newtype over [`anyhow::Error`], so it carries
//! arbitrary context and source chains while hiding the `anyhow` dependency from the
//! crate's public surface.
//!
//! # Design
//!
//! Callers have two ways to consume an [`Error`]:
//!
//! 1. **Display it.** Most callers propagate with `?` and only ever look at
//!    [`Display`](std::fmt::Display) output. Structured diagnostic information
//!    (field names, IDs, breadcrumbs) is emitted via the [`tracing`] crate at
//!    each failure site, so subscribers pick it up without the `Error` having
//!    to carry it.
//!
//! 2. **Downcast to a signal type.** A small, curated set of types
//!    (e.g. [`crate::InfeasibleDetected`], [`crate::CoefficientError`],
//!    [`crate::BoundError`]) are stable "signal" types that callers can recover
//!    by downcast:
//!
//!    ```ignore
//!    match instance.propagate(&state, atol) {
//!        Err(e) if e.is::<ommx::InfeasibleDetected>() => { /* handle infeasibility */ }
//!        Err(e) => return Err(e),
//!        Ok(outcome) => { /* ... */ }
//!    }
//!    ```
//!
//! Error *enums* that were once public (`InstanceError`, `SolutionError`,
//! `ParseError`, ...) are intentionally not part of the v3 API: their
//! discriminants were never recovered in practice by downstream crates, and
//! keeping them typed imposes a maintenance tax without a matching benefit.

use std::fmt;

/// Crate-wide error type. A thin newtype over [`anyhow::Error`].
///
/// See the [module-level documentation](self) for usage guidance.
pub struct Error(anyhow::Error);

/// Convenience alias for `std::result::Result<T, ommx::Error>`.
pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    /// Construct an [`Error`] from any type implementing [`std::error::Error`].
    pub fn new<E>(source: E) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        Self(anyhow::Error::new(source))
    }

    /// Construct an [`Error`] from a plain display-able message.
    pub fn msg<M>(message: M) -> Self
    where
        M: fmt::Display + fmt::Debug + Send + Sync + 'static,
    {
        Self(anyhow::Error::msg(message))
    }

    /// Returns a reference to the inner value if it is of type `E`.
    ///
    /// Use this to recover *signal* types documented at the module level.
    pub fn downcast_ref<E>(&self) -> Option<&E>
    where
        E: fmt::Display + fmt::Debug + Send + Sync + 'static,
    {
        self.0.downcast_ref::<E>()
    }

    /// Attempts to downcast the error to a concrete type `E`.
    ///
    /// If the inner error is not of type `E`, the original [`Error`] is
    /// returned as `Err` so the caller can propagate it unchanged.
    pub fn downcast<E>(self) -> std::result::Result<E, Self>
    where
        E: fmt::Display + fmt::Debug + Send + Sync + 'static,
    {
        self.0.downcast::<E>().map_err(Self)
    }

    /// Returns `true` if the inner value is of type `E`.
    pub fn is<E>(&self) -> bool
    where
        E: fmt::Display + fmt::Debug + Send + Sync + 'static,
    {
        self.0.is::<E>()
    }

    /// Walk the chain of source errors, starting from this one.
    pub fn chain(&self) -> anyhow::Chain<'_> {
        self.0.chain()
    }

    /// Returns the deepest error in the source chain.
    pub fn root_cause(&self) -> &(dyn std::error::Error + 'static) {
        self.0.root_cause()
    }

    /// Consume the [`Error`] and return the inner [`anyhow::Error`].
    ///
    /// Provided for interop with code that still uses [`anyhow::Error`]
    /// directly (e.g. `bin/`, tests, examples).
    pub fn into_anyhow(self) -> anyhow::Error {
        self.0
    }

    /// Borrow the inner [`anyhow::Error`].
    pub fn as_anyhow(&self) -> &anyhow::Error {
        &self.0
    }

    /// Wrap an [`anyhow::Error`] into this crate's [`Error`].
    ///
    /// Use this from internal helpers that still produce `anyhow::Result`
    /// when bubbling out to a public API that returns [`Result`].
    ///
    /// Rust's coherence rules forbid providing a `From<anyhow::Error>` impl
    /// alongside the blanket `From<E: std::error::Error + Send + Sync + 'static>`
    /// (anyhow may adopt [`std::error::Error`] in a future version), so the
    /// conversion is exposed as an explicit constructor instead.
    pub fn from_anyhow(source: anyhow::Error) -> Self {
        Self(source)
    }
}

// Delegating trait impls.

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.0, f)
    }
}

// Note: we intentionally do NOT implement `std::error::Error` for `Error`,
// mirroring `anyhow::Error`. This allows the blanket `From<E: StdError>`
// below without conflicting with the reflexive `From<T> for T` identity impl.

impl<E> From<E> for Error
where
    E: std::error::Error + Send + Sync + 'static,
{
    fn from(source: E) -> Self {
        Self(anyhow::Error::new(source))
    }
}

impl AsRef<dyn std::error::Error + Send + Sync + 'static> for Error {
    fn as_ref(&self) -> &(dyn std::error::Error + Send + Sync + 'static) {
        self.0.as_ref()
    }
}

impl AsRef<dyn std::error::Error + 'static> for Error {
    fn as_ref(&self) -> &(dyn std::error::Error + 'static) {
        self.0.as_ref()
    }
}

// Forward conversion lets internal helpers that still return `anyhow::Result`
// absorb a crate-boundary [`ommx::Result`](Result) via `?`.
impl From<Error> for anyhow::Error {
    fn from(error: Error) -> Self {
        error.0
    }
}

/// Build an [`Error`] with a formatted message, analogous to [`anyhow::anyhow!`].
#[macro_export]
macro_rules! bail {
    ($msg:literal $(,)?) => {
        return ::std::result::Result::Err($crate::Error::msg($msg))
    };
    ($err:expr $(,)?) => {
        return ::std::result::Result::Err($crate::Error::from($err))
    };
    ($fmt:expr, $($arg:tt)*) => {
        return ::std::result::Result::Err($crate::Error::msg(format!($fmt, $($arg)*)))
    };
}

/// Short-circuit with an [`Error`] if `cond` is false, analogous to [`anyhow::ensure!`].
#[macro_export]
macro_rules! ensure {
    ($cond:expr, $msg:literal $(,)?) => {
        if !$cond {
            return ::std::result::Result::Err($crate::Error::msg($msg));
        }
    };
    ($cond:expr, $err:expr $(,)?) => {
        if !$cond {
            return ::std::result::Result::Err($crate::Error::from($err));
        }
    };
    ($cond:expr, $fmt:expr, $($arg:tt)*) => {
        if !$cond {
            return ::std::result::Result::Err($crate::Error::msg(format!($fmt, $($arg)*)));
        }
    };
}

/// Construct an [`Error`] inline without returning, analogous to [`anyhow::anyhow!`].
#[macro_export]
macro_rules! error {
    ($msg:literal $(,)?) => {
        $crate::Error::msg($msg)
    };
    ($err:expr $(,)?) => {
        $crate::Error::from($err)
    };
    ($fmt:expr, $($arg:tt)*) => {
        $crate::Error::msg(format!($fmt, $($arg)*))
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, thiserror::Error)]
    #[error("signal variant for testing")]
    struct TestSignal;

    #[test]
    fn wraps_error() {
        let e: Error = TestSignal.into();
        assert_eq!(e.to_string(), "signal variant for testing");
    }

    #[test]
    fn downcast_recovers_signal() {
        let e: Error = TestSignal.into();
        assert!(e.is::<TestSignal>());
        assert!(e.downcast_ref::<TestSignal>().is_some());
    }

    #[test]
    fn downcast_wrong_type_returns_self() {
        let e: Error = TestSignal.into();
        match e.downcast::<std::io::Error>() {
            Ok(_) => panic!("unexpected downcast success"),
            Err(e) => assert!(e.is::<TestSignal>()),
        }
    }

    #[test]
    fn msg_constructs_from_string() {
        let e = Error::msg("plain message");
        assert_eq!(e.to_string(), "plain message");
    }

    #[test]
    fn macro_bail_returns_err() {
        fn inner() -> Result<()> {
            crate::bail!("boom")
        }
        assert!(inner().is_err());
    }

    #[test]
    fn macro_ensure_short_circuits() {
        fn inner(ok: bool) -> Result<()> {
            crate::ensure!(ok, "not ok");
            Ok(())
        }
        assert!(inner(true).is_ok());
        assert!(inner(false).is_err());
    }
}
