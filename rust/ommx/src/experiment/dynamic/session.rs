//! Lifecycle-safe scopes for dynamic Experiment and Run handles.

use super::{ExperimentDyn, RunDyn};
use crate::artifact::{LocalArtifactDyn, LocalRegistryHandle};
use crate::experiment::Name;
use anyhow::Result;

const EXPERIMENT_SCOPE_DROP_REASON: &str = "Experiment scope dropped before lifecycle completion";

/// Lifecycle owner for an [`ExperimentDyn`].
///
/// Explicit [`commit`](Self::commit), [`fail`](Self::fail), or
/// [`interrupt`](Self::interrupt) consumes the session and suppresses any
/// fallback transition. Dropping an unresolved session does nothing by
/// default. Call [`interrupt_on_drop`](Self::interrupt_on_drop) to opt into a
/// best-effort interrupted checkpoint; failures are reported through tracing
/// because [`Drop`] cannot return them.
///
/// `ExperimentDyn` is a shared runtime handle. Completing this session changes
/// the lifecycle observed by every clone of the same handle.
#[derive(Debug)]
pub struct ExperimentSession {
    experiment: ExperimentDyn,
    interrupted_reason_on_drop: Option<String>,
    resolved: bool,
}

impl ExperimentSession {
    fn new(experiment: ExperimentDyn) -> Self {
        Self {
            experiment,
            interrupted_reason_on_drop: None,
            resolved: false,
        }
    }

    /// Opt into a best-effort interrupted checkpoint if this session is
    /// dropped before an explicit lifecycle transition.
    pub fn interrupt_on_drop(mut self, reason: impl Into<String>) -> Self {
        self.interrupted_reason_on_drop = Some(reason.into());
        self
    }

    /// Access the lifecycle-owned Experiment handle.
    pub fn experiment(&self) -> &ExperimentDyn {
        &self.experiment
    }

    /// Commit the Experiment successfully.
    pub fn commit(mut self) -> Result<LocalArtifactDyn> {
        self.resolved = true;
        self.experiment.commit()
    }

    /// Publish a failed checkpoint while preserving closed Runs.
    pub fn fail(mut self, reason: impl Into<String>) -> Result<()> {
        self.resolved = true;
        self.experiment.commit_failed_checkpoint(reason)
    }

    /// Publish an interrupted checkpoint while preserving closed Runs.
    pub fn interrupt(mut self, reason: impl Into<String>) -> Result<()> {
        self.resolved = true;
        self.experiment.commit_interrupted_checkpoint(reason)
    }
}

impl Drop for ExperimentSession {
    fn drop(&mut self) {
        if self.resolved || !self.experiment.is_unsealed() {
            return;
        }
        let Some(reason) = self.interrupted_reason_on_drop.take() else {
            return;
        };
        self.resolved = true;
        if let Err(error) = self.experiment.commit_interrupted_checkpoint(reason) {
            tracing::warn!(
                error = %error,
                "Failed to publish interrupted Experiment checkpoint during session drop"
            );
        }
    }
}

/// Lifecycle owner for a [`RunDyn`].
///
/// Explicit [`finish`](Self::finish), [`fail`](Self::fail), or
/// [`interrupt`](Self::interrupt) consumes the session and appends the Run's
/// partial state to its parent Experiment. Dropping an unresolved session
/// abandons the Run by default. Call [`interrupt_on_drop`](Self::interrupt_on_drop)
/// to opt into best-effort interrupted finalization; failures are reported
/// through tracing because [`Drop`] cannot return them.
#[derive(Debug)]
pub struct RunSession {
    run: Option<RunDyn>,
    interrupt_on_drop: bool,
}

impl RunSession {
    fn new(run: RunDyn) -> Self {
        Self {
            run: Some(run),
            interrupt_on_drop: false,
        }
    }

    /// Opt into best-effort interrupted finalization if this session is
    /// dropped before an explicit lifecycle transition.
    pub fn interrupt_on_drop(mut self) -> Self {
        self.interrupt_on_drop = true;
        self
    }

    /// Access the open Run handle.
    pub fn run(&self) -> &RunDyn {
        self.run
            .as_ref()
            .expect("an unresolved RunSession always owns its RunDyn")
    }

    /// Mutably access the open Run handle.
    pub fn run_mut(&mut self) -> &mut RunDyn {
        self.run
            .as_mut()
            .expect("an unresolved RunSession always owns its RunDyn")
    }

