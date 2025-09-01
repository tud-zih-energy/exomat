pub mod harness {
    pub mod env;
    pub mod run;
    pub mod skeleton;
    pub mod table;
}
pub mod helper {
    pub mod archivist;
    pub mod errors;
    pub mod fs_names;
}

use chrono::Local;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use indicatif_log_bridge::LogWrapper;
use log::{info, trace};
use rand::seq::SliceRandom;
use spdlog::formatter::{pattern, PatternFormatter};
use spdlog::sink::FileSink;
use std::{path::Path, path::PathBuf, sync::Arc};

use harness::env::Environment;
use helper::archivist::find_marker_pwd;
use helper::errors::{Error, Result};
use helper::fs_names::*;

/// Initializes logging for all severity levels from info and up.
///
/// Logging will still work if this function is not called, however, only
/// messages of info-severity or higher will be recorded.
///
/// Logging messages will be handled by `spdlog` and printed to stdout.
///
/// ## Usage of return value
/// The returned `Multiprogress` can be used to stop log messages from interfering
/// with an `indicatif` progress bar. If you do not have a progress bar, you can
/// savely ignore this.
///
/// ```
/// use exomat::activate_logging;
/// use log::info;
/// use indicatif::{MultiProgress, ProgressBar};
///
/// let logging_handler = activate_logging(log::LevelFilter::Info);
/// let prog_bar = ProgressBar::new(42);
///
/// // protect progress bar from log Interference
/// let prog_bar = logging_handler.add(prog_bar);
///
/// // do work
/// for i in 1..10 {
///     info!("This will look nice and not intermingle!");
///     prog_bar.inc(1);
/// }
///
/// prog_bar.finish();
/// ```
pub fn activate_logging(verbosity: log::LevelFilter) -> MultiProgress {
    // configure the logger, default logger does not work because it gets messed up
    // when having multiple sinks with different level filters
    let pattern = pattern!("[{date} {time}.{millisecond}] [{level}] {payload}{eol}");
    let logger = spdlog::Logger::builder()
        .level_filter(spdlog::LevelFilter::All)
        .sink(Arc::new(
            spdlog::sink::StdStreamSink::builder()
                .formatter(Box::new(PatternFormatter::new(pattern)))
                .level_filter(spdlog::LevelFilter::from(verbosity))
                .std_stream(spdlog::sink::StdStream::Stdout)
                .build()
                .unwrap(),
        ))
        .build()
        .unwrap();

    // configure the logger, init spdlog, log and log_bridge
    spdlog::set_default_logger(Arc::new(logger));
    spdlog::re_export::log::set_max_level(spdlog::re_export::log::LevelFilter::Trace);

    let multi = MultiProgress::new();
    let log_wrapper = LogWrapper::new(multi.clone(), spdlog::log_crate_proxy());
    log_wrapper.try_init().map_err(Error::LoggerError).unwrap();

    multi
}

/// Disables log output to stdout.
///
/// warning: will reset the effect of duplicate_log_to_file()!
fn disable_console_log() {
    // create logger that logs to log_file
    let pattern = pattern!("[{date} {time}.{millisecond}] [{level}] {payload}{eol}");
    let new_logger = spdlog::Logger::builder()
        .level_filter(spdlog::LevelFilter::All)
        .sink(Arc::new(
            spdlog::sink::StdStreamSink::builder()
                .formatter(Box::new(PatternFormatter::new(pattern)))
                .level_filter(spdlog::LevelFilter::Off)
                .std_stream(spdlog::sink::StdStream::Stdout)
                .build()
                .unwrap(),
        ))
        .build()
        .unwrap();

    // update logger
    spdlog::set_default_logger(Arc::new(new_logger));
}

