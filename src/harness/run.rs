//! harness run subcommand

use chrono::Local;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use log::{info, trace};
use std::path::PathBuf;

use crate::experiment::{ExperimentSeries, ExperimentSource, FileReader, FileWriter, Runner};
use crate::helper::errors::Result;

/// Creates an experiment series/run directory for the given `experiment`.
/// Then executes the `run.sh` file for this experiment and dumps the output in
/// the log files.
///
/// The new experiment series directory will either be called `[experiment]-YYYY-MM-DD-HH-MM-SS`
/// or whatever is defined in `output`.
///
/// Requires a directory called `[experiment]` to be present in the current location.
///
/// Wrapper around `build_series_directory` and `execute_exp_repetitions`.
pub fn experiment(
    experiment: &ExperimentSource,
    output: PathBuf,
    log_progress_handler: MultiProgress,
    is_trial: bool,
) -> Result<()> {
    let mut series = ExperimentSeries::from_source(&experiment)?;
    series.persist(&output)?;

    execute_exp_repetitions(&mut series, log_progress_handler, is_trial)
}

/// Creates an experiment series/run directory for the given `experiment`.
/// Then executes the `run.sh` file for this experiment once and collects any
/// output/errors/results.
///
/// The new experiment series directory will be created as a tempdir.
pub fn trial(experiment: &ExperimentSource, log_progress_handler: MultiProgress) -> Result<()> {
    let format = &Local::now()
        .format("exomat_trial-%Y-%m-%d-%H-%M-%S")
        .to_string();
    let trial_dir_path = std::env::temp_dir().join(format);
    let trial = experiment.to_trial_source()?;

    crate::disable_console_log();

    // run experiment once
    let res = self::experiment(&trial, trial_dir_path.clone(), log_progress_handler, true);

    // flush exomat log
    spdlog::default_logger().flush();

    // gather results
    let reader = ExperimentSeries::parse(&trial_dir_path)?;
    assert!(reader.is_valid_trial());
    reader.print_report(&res);

    res
}

/// Runs the experiment defined in `exp_source_dir` `repetitions` times for each
/// environment.
///
/// This will create a new experiment run folder inside `exp_series_dir`.
///
/// This functions assumes that `build_series_directory` has been called before it.
/// Otherwise it will fail, because the files it expects to be there are not.
fn execute_exp_repetitions(
    series: &mut ExperimentSeries,
    log_progress_handler: MultiProgress,
    is_trial: bool,
) -> Result<()> {
    // if series
    //     Error::HarnessRunError {
    //         experiment: exp_source_dir.display().to_string(),
    //         err: format!(
    //             "No environments found in {}",
    //             exp_source_dir.join(SRC_ENV_DIR).display()
    //         ),
    //     }
    // })?;

    let prog_bar = if is_trial {
        ProgressBar::new(1)
    } else {
        ProgressBar::new(series.repetition_count() as u64)
    };

    prog_bar.set_style(
        ProgressStyle::with_template("[{elapsed_precise}] [{bar:.green}] {pos}/{len} ({eta})")
            .unwrap()
            .progress_chars("#>-"),
    );

    // protect progress bar from log interferance
    let prog_bar = log_progress_handler.add(prog_bar);
    prog_bar.tick(); // show on 0th repetition

    info!("Starting experiment runs for {}", series.experiment_name());
    trace!("exomat envs are: {:?}", series.exomat_envs());

    series.generate_runs()?;
    for mut run in series.iter() {
        trace!("Using envs: {:?}", run.environment());
        run.execute(&series.experiment_name())?;

        // update progress
        prog_bar.inc(1);

        // stop after one run if this is a trial
        if is_trial {
            break;
        }
    }

    prog_bar.finish();
    Ok(())
}

#[cfg(test)]
mod tests {
    use rusty_fork::rusty_fork_test;
    use std::collections::HashMap;
    use std::path::PathBuf;
    use tempfile::TempDir;

    use super::*;
    use crate::experiment::{ExperimentSource, FileWriter};
    use crate::harness::env::{Environment, ExomatEnvironment};
    use crate::helper::fs_names::*;
    use crate::helper::test_helper::read_log;