    /// Finish the Run successfully.
    pub fn finish(mut self) -> Result<()> {
        self.run
            .take()
            .expect("an unresolved RunSession always owns its RunDyn")
            .finish()
    }

    /// Finish the Run as failed while preserving its partial state.
    pub fn fail(mut self) -> Result<()> {
        self.run
            .take()
            .expect("an unresolved RunSession always owns its RunDyn")
            .finish_failed()
    }

    /// Finish the Run as interrupted while preserving its partial state.
    pub fn interrupt(mut self) -> Result<()> {
        self.run
            .take()
            .expect("an unresolved RunSession always owns its RunDyn")
            .finish_interrupted()
    }
}

impl Drop for RunSession {
    fn drop(&mut self) {
        let Some(run) = self.run.take() else {
            return;
        };
        if !self.interrupt_on_drop {
            run.abandon();
            return;
        }
        if let Err(error) = run.finish_interrupted() {
            tracing::warn!(
                error = %error,
                "Failed to finish interrupted Run during session drop"
            );
        }
    }
}

impl ExperimentDyn {
    /// Wrap this dynamic Experiment in an explicit lifecycle session.
    ///
    /// The returned session does not perform a fallback transition on drop
    /// unless the caller opts in with [`ExperimentSession::interrupt_on_drop`].
    pub fn session(&self) -> ExperimentSession {
        ExperimentSession::new(self.clone())
    }

    /// Create and run a lifecycle-safe Experiment in the default Local Registry.
    ///
    /// A successful callback commits the Experiment. A returned error publishes
    /// a best-effort failed checkpoint and returns the original callback error.
    /// A panic or unresolved drop publishes a best-effort interrupted checkpoint.
    /// This method only writes to the Local Registry; it never pushes remotely.
    pub fn scoped(
        name: impl Into<Name>,
        f: impl FnOnce(&ExperimentDyn) -> Result<()>,
    ) -> Result<LocalArtifactDyn> {
        let experiment = Self::new(name)?;
        Self::run_scope(experiment, f)
    }

    /// Create and run a lifecycle-safe Experiment with an explicit Local Registry.
    ///
    /// This has the same lifecycle behavior as [`Self::scoped`] while keeping
    /// the caller-provided registry owner alive through the dynamic handle.
    pub fn scoped_with_registry_handle(
        registry_handle: LocalRegistryHandle,
        name: impl Into<Name>,
        f: impl FnOnce(&ExperimentDyn) -> Result<()>,
    ) -> Result<LocalArtifactDyn> {
        let experiment = Self::with_registry_handle(registry_handle, name)?;
        Self::run_scope(experiment, f)
    }

    fn run_scope(
        experiment: ExperimentDyn,
        f: impl FnOnce(&ExperimentDyn) -> Result<()>,
    ) -> Result<LocalArtifactDyn> {
        let session = experiment
            .session()
            .interrupt_on_drop(EXPERIMENT_SCOPE_DROP_REASON);
        match f(session.experiment()) {
            Ok(()) => session.commit(),
            Err(error) => {
                let reason = format!("{error:#}");
                if let Err(checkpoint_error) = session.fail(reason) {
                    tracing::warn!(
                        error = %checkpoint_error,
                        "Failed to publish failed Experiment checkpoint after callback error"
                    );
                }
                Err(error)
            }
        }
    }

    /// Run one lifecycle-safe callback against a new Run.
    ///
    /// A successful callback finishes the Run. A returned error finishes it as
    /// failed and returns the original callback error. A panic or unresolved
    /// drop finishes it as interrupted. Partial parameters and attachments are
    /// preserved in all failed and interrupted paths that can reach the parent
    /// Experiment.
    pub fn scoped_run<T>(&self, f: impl FnOnce(&mut RunDyn) -> Result<T>) -> Result<T> {
        let mut session = self.run()?.into_session().interrupt_on_drop();
        match f(session.run_mut()) {
            Ok(value) => {
                session.finish()?;
                Ok(value)
            }
            Err(error) => {
                if let Err(finish_error) = session.fail() {
                    tracing::warn!(
                        error = %finish_error,
                        "Failed to finish failed Run after callback error"
                    );
                }
                Err(error)
            }
        }
    }
}

impl RunDyn {
    /// Convert this Run into an explicit lifecycle session.
    ///
    /// The returned session abandons the Run on drop unless the caller opts in
    /// with [`RunSession::interrupt_on_drop`].
    pub fn into_session(self) -> RunSession {
        RunSession::new(self)
    }
}