/// Duplicate logging messages to `log_file`.
///
/// This does not overwrite previous configurations of the logger. It simply adds
/// `log_file` as an additional output for log messages without a level filter.
///
/// If the default logger was not initilized by `activate_logging()` before, this
/// will not initialize the logger, so no messages will be written to the file.
pub fn duplicate_log_to_file(log_file: &PathBuf) {
    let pattern = pattern!("[{date} {time}.{millisecond}] [{level}] {payload}{eol}");

    // create logger that logs to log_file
    let new_logger = spdlog::default_logger()
        .fork_with(|new| {
            let file_sink = Arc::new(
                FileSink::builder()
                    .formatter(Box::new(PatternFormatter::new(pattern)))
                    .level_filter(spdlog::LevelFilter::All)
                    .path(log_file)
                    .build()?,
            );
            new.sinks_mut().push(file_sink);
            Ok(())
        })
        .map_err(|e| Error::HarnessCreateError {
            entry: log_file.display().to_string(),
            reason: e.to_string(),
        })
        .expect("Could not create new logger: {e}");

    // update logger
    spdlog::set_default_logger(new_logger);
}

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
pub fn run_experiment(
    experiment: &PathBuf,
    repetitions: u64,
    output: PathBuf,
    log_progress_handler: MultiProgress,
    is_trial: bool,
) -> Result<()> {
    harness::skeleton::build_series_directory(experiment, &output)?;
    execute_exp_repetitions(
        experiment,
        &output,
        repetitions,
        log_progress_handler,
        is_trial,
    )
}

/// Creates an experiment series/run directory for the given `experiment`.
/// Then executes the `run.sh` file for this experiment once and collects any
/// output/errors/results.
///
/// The new experiment series directory will be created as a tempdir. The
pub fn run_trial(experiment: &PathBuf, log_progress_handler: MultiProgress) -> Result<()> {
    let exp_name = file_name_string(&experiment.canonicalize().unwrap());

    if experiment.display().to_string() == "." {
        return Err(Error::HarnessRunError {
            experiment: exp_name,
            err: "Cannot start experiment run from the experiment source folder.".to_string(),
        });
    }

    let format = &Local::now()
        .format("exomat_trial-%Y-%m-%d-%H-%M-%S")
        .to_string();

    let trial_dir_path = std::env::temp_dir().join(format);

    disable_console_log();

    // run experiment once
    let res = run_experiment(
        experiment,
        1,
        trial_dir_path.clone(),
        log_progress_handler,
        true,
    );

    // flush exomat log
    spdlog::default_logger().flush();

    // gather results
    let stdout =
        std::fs::read_to_string(trial_dir_path.join(SERIES_RUNS_DIR).join(SERIES_STDOUT_LOG))?;
    let stderr =
        std::fs::read_to_string(trial_dir_path.join(SERIES_RUNS_DIR).join(SERIES_STDERR_LOG))?;
    let exomat =
        std::fs::read_to_string(trial_dir_path.join(SERIES_RUNS_DIR).join(SERIES_EXOMAT_LOG))?;

    let eval_res = harness::run::create_report(&exp_name, &res, &stdout, &stderr, &exomat);
    print!("{eval_res}");

    res
}

