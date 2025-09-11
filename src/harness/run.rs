//! harness run subcommand

use log::{error, info, trace, warn};
use std::{
    fs::OpenOptions,
    io::Read,
    path::Path,
    process::{Command, Stdio},
};
use strip_ansi::strip_ansi;

use super::env::Environment;
use crate::helper::errors::{Error, Result};
use crate::helper::fs_names::*;

/// Executes [RUN_RUN_FILE] script found in `run_folder`.
///
/// Appends any stderr/stdout output into their respective log file in the
/// parent series directory of `run_folder`.
///
/// Exomat output will **not** automatically be duplicated to the log file
/// by calling this function.
///
/// ## Errors and Panics
/// - Returns a `HarnessRunrror` if [RUN_RUN_FILE] could not be executed
/// - panics if there is no [RUN_RUN_FILE] in `run_folder`
/// - panics if there is no [RUN_ENV_FILE] in `run_folder`
pub fn run_experiment(exp_name: &str, run_folder: &Path) -> Result<()> {
    assert!(
        run_folder.join(RUN_RUN_FILE).is_file(),
        "Missing run.sh in experiment run directory"
    );

    assert!(
        run_folder.join(RUN_ENV_FILE).is_file(),
        "Missing environment.env in experiment run directory"
    );

    // this file also contains internal variables, which will be treated as normal
    // variables from now on
    let envs = Environment::from_file_with_load(&run_folder.join(RUN_ENV_FILE))?;

    let out_log = OpenOptions::new()
        .append(true)
        .open(run_folder.parent().unwrap().join(SERIES_STDOUT_LOG))?;

    let err_log = OpenOptions::new()
        .append(true)
        .open(run_folder.parent().unwrap().join(SERIES_STDERR_LOG))?;

    trace!(
        "{exp_name}: Starting execution of {}",
        run_folder.file_stem().unwrap().to_str().unwrap()
    );

    let run_folder_absolute = &run_folder.canonicalize().unwrap();

    // execute command with envs and collect any output in child
    let run = Command::new(run_folder_absolute.join(RUN_RUN_FILE))
        .stderr(Stdio::from(err_log))
        .stdout(Stdio::from(out_log))
        .envs(envs.to_env_map())
        .current_dir(run_folder_absolute)
        .output()
        .map_err(|e| Error::HarnessRunError {
            experiment: exp_name.to_string(),
            err: e.to_string(),
        })?;

    trace!("{exp_name}: Finished run {}", run_folder.display());

    // open file again, but in read-only mode
    log_run_result(
        run_folder.file_stem().unwrap().to_str().unwrap(),
        run.status,
        &mut OpenOptions::new()
            .read(true)
            .open(run_folder.parent().unwrap().join(SERIES_STDERR_LOG))?,
    )
}

/// Produce log output based on exit_status and err_log content.
///
/// - exit_status:
///    - **success**  : log info
///    - **failed**   : log error (don't evaluate err_log after)
/// - err_log:
///    - **empty**    : log info
///    - **not empty**: log warning
///
/// ## Errors
/// - Returns a HarnessRunError if `exit_status` shows a failure
fn log_run_result(
    run_name: &str,
    exit_status: std::process::ExitStatus,
    err_log: &mut std::fs::File,
) -> Result<()> {
    // read stderr
    let mut stderr = String::new();
    err_log.read_to_string(&mut stderr)?;

    if exit_status.success() {
        info!("{run_name} finished successfully with {exit_status}");

        if stderr.is_empty() {
            info!("{run_name} did not produce stderr output");
        } else {
            warn!("{run_name} produced stderr output");
        }
    } else {
        error!("{run_name} finished with non-zero {exit_status}");

        // fail fast in case of unsuccessful run
        return Err(Error::HarnessRunError {
            experiment: run_name.to_string(),
            err: String::from(strip_ansi(&stderr).trim()),
        });
    }

    Ok(())
}

/// Creates a ready-to-print String based on the given parameters.
///
/// ## Example
/// Given the values:
/// - `exp_name = Foo`
/// - `run = Ok(_)`
/// - `stdout = "normal output"`
/// - `stderr = ""`
/// - `exomat = "[info] ..."`
///
/// ```bash
/// [Foo] exomat:
/// [info] ...
/// ---
/// [Foo] stdout:
/// normal output
/// ---
/// [Foo] stderr:
///
/// ---
/// [Foo] returned:
/// Successful
/// ```
///
/// An extra "\n" will be added to `stdout`, `stderr` and `exomat`.
///
/// If `run = Err(e)`, the last lines will be:
/// ```bash
/// [Foo] returned:
/// Failed (reason: e)
/// ```
pub fn create_report<T>(
    exp_name: &str,
    run: &Result<T>,
    stdout: &str,
    stderr: &str,
    exomat: &str,
) -> String {
    let mut eval_str = String::new();

    // append exomat
    eval_str.push_str(&format!("[{exp_name}] exomat:\n"));
    eval_str.push_str(exomat);
    eval_str.push_str("\n---\n");

    // append stdout
    eval_str.push_str(&format!("[{exp_name}] stdout:\n"));
    eval_str.push_str(stdout);
    eval_str.push_str("\n---\n");

    // append stderr
    eval_str.push_str(&format!("[{exp_name}] stderr:\n"));
    eval_str.push_str(stderr);
    eval_str.push_str("\n---\n");

    // append overall success/failure report
    eval_str.push_str(&format!("[{exp_name}] returned:\n"));
    match run {
        Ok(_) => eval_str.push_str("Successful\n"),
        Err(e) => eval_str.push_str(&format!("Failed (reason: {e})\n")),
    }

    eval_str
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use rusty_fork::rusty_fork_test;
    use tempfile::TempDir;

    use super::super::skeleton::{
        build_run_directory, build_series_directory, create_source_directory,
    };
    use super::*;

    rusty_fork_test! {
        #[test]
        fn test_run() {
            // create base tempdir, to act as parent
            let tmpdir = TempDir::new().unwrap();
            let series_dir_handle = TempDir::new().unwrap();

            // create experiment source and series dir
            let exp_source = TempDir::new_in(tmpdir.path()).unwrap();
            let exp_source = exp_source.path().to_path_buf();
            std::env::set_current_dir(&exp_source).unwrap();
            create_source_directory(&exp_source).unwrap();

            // write something in run.sh
            let mut runsh = OpenOptions::new()
                .append(true)
                .open(exp_source.join(SRC_TEMPLATE_DIR).join(SRC_RUN_FILE))
                .unwrap();

            writeln!(runsh, "echo Hello!").unwrap();

            let series = series_dir_handle.path();
            build_series_directory(&exp_source, series).unwrap();
            let default_env = series
                .join(SERIES_SRC_DIR)
                .join(SRC_ENV_DIR)
                .join(SRC_ENV_FILE);

            // create run dir and run experiment
            let run = build_run_directory(series, &default_env, 1, 1).unwrap();
            run_experiment(&file_name_string(&exp_source), &run).unwrap();

            let out_log = std::fs::read_to_string(series.join(SERIES_RUNS_DIR).join(SERIES_STDOUT_LOG)).unwrap();
            let err_log = std::fs::read_to_string(series.join(SERIES_RUNS_DIR).join(SERIES_STDERR_LOG)).unwrap();

            assert!(out_log.contains("Hello!"));
            assert!(err_log.is_empty());
        }
    }
}
