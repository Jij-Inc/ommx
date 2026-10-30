//! Experiment and Run lifecycle values.

use super::{
    ExperimentStatus, RunStatus, EXPERIMENT_STATUS_DRAFT, EXPERIMENT_STATUS_FAILED,
    EXPERIMENT_STATUS_FINISHED, EXPERIMENT_STATUS_INTERRUPTED, RUN_STATUS_FAILED,
    RUN_STATUS_FINISHED, RUN_STATUS_INTERRUPTED,
};
use serde::de::Error as _;
use serde::ser::SerializeStruct;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Lifecycle of an Experiment Artifact.
///
/// Failure metadata belongs only to failed and interrupted variants, so an
/// invalid combination such as a finished Experiment with a failure reason
/// cannot be represented.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ExperimentLifecycle {
    /// An uncommitted rolling checkpoint.
    Draft,
    /// A successfully committed Experiment.
    Finished,
    /// An Experiment that exited with an error and retained partial state.
    Failed {
        /// Concise caller-provided lifecycle metadata.
        reason: Option<String>,
    },
    /// An Experiment interrupted by the user that retained partial state.
    Interrupted {
        /// Concise caller-provided lifecycle metadata.
        reason: Option<String>,
    },
}

impl ExperimentLifecycle {
    /// Status discriminant used by listings and compatibility accessors.
    pub fn status(&self) -> &ExperimentStatus {
        match self {
            Self::Draft => &ExperimentStatus::Draft,
            Self::Finished => &ExperimentStatus::Finished,
            Self::Failed { .. } => &ExperimentStatus::Failed,
            Self::Interrupted { .. } => &ExperimentStatus::Interrupted,
        }
    }

    /// Concise caller-provided reason for a failed or interrupted lifecycle.
    pub fn reason(&self) -> Option<&str> {
        match self {
            Self::Failed { reason } | Self::Interrupted { reason } => reason.as_deref(),
            Self::Draft | Self::Finished => None,
        }
    }
}

/// Lifecycle of a closed Run recorded in an Experiment.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum RunLifecycle {
    /// The Run closed normally.
    Finished,
    /// The Run exited with an error and retained partial state.
    Failed {
        /// Concise caller-provided lifecycle metadata.
        reason: Option<String>,
    },
    /// The Run was interrupted and retained partial state.
    Interrupted {
        /// Concise caller-provided lifecycle metadata.
        reason: Option<String>,
    },
}

impl RunLifecycle {
    /// Status discriminant used by listings and compatibility accessors.
    pub fn status(&self) -> &RunStatus {
        match self {
            Self::Finished => &RunStatus::Finished,
            Self::Failed { .. } => &RunStatus::Failed,
            Self::Interrupted { .. } => &RunStatus::Interrupted,
        }
    }

    /// Concise caller-provided reason for a failed or interrupted lifecycle.
    pub fn reason(&self) -> Option<&str> {
        match self {
            Self::Failed { reason } | Self::Interrupted { reason } => reason.as_deref(),
            Self::Finished => None,
        }
    }
}

#[derive(Serialize, Deserialize)]
struct LifecycleOutcome {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
}

#[derive(Deserialize)]
struct ExperimentLifecycleWire {
    status: String,
    #[serde(default)]
    outcome: Option<LifecycleOutcome>,
}

#[derive(Deserialize)]
struct RunLifecycleWire {
    #[serde(default = "default_run_status")]
    status: String,
    #[serde(default)]
    outcome: Option<LifecycleOutcome>,
}

fn serialize_lifecycle<S>(
    serializer: S,
    status: &'static str,
    reason: Option<&str>,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let mut state = serializer.serialize_struct("Lifecycle", usize::from(reason.is_some()) + 1)?;
    state.serialize_field("status", status)?;
    if let Some(reason) = reason {
        state.serialize_field(
            "outcome",
            &LifecycleOutcome {
                reason: Some(reason.to_string()),
            },
        )?;
    }
    state.end()
}

impl Serialize for ExperimentLifecycle {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serialize_lifecycle(serializer, self.status().as_str(), self.reason())
    }
}

impl<'de> Deserialize<'de> for ExperimentLifecycle {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = ExperimentLifecycleWire::deserialize(deserializer)?;
        match (wire.status.as_str(), wire.outcome) {
            (EXPERIMENT_STATUS_DRAFT, None) => Ok(Self::Draft),
            (EXPERIMENT_STATUS_FINISHED, None) => Ok(Self::Finished),
            (EXPERIMENT_STATUS_FAILED, outcome) => Ok(Self::Failed {
                reason: outcome.and_then(|outcome| outcome.reason),
            }),
            (EXPERIMENT_STATUS_INTERRUPTED, outcome) => Ok(Self::Interrupted {
                reason: outcome.and_then(|outcome| outcome.reason),
            }),
            (EXPERIMENT_STATUS_DRAFT | EXPERIMENT_STATUS_FINISHED, Some(_)) => {
                Err(D::Error::custom(format_args!(
                    "Experiment status {} cannot have a lifecycle outcome",
                    wire.status
                )))
            }
            _ => Err(D::Error::custom(format_args!(
                "unknown Experiment status {}",
                wire.status
            ))),
        }
    }
}

impl Serialize for RunLifecycle {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serialize_lifecycle(serializer, self.status().as_str(), self.reason())
    }
}

impl<'de> Deserialize<'de> for RunLifecycle {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = RunLifecycleWire::deserialize(deserializer)?;
        match (wire.status.as_str(), wire.outcome) {
            (RUN_STATUS_FINISHED, None) => Ok(Self::Finished),
            (RUN_STATUS_FAILED, outcome) => Ok(Self::Failed {
                reason: outcome.and_then(|outcome| outcome.reason),
            }),
            (RUN_STATUS_INTERRUPTED, outcome) => Ok(Self::Interrupted {
                reason: outcome.and_then(|outcome| outcome.reason),
            }),
            (RUN_STATUS_FINISHED, Some(_)) => Err(D::Error::custom(
                "finished Run cannot have a lifecycle outcome",
            )),
            _ => Err(D::Error::custom(format_args!(
                "unknown Run status {}",
                wire.status
            ))),
        }
    }
}

fn default_run_status() -> String {
    RUN_STATUS_FINISHED.to_string()
}