// / Runs the experiment defined in `exp_source_dir` `repetitions` times for each
/// environment.
///
/// This will create a new experiment run folder inside `exp_series_dir`.
///
/// This functions assumes that `build_series_directory` has been called before it.
/// Otherwise it will fail, because the files it expects to be there are not.
fn execute_exp_repetitions(
    exp_source_dir: &Path,
    exp_series_dir: &Path,
    repetitions: u64,
    log_progress_handler: MultiProgress,
    is_trial: bool,
) -> Result<()> {
    let length = repetitions.to_string().len();
    let envs =
        harness::env::fetch_env_files(&exp_source_dir.join(SRC_ENV_DIR)).ok_or_else(|| {
            Error::HarnessRunError {
                experiment: exp_source_dir.display().to_string(),
                err: format!(
                    "No environments found in {}",
                    exp_source_dir.join(SRC_ENV_DIR).display()
                ),
            }
        })?;

    let prog_bar = ProgressBar::new(repetitions * envs.len() as u64);
    prog_bar.set_style(
        ProgressStyle::with_template("[{elapsed_precise}] [{bar:.green}] {pos}/{len} ({eta})")
            .unwrap()
            .progress_chars("#>-"),
    );

    // protect progress bar from log interferance
    let prog_bar = log_progress_handler.add(prog_bar);
    prog_bar.tick(); // show on 0th repetition

    info!("Starting experiment runs for {}", exp_source_dir.display());

    let running_order: Vec<(&PathBuf, u64)> = shuffle_experiments(&envs, &repetitions);
    for (environment, rep) in running_order {
        let run_folder =
            harness::skeleton::build_run_directory(exp_series_dir, &environment, rep, length)?;
        trace!(
            "Using envs: {:?}",
            harness::env::Environment::from_file(&environment)?
        );

        harness::run::run_experiment(&file_name_string(exp_source_dir), &run_folder)?;

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

/// Compiles a list of all repetition for each environment, then suffles said list.
///
/// The shuffled list is then sorted by repetition, so that all n-repetitions run
/// before all n+1-repetitions.
fn shuffle_experiments<'a>(
    environments: &'a Vec<PathBuf>,
    repetition_count: &'a u64,
) -> Vec<(&'a PathBuf, u64)> {
    let mut running_order = vec![];

    for env in environments {
        for rep in 0..*repetition_count {
            running_order.push((env, rep));
        }
    }

    running_order.shuffle(&mut rand::rng());
    running_order.sort_by(|a, b| (a.1).cmp(&b.1));

    return running_order;
}

/// Filters output (files) from every run repetition in the pwd.
///
/// Looks through every `series_dir/runs/run_*` directory and accumulates the content of
/// every `out_*` file into one csv file.
///
/// ## Example
/// ```bash
/// exp_series
/// \-> runs
///     |-> run_0_rep0
///     |   |-> out_foo # content: "42"
///     |   \-> out_bar # content: "true"
///     \-> run_0_rep1
///         |-> out_foo # content: "300"
///         \-> out_bar # content: "false"
/// ```
/// results in `exp_series.csv` with:
/// ```notest
/// foo,bar
/// 42, true
/// 300,false
/// ```
///
pub fn make_table() -> Result<()> {
    let series_dir = find_marker_pwd(MARKER_SERIES)?;

    // collect all output from every run in series_dir
    let out_content = harness::table::collect_output(&series_dir)?;
    info!("Collected output for {} keys", out_content.len());
    info!("Found keys: {:?}", out_content.keys());

    // output file will be "series_dir/[series_dir].csv"
    let mut out_file = PathBuf::from(
        series_dir
            .file_name()
            .expect("Could not read experiment series name"),
    );
    out_file.set_extension("csv");

    // serialize data and write to file
    harness::table::serialize_csv(&series_dir.join(out_file), &out_content)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    use rusty_fork::rusty_fork_test;
    use std::fs::OpenOptions;
    use std::io::Write;
    use tempfile::TempDir;

    use log::{error, info, trace, warn};
    use tempfile::NamedTempFile;

    rusty_fork_test! {

        // this is in here to that logging is not enables for all following tests
        #[test]
        fn log_to_file() {
            let log = NamedTempFile::with_suffix("log").unwrap();
            let log = log.path().to_path_buf();

            activate_logging(log::LevelFilter::Info);
            trace!("Trace on console");
            info!("Info on console");
            warn!("Warn on console");
            error!("Error on console");

            duplicate_log_to_file(&log);
            trace!("Trace in file");
            info!("Info in file");
            warn!("Warn in file");
            error!("Error in file");

            // simulate program ending
            spdlog::default_logger().flush();

            let file_content = std::fs::read_to_string(&log).unwrap();
            print!("log: {file_content}\n");

            assert!(file_content.contains("Trace in file"));
            assert!(file_content.contains("Info in file"));
            assert!(file_content.contains("Warn in file"));
            assert!(file_content.contains("Error in file"));
            assert!(!file_content.contains("on console"));
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
            harness::skeleton::main(&PathBuf::from(exp_name)).unwrap();

            // Write something to run.sh that uses env var
            let mut run_sh = OpenOptions::new()
                .append(true)
                .open(
                    &tmpdir
                        .join(exp_name)
                        .join(SRC_TEMPLATE_DIR)
                        .join(SRC_RUN_FILE),
                )
                .unwrap();
            run_sh
                .write("echo $FOO\necho $FOO >> out_file".as_bytes()) // write to stdout and in file
                .unwrap();

            // make multiple .env files that set $FOO to different values
            let mut env1 =
                std::fs::File::create(&tmpdir.join(exp_name).join(SRC_ENV_DIR).join("0.env")).unwrap();

            let mut env2 =
                std::fs::File::create(&tmpdir.join(exp_name).join(SRC_ENV_DIR).join("m.env")).unwrap();

            env1.write_all("FOO=BAR".as_bytes()).unwrap();
            env2.write_all("FOO=Z".as_bytes()).unwrap();

            // run experiment and check logs
            run_experiment(
                &PathBuf::from(exp_name.to_string()),
                1,
                PathBuf::from(out_name),
                MultiProgress::new(), // empty
                false
            )
            .unwrap();

            let stdout_log = std::fs::read_to_string(
                tmpdir
                    .join(out_name)
                    .join(SERIES_RUNS_DIR)
                    .join(SERIES_STDOUT_LOG),
            )
            .unwrap();
            let stderr_log = std::fs::read_to_string(
                tmpdir
                    .join(out_name)
                    .join(SERIES_RUNS_DIR)
                    .join(SERIES_STDERR_LOG),
            )
            .unwrap();

            assert_eq!(stderr_log.lines().count(), 0);
            // two lines for variable (inserted here), two lines for current time (part of run.sh template)
            assert_eq!(dbg!(stdout_log.lines()).count(), 4);
            assert!(stdout_log.contains("Z"));
            assert!(stdout_log.contains("BAR"));

            // take one out_file and check its content
            let output = std::fs::read_to_string(
                tmpdir
                    .join(out_name)
                    .join(SERIES_RUNS_DIR)
                    .join("run_0_rep0/out_file"),
            )
            .unwrap();
            assert_eq!(output.lines().count(), 1);
            assert!(output.contains("BAR"));
        }

        #[test]
        fn trial_e2e() {
            // create ouput dir
            let tmpdir = TempDir::new().unwrap();
            let tmpdir = tmpdir.path().to_path_buf();
            std::env::set_current_dir(&tmpdir).unwrap();
            let exp_name = "SomeExperiment";

            // build basic experiment
            harness::skeleton::main(&PathBuf::from(exp_name)).unwrap();

            // Write something to run.sh that uses env var
            let mut run_sh = OpenOptions::new()
                .append(true)
                .open(&tmpdir.join(exp_name).join("template").join("run.sh"))
                .unwrap();
            run_sh
                .write("echo $FOO\necho $FOO >> out_file".as_bytes()) // write to stdout and in file
                .unwrap();

            // make multiple .env files that set $FOO to different values
            let mut env1 =
                std::fs::File::create(&tmpdir.join(exp_name).join("envs").join("0.env")).unwrap();

            let mut env2 =
                std::fs::File::create(&tmpdir.join(exp_name).join("envs").join("m.env")).unwrap();

            env1.write_all("FOO=BAR".as_bytes()).unwrap();
            env2.write_all("FOO=Z".as_bytes()).unwrap();

            // no error
            run_trial(&PathBuf::from(exp_name), MultiProgress::new()).unwrap();
        }
    }
}
