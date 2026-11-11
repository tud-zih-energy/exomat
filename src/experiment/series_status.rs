use super::experiment_run::RunStatus;
use derive_more::{Add, Display, Sum};

/// Aggregated counts over multiple `RunStatus` values
#[derive(Clone, Debug, Default, PartialEq, Add, Sum, Display)]
#[display(
    "{} not started/{} not finished/{} failed/{} success",
    not_started,
    not_finished,
    success,
    failed
)]
pub struct SeriesStatus {
    pub not_started: usize,
    pub not_finished: usize,
    pub success: usize,
    pub failed: usize,
}

impl SeriesStatus {
    pub fn total(&self) -> usize {
        self.not_started + self.not_finished + self.success + self.failed
    }
}

impl From<RunStatus> for SeriesStatus {
    fn from(status: RunStatus) -> Self {
        match status {
            RunStatus::NotStarted => Self {
                not_started: 1,
                ..Self::default()
            },
            RunStatus::StartedButNotFinished => Self {
                not_finished: 1,
                ..Self::default()
            },
            RunStatus::Success => Self {
                success: 1,
                ..Self::default()
            },
            RunStatus::ExitNonZero(_) => Self {
                failed: 1,
                ..Self::default()
            },
        }
    }
}
