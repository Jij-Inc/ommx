//! Lifecycle-safe scopes for dynamic Experiment and Run handles.

use super::super::logging::AttachmentLoggerStorage;
use super::super::{AttachmentTable, AutosavePolicy, Name};
use super::{lock_experiment_state, ExperimentDyn, ExperimentDynLifecycle, RunDyn};
use crate::artifact::local_registry::LocalRegistry;
use crate::artifact::{ImageRef, LocalArtifactDyn, LocalRegistryHandle};
use anyhow::Result;
use oci_spec::image::Descriptor;
use std::sync::Mutex;

const EXPERIMENT_SCOPE_DROP_REASON: &str = "Experiment scope dropped before lifecycle completion";

/// Restricted Experiment view used by [`ExperimentDyn::scoped`].
///
/// This is a capability view over an [`ExperimentDyn`], not another Experiment
/// state. It exposes Experiment-space logging and configuration together with
/// [`scoped_run`](Self::scoped_run), but deliberately does not expose
/// [`ExperimentDyn::run`]. Every Run created through this view is therefore
/// finalized before the callback can return or unwind, so a terminal
/// Experiment checkpoint cannot omit an escaped open Run.
///
/// Attempting to move a raw Run outside the callback is rejected at compile
/// time:
///
/// ```compile_fail
/// use ommx::experiment::{ExperimentDyn, Name};
///
/// let mut escaped_run = None;
/// let _ = ExperimentDyn::scoped(Name::Anonymous, |scope| {
///     escaped_run = Some(scope.run()?);
///     anyhow::bail!("stop")
/// });
/// ```
///
/// The mutable Run reference passed to `scoped_run` cannot escape either:
///
/// ```compile_fail
/// use ommx::experiment::{ExperimentDyn, Name};
///
/// let mut escaped_run = None;
/// let _ = ExperimentDyn::scoped(Name::Anonymous, |scope| {
///     scope.scoped_run(|run| -> anyhow::Result<()> {
///         escaped_run = Some(run);
///         anyhow::bail!("stop")
///     })?;
///     Ok(())
/// });
/// ```
#[derive(Debug)]
pub struct ExperimentScope<'exp> {
    experiment: &'exp ExperimentDyn,
}

impl ExperimentScope<'_> {
    /// Concrete Local Registry image name for this Experiment.
    pub fn image_name(&self) -> Result<ImageRef> {
        self.experiment.image_name()
    }

    /// Set a manifest annotation on this unsealed Experiment.
    pub fn set_annotation(&self, key: impl Into<String>, value: impl Into<String>) -> Result<()> {
        self.experiment.set_annotation(key, value)
    }

    /// Pending manifest annotations for this unsealed Experiment.
    pub fn annotations(&self) -> Result<std::collections::HashMap<String, String>> {
        self.experiment.annotations()
    }

    /// Set the rolling draft checkpoint policy for closed Runs.
    pub fn set_autosave_policy(&self, policy: AutosavePolicy) -> Result<()> {
        self.experiment.set_autosave_policy(policy)
    }

    /// Rename the Local Registry image ref used by the final Experiment.
    pub fn rename(&self, image_name: ImageRef) -> Result<()> {
        self.experiment.rename(image_name)
    }

    /// Run one lifecycle-safe callback against a new Run.
    ///
    /// A successful callback finishes the Run. A returned error finishes it as
    /// failed and returns the original callback error. A panic or unresolved
    /// drop finishes it as interrupted.
    pub fn scoped_run<T>(&self, f: impl FnOnce(&mut RunDyn) -> Result<T>) -> Result<T> {
        self.experiment.scoped_run(f)
    }
}

impl AttachmentLoggerStorage for &ExperimentScope<'_> {
    type Descriptor = Descriptor;

    fn with_local_registry<R>(&self, f: impl FnOnce(&LocalRegistry) -> Result<R>) -> Result<R> {
        let experiment = self.experiment;
        AttachmentLoggerStorage::with_local_registry(&experiment, f)
    }

    fn with_attachment_table<R>(
        &mut self,
        f: impl FnOnce(&mut AttachmentTable<Self::Descriptor>) -> Result<R>,
    ) -> Result<R> {
        let mut experiment = self.experiment;
        AttachmentLoggerStorage::with_attachment_table(&mut experiment, f)
    }

    fn descriptor_for_attachment_table(&self, descriptor: Descriptor) -> Result<Self::Descriptor> {
        let experiment = self.experiment;
        AttachmentLoggerStorage::descriptor_for_attachment_table(&experiment, descriptor)
    }
}

