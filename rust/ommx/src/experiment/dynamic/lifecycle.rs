//! Opt-in drop lifecycle for runtime-owned Experiment handles.

use super::{lock_experiment_state, ExperimentDyn, ExperimentDynLifecycle, ExperimentDynState};
use std::sync::Mutex;

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
    state: &mut ExperimentDynState,
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
    use crate::experiment::Name;

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
