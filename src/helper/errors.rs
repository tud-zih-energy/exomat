//! Custom Error and Result type definition

use thiserror::Error;

/// The return type used by the exomat library.
pub type Result<T> = core::result::Result<T, Error>;

#[derive(Error, Debug)]
pub enum Error {
    #[error(transparent)]
    IoError(#[from] std::io::Error),

    #[error(transparent)]
    RegexError(#[from] regex::Error),

    #[error(transparent)]
    LoggerError(#[from] spdlog::re_export::log::SetLoggerError),

    /// Occurs when the harness command failed to create files/directories.
    #[error("Cannot create {entry:?}: {reason:?}")]
    HarnessCreateError { entry: String, reason: String },

    /// Occurs when the harness command could not run an experiment.
    #[error("Encountered error while trying to run {experiment}: {err}")]
    HarnessRunError { experiment: String, err: String },

    #[error("Something went wrong in .env generation: {reason:?}")]
    EnvError { reason: String },

    /// Occurs when the make-table command could not generate CSV output.
    #[error("CSV conversion failed: {reason}")]
    CsvError { reason: String },

    #[error("Cannot generate autocompletion file: {err}")]
    CompletionError { err: String },

    /// error from whitin dotenvy
    #[error("Error during environment file handling: {0}")]
    DotenvyError(#[from] dotenvy::Error),

    #[error("Error trying to determine exomat-related dir: {0}")]
    FindMarkerError(String),

    /// Something was empty that shouldn't be empty
    #[error("Value missing/empty, but must be given: {0}")]
    Empty(String),
}
