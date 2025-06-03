//! Default file/directory names for the harness file structure

use std::path::Path;

// experiment source folder
pub const SRC_TEMPLATE_DIR: &str = "template";
pub const SRC_RUN_FILE: &str = "run.sh";
pub const SRC_ENV_DIR: &str = "envs";
pub const SRC_ENV_FILE: &str = "0.env";
pub const SRC_README: &str = "README";

// experiment series folder
pub const SERIES_SRC_DIR: &str = ".src";
pub const SERIES_RUNS_DIR: &str = "runs";
pub const SERIES_EXOMAT_LOG: &str = "exomat.log";
pub const SERIES_STDERR_LOG: &str = "stderr.log";
pub const SERIES_STDOUT_LOG: &str = "stdout.log";

// experiment run folder
pub const RUN_RUN_FILE: &str = "run.sh";
pub const RUN_ENV_FILE: &str = "environment.env";

// names for marker files
pub const MARKER_SRC: &str = ".exomat_source";
pub const MARKER_SRC_CP: &str = ".exomat_source_copy";
pub const MARKER_SERIES: &str = ".exomat_series";
pub const MARKER_RUN: &str = ".exomat_run";

/// Returns the last part of a path (which is the file-/directory name).
///
/// ## Panics
/// - panics if file `ends` with "." or "..".
pub fn file_name_string(file: &Path) -> String {
    file.file_name().unwrap().to_str().unwrap().to_string()
}
