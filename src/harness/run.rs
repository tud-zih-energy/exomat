//! harness run subcommand

use chrono::Local;
use indicatif::ProgressStyle;
use log::{info, trace};
use std::path::PathBuf;
use tracing::info_span;
use tracing_indicatif::span_ext::IndicatifSpanExt;

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
    output: Option<PathBuf>,
    is_trial: bool,
) -> Result<()> {
    let output = match output {
        Some(x) => x,
        None => ExperimentSeries::generate_series_filepath(experiment.location())?,
    };

    let mut series = ExperimentSeries::from_source(experiment)?;
    series.generate_runs()?;
    series.persist(&output)?;

    execute_exp_repetitions(&mut series, is_trial)
}

/// Creates an experiment series/run directory for the given `experiment`.
/// Then executes the `run.sh` file for this experiment once and collects any
/// output/errors/results.
///
/// The new experiment series directory will be created as a tempdir.
pub fn trial(experiment: &ExperimentSource) -> Result<()> {
    let format = &Local::now()
        .format("exomat_trial-%Y-%m-%d-%H-%M-%S")
        .to_string();
    let trial_dir_path = std::env::temp_dir().join(format);
    let trial = experiment.to_trial_source();

    // run experiment once
    let res = self::experiment(&trial, Some(trial_dir_path.clone()), true);

    // gather results
    let mut reader = ExperimentSeries::parse(&trial_dir_path)?;
    reader.include_source(&trial);
    println!("{reader}");

    res
}

/// Runs the experiment defined in `exp_source_dir` `repetitions` times for each
/// environment.
///
/// This will create a new experiment run folder inside `exp_series_dir`.
///
/// This functions assumes that `build_series_directory` has been called before it.
/// Otherwise it will fail, because the files it expects to be there are not.
fn execute_exp_repetitions(series: &mut ExperimentSeries, is_trial: bool) -> Result<()> {
    // if series
    //     Error::HarnessRunError {
    //         experiment: exp_source_dir.display().to_string(),
    //         err: format!(
    //             "No environments found in {}",
    //             exp_source_dir.join(SRC_ENV_DIR).display()
    //         ),
    //     }
    // })?;

    let series_span = info_span!("run_series");

    let prog_bar_len = match is_trial {
        true => 1,
        false => series.repetition_count(),
    };
    series_span.pb_set_style(
        &ProgressStyle::with_template("[{elapsed_precise}] [{bar:.green}] {pos}/{len} ({eta})")
            .unwrap()
            .progress_chars("=>-"),
    );
    series_span.pb_set_length(prog_bar_len);
    series_span.pb_set_message("run experiment series");
    series_span.pb_set_finish_message("all experiments done");
    let _span_entered = series_span.enter();

    tracing::Span::current().pb_tick(); // show on 0th repetition

    let experiment_name = series.experiment_name()?;
    info!("Starting experiment runs for {experiment_name}");
    trace!("exomat envs are: {:?}", series.exomat_envs());

    for run in series.runs.iter_mut() {
        trace!("Using envs: {:?}", run.env());

        run.execute(&experiment_name)?;

        // update progress
        tracing::Span::current().pb_inc(1);

        // stop after one run if this is a trial
        if is_trial {
            break;
        }
    }

    // progress bar closed using RAII
    Ok(())
}

#[cfg(test)]
mod tests {
    use rusty_fork::rusty_fork_test;
    use std::collections::BTreeMap;
    use std::path::PathBuf;
    use tempfile::TempDir;

    use super::*;
    use crate::experiment::{ExperimentRun, ExperimentSource, FileWriter};
    use crate::harness::env::{Environment, ExomatEnvironment};
    use crate::test_helper::read_log;

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
            src.set_run_script(format!("#!/usr/bin/env bash\necho $EXP_SRC_DIR\necho $EXP_SRC_DIR >> out_file"));
            src.set_exomat_envs(ExomatEnvironment::new(&exp_source, 1));
            src.persist(&exp_source).unwrap();

            let mut ser = ExperimentSeries::from_source(&src).unwrap();
            ser.generate_runs().unwrap();
            ser.persist(&exp_series).unwrap();

            // run experiment
            assert_eq!(ser.runs().len(), 1);
            let run: &mut  ExperimentRun = ser.runs_mut().first_mut().unwrap();

            run.execute(exp_source.file_name().unwrap().to_str().unwrap()).unwrap();
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
            src.set_run_script(format!("#!/usr/bin/env bash\necho $FOO\necho $FOO >> out_file"));
            src.set_envs(BTreeMap::from([
                (PathBuf::from("0.env"), Environment::from_env_list(vec![("FOO".to_string(), "BAR".to_string())])),
                (PathBuf::from("1.env"), Environment::from_env_list(vec![("FOO".to_string(), "Z".to_string())])),
            ])).unwrap();

            src.persist(&tmpdir.join(exp_name)).unwrap();

            // run experiment and check logs
            experiment(
                &src,
                Some(PathBuf::from(out_name)),
                false
            )
            .unwrap();

            // take one out_file and check its content
            let output = read_log(tmpdir.join(out_name), format!("run_0_rep0/out_file").as_str());
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
            src.set_run_script(format!("#!/usr/bin/env bash\necho $FOO\necho $FOO >> out_file"));
            src.set_envs(BTreeMap::from([
                (PathBuf::from("0.env"),Environment::from_env_list(vec![("FOO".to_string(), "BAR".to_string())])),
                (PathBuf::from("1.env"),Environment::from_env_list(vec![("FOO".to_string(), "Z".to_string())])),
            ])).unwrap();
            src.persist(&tmpdir.join("TestSource")).unwrap();

            // no error
            trial(&src).unwrap();
        }
    }
}