    rusty_fork_test! {
        #[test]
        fn test_run() {
            // create base tempdir, to act as parent
            let tmpdir = TempDir::new().unwrap();
            let tmpdir = tmpdir.path().to_path_buf();
            std::env::set_current_dir(&tmpdir).unwrap();
            let exp_source = &tmpdir.join("TestSource");
            let exp_series = &tmpdir.join("TestSeries");

            // write something in run.sh
            let mut src = ExperimentSource::new();
            src.set_run_script(format!("#!/bin/bash\necho $EXP_SRC_DIR\necho $EXP_SRC_DIR >> out_file"));
            src.set_exomat_envs(ExomatEnvironment::new(&exp_source, 1));
            src.persist(&exp_source).unwrap();

            let mut ser = ExperimentSeries::from_source(&src).unwrap();
            ser.generate_runs().unwrap();
            ser.persist(&exp_series).unwrap();

            // run experiment
            assert_eq!(ser.get_runs().len(), 1);
            ser.execute(&src.name()).unwrap();

            let out_log = read_log(exp_series.to_path_buf(), SERIES_STDOUT_LOG);
            let err_log = read_log(exp_series.to_path_buf(), SERIES_STDERR_LOG);

            assert!(out_log.contains(&exp_source.canonicalize().unwrap().display().to_string()));
            assert!(err_log.is_empty());
        }

        #[test]
        fn harness_run_e2e() {
            // create ouput dir
            let tmpdir = TempDir::new().unwrap();
            let tmpdir = tmpdir.path().to_path_buf();
            std::env::set_current_dir(&tmpdir).unwrap();
            let exp_name = "SomeExperiment";
            let out_name = "ExpOutput";

            // build basic experiment
            // Write something to run.sh that uses env var
            // make multiple .env files that set $FOO to different values
            let mut src = ExperimentSource::new();
            src.set_run_script(format!("#!/bin/bash\necho $FOO\necho $FOO >> out_file"));
            src.set_envs(HashMap::from([
                (PathBuf::from(TEST_RUN_REP_DIR0), Environment::from_env_list(vec![("FOO".to_string(), "BAR".to_string())])),
                (PathBuf::from(TEST_RUN_REP_DIR1), Environment::from_env_list(vec![("FOO".to_string(), "Z".to_string())])),
            ]));

            src.persist(&tmpdir.join(exp_name)).unwrap();

            // run experiment and check logs
            experiment(
                &src,
                PathBuf::from(out_name),
                MultiProgress::new(), // empty
                false
            )
            .unwrap();

            let stderr_log = read_log(tmpdir.join(out_name), SERIES_STDERR_LOG);
            assert_eq!(stderr_log.lines().count(), 0);

            // two lines for variable (inserted here), two lines for current time (part of run.sh template)
            let stdout_log = read_log(tmpdir.join(out_name), SERIES_STDOUT_LOG);
            assert_eq!(dbg!(stdout_log.lines()).count(), 4);
            assert!(stdout_log.contains("Z"));
            assert!(stdout_log.contains("BAR"));

            // take one out_file and check its content
            let output = read_log(tmpdir.join(out_name), format!("{TEST_RUN_REP_DIR0}/out_file").as_str());
            assert_eq!(output.lines().count(), 1);
            assert!(output.contains("BAR"));
        }

        #[test]
        fn trial_e2e() {
            // create ouput dir
            let tmpdir = TempDir::new().unwrap();
            let tmpdir = tmpdir.path().to_path_buf();
            std::env::set_current_dir(&tmpdir).unwrap();

            // build basic experiment
            // Write something to run.sh that uses env var
            // make multiple .env files that set $FOO to different values
            let mut src = ExperimentSource::new();
            src.set_run_script(format!("#!/bin/bash\necho $FOO\necho $FOO >> out_file"));
            src.set_envs(HashMap::from([
                (PathBuf::from(TEST_RUN_REP_DIR0),Environment::from_env_list(vec![("FOO".to_string(), "BAR".to_string())])),
                (PathBuf::from(TEST_RUN_REP_DIR1),Environment::from_env_list(vec![("FOO".to_string(), "Z".to_string())])),
            ]));

            // no error
            trial(&src, MultiProgress::new()).unwrap();
        }
    }
}