impl ExperimentDyn {
    /// Opt into a best-effort interrupted checkpoint when this handle is
    /// dropped while the shared Experiment is still unsealed.
    ///
    /// Ordinary `ExperimentDyn` handles do nothing on drop. The configured
    /// behavior belongs only to this handle and is not inherited by clones.
    /// If Runs are still open, checkpoint publication is deferred until the
    /// final Run is closed or abandoned, and no new Run can be opened while
    /// that terminal checkpoint is pending.
    /// Explicit commit or checkpoint methods suppress the fallback even when
    /// the explicit operation returns an error. Drop failures are reported
    /// through tracing because [`Drop`] cannot return them.
    pub fn interrupt_on_drop(mut self, reason: impl Into<String>) -> Self {
        self.interrupted_reason_on_drop = Mutex::new(Some(reason.into()));
        self
    }

    pub(super) fn suppress_interrupted_on_drop(&self) {
        let mut reason = self
            .interrupted_reason_on_drop
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        reason.take();
    }

    /// Create and run a lifecycle-safe Experiment in the default Local Registry.
    ///
    /// A successful callback commits the Experiment. A returned error publishes
    /// a best-effort failed checkpoint and returns the original callback error.
    /// A panic or unresolved drop publishes a best-effort interrupted checkpoint.
    /// This method only writes to the Local Registry; it never pushes remotely.
    pub fn scoped(
        name: impl Into<Name>,
        f: impl FnOnce(&ExperimentScope<'_>) -> Result<()>,
    ) -> Result<LocalArtifactDyn> {
        let experiment = Self::new(name)?.interrupt_on_drop(EXPERIMENT_SCOPE_DROP_REASON);
        Self::run_scope(experiment, f)
    }

    /// Create and run a lifecycle-safe Experiment with an explicit Local Registry.
    ///
    /// This has the same lifecycle behavior as [`Self::scoped`] while keeping
    /// the caller-provided registry owner alive through the dynamic handle.
    pub fn scoped_with_registry_handle(
        registry_handle: LocalRegistryHandle,
        name: impl Into<Name>,
        f: impl FnOnce(&ExperimentScope<'_>) -> Result<()>,
    ) -> Result<LocalArtifactDyn> {
        let experiment = Self::with_registry_handle(registry_handle, name)?
            .interrupt_on_drop(EXPERIMENT_SCOPE_DROP_REASON);
        Self::run_scope(experiment, f)
    }

    fn run_scope(
        experiment: ExperimentDyn,
        f: impl FnOnce(&ExperimentScope<'_>) -> Result<()>,
    ) -> Result<LocalArtifactDyn> {
        let scope = ExperimentScope {
            experiment: &experiment,
        };
        match f(&scope) {
            Ok(()) => experiment.commit(),
            Err(error) => {
                let reason = format!("{error:#}");
                if let Err(checkpoint_error) = experiment.commit_failed_checkpoint(reason) {
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
        let mut run = self.run()?.interrupt_on_drop();
        match f(&mut run) {
            Ok(value) => {
                run.finish()?;
                Ok(value)
            }
            Err(error) => {
                if let Err(finish_error) = run.finish_failed() {
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

impl Drop for ExperimentDyn {
    fn drop(&mut self) {
        let reason = self
            .interrupted_reason_on_drop
            .get_mut()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take();
        let Some(reason) = reason else {
            return;
        };
        let reason_to_publish = {
            let mut state = lock_experiment_state(&self.state);
            select_interrupted_checkpoint_reason(&mut state, reason)
        };
        if let Some(reason) = reason_to_publish {
            if let Err(error) = self.commit_interrupted_checkpoint(reason) {
                tracing::warn!(
                    error = %error,
                    "Failed to publish interrupted Experiment checkpoint during drop"
                );
            }
        }
    }
}

fn select_interrupted_checkpoint_reason(
    state: &mut super::ExperimentDynState,
    reason: String,
) -> Option<String> {
    let open_run_count = match &state.lifecycle {
        ExperimentDynLifecycle::Unsealed { open_runs, .. } => *open_runs,
        _ => return None,
    };
    let selected_reason = state
        .pending_interrupted_checkpoint
        .get_or_insert(reason)
        .clone();
    (open_run_count == 0).then_some(selected_reason)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_open_run_publication_uses_first_pending_reason() {
        let experiment = ExperimentDyn::new(Name::Anonymous).unwrap();
        let selected_reason = {
            let mut state = lock_experiment_state(&experiment.state);
            state.pending_interrupted_checkpoint = Some("first terminal reason".to_owned());
            select_interrupted_checkpoint_reason(&mut state, "second terminal reason".to_owned())
        };

        assert_eq!(selected_reason.as_deref(), Some("first terminal reason"));
    }
}
